use std::{fmt::Debug, fs::File, io::BufReader, path::Path, time::Duration};

use actix_http::http::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    adm::DEFAULT,
    error::{HandlerError, HandlerErrorKind, HandlerResult},
    metrics::Metrics,
    server::{img_storage::ImageMetrics, location::LocationResult, ServerState},
    settings::Settings,
    tags::Tags,
    web::DeviceInfo,
};

/// The payload provided by ADM
#[derive(Debug, Deserialize, Serialize)]
pub struct AdmTileResponse {
    #[serde(default)]
    pub tiles: Vec<AdmTile>,
}

impl AdmTileResponse {
    /// Return a fake response from the contents of `response_file`
    ///
    /// This is only used when the server is in `test_mode` and passed a `fake-response` header.
    /// The test file is located in `CONTILE_TEST_FILE_PATH`, and will be lowercased. Unless
    /// specified, the `CONTILE_TEST_PATH` is `tools/test/test_data` and presumes that you are
    /// running in the Project Root directory. An example resolution for a `Fake-Response:DEFAULT`
    /// would be to open `./tools/test/test_data/default.json`. If you are not running in the
    /// Project root, you will need to specify the full path in `CONTILE_TEST_FILE_PATH`.
    pub fn fake_response(settings: &Settings, mut response_file: String) -> HandlerResult<Self> {
        trace!("Response file: {:?}", &response_file);
        response_file.retain(|x| char::is_alphanumeric(x) || x == '_');
        if response_file.is_empty() {
            return Err(HandlerError::internal(
                "Invalid test response file specified",
            ));
        }
        let path = Path::new(&settings.test_file_path)
            .join(format!("{}.json", response_file.to_lowercase()));
        if path.exists() {
            let file =
                File::open(path.as_os_str()).map_err(|e| HandlerError::internal(&e.to_string()))?;
            let reader = BufReader::new(file);
            let content = serde_json::from_reader(reader)
                .map_err(|e| HandlerError::internal(&e.to_string()))?;
            trace!("Content: {:?}", &content);
            return Ok(content);
        }
        let err = format!(
            "Invalid or missing test file {}",
            path.to_str().unwrap_or(&response_file)
        );
        trace!("Err: {:?}", &err);
        Err(HandlerError::internal(&err))
    }
}

/// The individual tile data provided by ADM
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AdmTile {
    pub id: u64,
    pub name: String,
    pub advertiser_url: String,
    pub click_url: String,
    pub image_url: String,
    pub impression_url: String,
    pub position: Option<u8>,
}

/// The response payload sent to the User Agent
#[derive(Debug, Deserialize, Serialize)]
pub struct TileResponse {
    pub tiles: Vec<Tile>,
}

/// The individual tile data sent to the User Agent
/// Differs from AdmTile in:
///   - advertiser_url -> url
///   - includes an optional position
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tile {
    pub id: u64,
    pub name: String,
    pub url: String,
    pub click_url: String,
    pub image_url: String,
    pub image_size: Option<u32>,
    pub image_metrics: ImageMetrics,
    pub impression_url: String,
    pub position: Option<u8>,
    #[serde(skip_serializing)]
    pub new: bool,
}

impl Tile {
    pub fn from_adm_tile(tile: AdmTile, position: Option<u8>, new: bool) -> Self {
        Self {
            id: tile.id,
            name: tile.name,
            url: tile.advertiser_url,
            click_url: tile.click_url,
            image_url: tile.image_url,
            image_size: None,
            impression_url: tile.impression_url,
            image_metrics: ImageMetrics::default(),
            position,
            new,
        }
    }
}

/// Main handler for the User Agent HTTP request
///
pub async fn get_tiles(
    state: &ServerState,
    location: &LocationResult,
    device_info: DeviceInfo,
    tags: &mut Tags,
    metrics: &Metrics,
    headers: Option<&HeaderMap>,
) -> Result<TileResponse, HandlerError> {
    let settings = &state.settings;
    let image_store = &state.img_store;
    let adm_url = Url::parse_with_params(
        &state.adm_endpoint_url,
        &[
            ("partner", settings.adm_partner_id.clone().unwrap().as_str()),
            ("sub1", settings.adm_sub1.clone().unwrap().as_str()),
            ("country-code", &location.country()),
            ("region-code", &location.region()),
            // ("dma-code", location.dma),
            ("form-factor", &device_info.form_factor.to_string()),
            ("os-family", &device_info.os_family.to_string()),
            ("sub2", "newtab"),
            ("v", "1.0"),
            // XXX: some value for results seems required, it defaults to 0
            // when omitted (despite AdM claiming it would default to 1)
            ("results", &settings.adm_query_tile_count.to_string()),
        ],
    )
    .map_err(|e| HandlerError::internal(&e.to_string()))?;
    let adm_url = adm_url.as_str();

    info!("adm::get_tiles GET {}", adm_url);
    metrics.incr("tiles.adm.request");
    let response: AdmTileResponse = if state.settings.test_mode {
        let default = HeaderValue::from_str(DEFAULT).unwrap();
        let test_response = headers
            .unwrap_or(&HeaderMap::new())
            .get("fake-response")
            .unwrap_or(&default)
            .to_str()
            .unwrap()
            .to_owned();
        trace!("Getting fake response: {:?}", &test_response);
        AdmTileResponse::fake_response(&state.settings, test_response)?
    } else {
        state
            .reqwest_client
            .get(adm_url)
            .timeout(Duration::from_secs(settings.adm_timeout))
            .send()
            .await
            .map_err(|e| {
                // ADM servers are down, or improperly configured
                let mut err: HandlerError = HandlerErrorKind::AdmServerError().into();
                err.tags.add_extra("error", &e.to_string());
                err
            })?
            .error_for_status()?
            .json()
            .await
            .map_err(|e| {
                // ADM servers are not returning correct information
                HandlerErrorKind::BadAdmResponse(format!("ADM provided invalid response: {:?}", e))
            })?
    };
    if response.tiles.is_empty() {
        warn!("adm::get_tiles empty response {}", adm_url);
    }

    let filtered: Vec<Tile> = response
        .tiles
        .into_iter()
        .filter_map(|tile| state.filter.filter_and_process(tile, tags, metrics))
        .take(settings.adm_max_tiles as usize)
        .collect();

    let mut tiles: Vec<Tile> = Vec::new();
    for mut tile in filtered {
        if tile.new {
            if let Some(storage) = image_store {
                // we should have already proven the image_url in `filter_and_process`
                let result = storage.store(&tile.image_url.parse().unwrap()).await?;
                tile.image_url = result.url.to_string();
                tile.image_size = Some(result.image_metrics.width);
                tile.image_metrics = result.image_metrics;
            }
        }
        tiles.push(tile);
    }
    Ok(TileResponse { tiles })
}
