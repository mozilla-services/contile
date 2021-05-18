use actix_http::http::{header::HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug, fs::File, io::BufReader, path::Path};
use url::Url;

use crate::error::{HandlerError, HandlerErrorKind, HandlerResult};
use crate::server::location::LocationResult;
use crate::server::ServerState;
use crate::settings::Settings;
use crate::tags::Tags;
use crate::web::middleware::sentry as l_sentry;
//use crate::server::img_storage;

pub(crate) const DEFAULT: &str = "DEFAULT";

#[derive(Debug, Deserialize, Serialize)]
pub struct AdmTileResponse {
    pub tiles: Vec<AdmTile>,
}

impl AdmTileResponse {
    pub fn fake_response(settings: &Settings, mut response_file: String) -> HandlerResult<Self> {
        response_file.retain(char::is_alphanumeric);
        let path = Path::new(&settings.test_file_path).join(format!("{}.json", response_file));
        if path.exists() {
            let file =
                File::open(path.as_os_str()).map_err(|e| HandlerError::internal(&e.to_string()))?;
            let reader = BufReader::new(file);
            let content = serde_json::from_reader(reader)
                .map_err(|e| HandlerError::internal(&e.to_string()))?;
            dbg!(&content);
            return Ok(content);
        }
        Err(HandlerError::internal(&format!(
            "Invalid test file {}",
            response_file
        )))
    }
}

/// Filter criteria for adm Tiles
/// Each "filter"  is a set of [AdmAdvertiserFilterSettings] that are
/// specific to a given Advertiser name (the names are matched against
/// the tiles fetch request)
/// In addition there is a special [DEFAULT] value which is a filter
/// that will be applied to all advertisers that do not supply their
/// own values.
#[derive(Default, Clone, Debug)]
pub struct AdmFilter {
    pub filter_set: HashMap<String, AdmAdvertiserFilterSettings>,
}

/// The AdmAdvertiserFilterSettings contain the settings for the various
/// ADM provided partners. These are specified as a JSON formatted hash
/// that contains the components. A special "DEFAULT" setting provides
/// information that may be used as a DEFAULT, or commonly appearing set
/// of data.
/// See `impl From<Settings>` for details of the structure.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AdmAdvertiserFilterSettings {
    /// Set of valid hosts for the `advertiser_url`
    pub(crate) advertiser_hosts: Vec<String>,
    /// Set of valid hosts for the `impression_url`
    #[serde(default)]
    pub(crate) impression_hosts: Vec<String>,
    /// Set of valid hosts for the `click_url`
    #[serde(default)]
    pub(crate) click_hosts: Vec<String>,
    /// valid position for the tile
    pub(crate) position: Option<u8>,
    /// Set of valid regions for the tile (e.g ["en", "en-US/TX"])
    #[serde(default)]
    pub(crate) include_regions: Vec<String>,
}

pub(crate) type AdmSettings = HashMap<String, AdmAdvertiserFilterSettings>;
impl From<&Settings> for AdmSettings {
    fn from(settings: &Settings) -> Self {
        if settings.adm_settings.is_empty() {
            // TODO: Read these out of a file?
            let mut def = Self::default();
            if settings.test_mode {
                def.insert(
                    "acme".to_owned(),
                    AdmAdvertiserFilterSettings {
                        advertiser_hosts: vec!["www.acme.biz".to_owned()],
                        position: Some(0),
                        ..Default::default()
                    },
                );
                def.insert(
                    "dunder mifflin".to_owned(),
                    AdmAdvertiserFilterSettings {
                        advertiser_hosts: vec!["www.dunderm.biz".to_owned()],
                        position: Some(1),
                        ..Default::default()
                    },
                );
                def.insert(
                    "los pollos hermanos".to_owned(),
                    AdmAdvertiserFilterSettings {
                        advertiser_hosts: vec!["www.lph-nm.biz".to_owned()],
                        ..Default::default()
                    },
                );
                def.insert(
                    "default".to_owned(),
                    AdmAdvertiserFilterSettings {
                        advertiser_hosts: vec!["example.com".to_string()],
                        impression_hosts: vec!["example.net".to_string()],
                        click_hosts: vec!["example.com".to_string()],
                        ..Default::default()
                    },
                );
            };
            return def;
        }
        dbg!(&settings.adm_settings);
        if Path::new(&settings.adm_settings).exists() {
            if let Ok(f) = File::open(&settings.adm_settings) {
                return serde_json::from_reader(f).expect("Invalid ADM Settings file");
            }
        }
        serde_json::from_str(&settings.adm_settings).expect("Invalid ADM Settings")
    }
}

