use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    error::{HandlerError, HandlerErrorKind},
    server::{location::LocationResult, ServerState},
    tags::Tags,
    web::{FormFactor, OsFamily},
};

/// The response message sent to the User Agent.
#[derive(Debug, Deserialize, Serialize)]
pub struct AdmTileResponse {
    pub tiles: Vec<AdmTile>,
}

/// The tile data provided by ADM
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

/// Main handler for the User Agent HTTP request
///
#[allow(clippy::too_many_arguments)]
pub async fn get_tiles(
    reqwest_client: &reqwest::Client,
    adm_endpoint_url: &str,
    location: &LocationResult,
    os_family: OsFamily,
    form_factor: FormFactor,
    state: &ServerState,
    tags: &mut Tags,
) -> Result<AdmTileResponse, HandlerError> {
    // XXX: Assumes adm_endpoint_url includes
    // ?partner=<mozilla_partner_name>&sub1=<mozilla_tag_id> (probably should
    // validate this on startup)
    let settings = &state.settings;
    let adm_url = Url::parse_with_params(
        adm_endpoint_url,
        &[
            ("partner", settings.partner_id.as_str()),
            ("sub1", settings.sub1.as_str()),
            ("ip", &location.fake_ip), // TODO: remove once ADM API finalized
            ("country-code", &location.country()),
            ("region-code", &location.region()),
            // ("dma-code", location.dma),
            ("form-factor", &form_factor.to_string()),
            ("os-family", &os_family.to_string()),
            ("sub2", "newtab"),
            ("v", "1.0"),
            // XXX: some value for results seems required, it defaults to 0
            // when omitted (despite AdM claiming it would default to 1)
            ("results", &settings.adm_query_tile_count.to_string()),
        ],
    )
    .map_err(|e| HandlerError::internal(&e.to_string()))?;
    let adm_url = adm_url.as_str();

    trace!("get_tiles GET {}", adm_url);
    let mut response: AdmTileResponse = reqwest_client
        .get(adm_url)
        .send()
        .await
        .map_err(|e| {
            // ADM servers are down, or improperly configured
            HandlerErrorKind::BadAdmResponse(format!("ADM Server Error: {:?}", e))
        })?
        .error_for_status()?
        .json()
        .await
        .map_err(|e| {
            // ADM servers are not returning correct information
            HandlerErrorKind::BadAdmResponse(format!("ADM provided invalid response: {:?}", e))
        })?;
    response.tiles = response
        .tiles
        .into_iter()
        .filter_map(|tile| state.filter.filter_and_process(tile, tags))
        .take(settings.adm_max_tiles as usize)
        .collect();
    Ok(response)
}