/// Check that a given URL is valid according to it's corresponding filter
fn check_url(
    url: &str,
    species: &'static str,
    filter: &[String],
    tags: &mut Tags,
) -> HandlerResult<()> {
    let parsed: Url = match url.parse() {
        Ok(v) => v,
        Err(e) => {
            tags.add_tag("type", species);
            tags.add_extra("parse_error", &e.to_string());
            tags.add_extra("url", &url);
            return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
        }
    };
    let host = match parsed.host() {
        Some(v) => v.to_string(),
        None => {
            tags.add_tag("type", species);
            tags.add_extra("url", &url);
            return Err(HandlerErrorKind::MissingHost(species, parsed.to_string()).into());
        }
    };
    if !filter.contains(&host) {
        tags.add_tag("type", species);
        tags.add_extra("url", &url);
        return Err(HandlerErrorKind::UnexpectedHost(species, host).into());
    }
    Ok(())
}

impl AdmFilter {
    /// Report the error directly to sentry
    fn report(&self, error: &HandlerError, tags: &Tags) {
        // dbg!(&error, &tags);
        // TODO: if not error.is_reportable, just add to metrics.
        l_sentry::report(tags, sentry::event_from_error(error));
    }

    /// Check the advertiser URL
    fn check_advertiser(
        &self,
        filter: &AdmAdvertiserFilterSettings,
        tile: &mut AdmTile,
        tags: &mut Tags,
    ) -> HandlerResult<()> {
        check_url(
            &tile.advertiser_url,
            "Advertiser",
            &filter.advertiser_hosts,
            tags,
        )
    }

    /// Check the click URL
    fn check_click(
        &self,
        filter: &AdmAdvertiserFilterSettings,
        tile: &mut AdmTile,
        tags: &mut Tags,
    ) -> HandlerResult<()> {
        check_url(&tile.click_url, "Click", &filter.click_hosts, tags)
    }

    /// Check the impression URL to see if it's valid.
    ///
    /// This extends `filter_and_process`
    fn check_impression(
        &self,
        filter: &AdmAdvertiserFilterSettings,
        tile: &mut AdmTile,
        tags: &mut Tags,
    ) -> HandlerResult<()> {
        check_url(
            &tile.impression_url,
            "Impression",
            &filter.impression_hosts,
            tags,
        )
    }

    /// Filter and process tiles from ADM:
    ///
    /// - Returns None for tiles that shouldn't be shown to the client
    /// - Modifies tiles for output to the client (adding additional fields, etc.)
    pub fn filter_and_process(&self, mut tile: AdmTile, tags: &mut Tags) -> Option<AdmTile> {
        // Use strict matching for now, eventually, we may want to use backwards expanding domain
        // searches, (.e.g "xyz.example.com" would match "example.com")
        match self.filter_set.get(&tile.name.to_lowercase()) {
            Some(filter) => {
                // Apply any additional tile filtering here.
                let none = AdmAdvertiserFilterSettings::default();
                let default = self
                    .filter_set
                    .get(&DEFAULT.to_lowercase())
                    .unwrap_or(&none);
                // if the filter doesn't have anything defined, try using what's in the default.
                // Sadly, `vec.or()` doesn't exist, so do this a bit "long hand"
                let adv_filter = if filter.advertiser_hosts.is_empty() {
                    default
                } else {
                    filter
                };
                let impression_filter = if filter.impression_hosts.is_empty() {
                    default
                } else {
                    filter
                };
                let click_filter = if filter.click_hosts.is_empty() {
                    default
                } else {
                    filter
                };
                if let Err(e) = self.check_advertiser(adv_filter, &mut tile, tags) {
                    self.report(&e, tags);
                    return None;
                }
                if let Err(e) = self.check_click(click_filter, &mut tile, tags) {
                    self.report(&e, tags);
                    return None;
                }
                if let Err(e) = self.check_impression(impression_filter, &mut tile, tags) {
                    self.report(&e, tags);
                    return None;
                }
                // Use the default.position (Option<u8>) if the filter.position (Option<u8>) isn't
                // defined. In either case `None` is a valid return, but we should favor `filter` over
                // `default`.
                tile.position = filter.position.or(default.position);
                Some(tile)
            }
            None => {
                self.report(
                    &HandlerErrorKind::UnexpectedAdvertiser(tile.name).into(),
                    tags,
                );
                None
            }
        }
    }
}

/// Construct the AdmFilter from the provided settings.
/// This uses a JSON construct of settings, e.g.
/// ```javascript
/// /* for the Example Co advertiser... */
/// {"Example": {
///     /* The allowed hosts for URLs */
///     "advertiser_hosts": ["www.example.org", "example.org"],
///     /* Valid tile positions for this advertiser (empty for "all") */
///     "positions": 1,
///     /* Valid target regions for this advertiser
///        (use "en-US" for "all in english speaking United States") */
///     "include_regions": ["en-US/TX", "en-US/CA"],
///     /* Allowed hosts for impression URLs.
///        Empty means to use the impression URLs in "DEFAULT" */
///     "impression_hosts: [],
///     },
///     ...,
///  "DEFAULT": {
///    /* The default impression URL host to check for. */
///    "impression_hosts": ["example.net"]
///     }
/// }
/// ```
///
impl From<&Settings> for HandlerResult<AdmFilter> {
    fn from(settings: &Settings) -> Self {
        let mut filter_map: HashMap<String, AdmAdvertiserFilterSettings> = HashMap::new();
        for (adv, setting) in AdmSettings::from(settings) {
            dbg!("Processing records for", &adv.to_lowercase());
            // map the settings to the URL we're going to be checking
            filter_map.insert(adv.to_lowercase(), setting);
        }
        Ok(AdmFilter {
            filter_set: filter_map,
        })
    }
}

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

#[allow(clippy::clippy::too_many_arguments)]
pub async fn get_tiles(
    reqwest_client: &reqwest::Client,
    adm_endpoint_url: &str,
    location: &LocationResult,
    stripped_ua: &str,
    placement: &str,
    state: &ServerState,
    tags: &mut Tags,
    headers: Option<&HeaderMap>,
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
            ("ua", &stripped_ua),      // TODO: remove once ADM API finalized
            ("country-code", &location.country()),
            ("region-code", &location.region()),
            // ("dma-code", location.dma),
            // ("form-factor", form_factor),
            ("os-family", stripped_ua),
            ("sub2", placement),
            ("v", "1.0"),
            // XXX: some value for results seems required, it defaults to 0
            // when omitted (despite AdM claiming it would default to 1)
            ("results", &settings.adm_query_tile_count.to_string()),
        ],
    )
    .map_err(|e| HandlerError::internal(&e.to_string()))?;
    let adm_url = adm_url.as_str();

    trace!("get_tiles GET {}", adm_url);
    let mut response: AdmTileResponse = if state.settings.test_mode {
        // we can be a bit unforgiving here because we want to absolutely block bad things.
        let default = HeaderValue::from_str("default").unwrap();
        let test_response = headers
            .unwrap_or(&HeaderMap::new())
            .get("fake-response")
            .unwrap_or(&default)
            .to_str()
            .unwrap()
            .to_owned();
        dbg!("Getting fake response:", &test_response);
        AdmTileResponse::fake_response(&state.settings, test_response)?
    } else {
        reqwest_client
            .get(adm_url)
            .header(reqwest::header::USER_AGENT, stripped_ua)
            .send()
            .await
            .map_err(|e| {
                // ADM servers are down, or improperly configured
                HandlerErrorKind::BadAdmResponse(format!("ADM Server Error: {:?}", e))
            })?
            .error_for_status()
            .map_err(|e| {
                dbg!(&e);
                HandlerErrorKind::BadAdmResponse(format!("ADM provided invalid response: {:?}", e))
            })?
            .json()
            .await
            .map_err(|e| {
                // ADM servers are not returning correct information
                HandlerErrorKind::BadAdmResponse(format!("ADM provided invalid response: {:?}", e))
            })?
    };
    response.tiles = response
        .tiles
        .into_iter()
        .filter_map(|tile| state.filter.filter_and_process(tile, tags))
        .take(settings.adm_max_tiles as usize)
        .collect();
    Ok(response)
}
