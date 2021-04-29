use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use url::Url;

use crate::error::{HandlerError, HandlerErrorKind, HandlerResult};
use crate::server::ServerState;
use crate::settings::Settings;
//use crate::server::img_storage;

pub(crate) const DEFAULT: &str = "DEFAULT";

#[derive(Debug, Deserialize, Serialize)]
pub struct AdmTileResponse {
    pub tiles: Vec<AdmTile>,
}

/// Filter criteria for adm Tiles
#[derive(Default, Clone, Debug)]
pub struct AdmFilter {
    /// list of allowed base host strings.
    pub filter_set: BTreeMap<String, AdmAdvertiserFilterSettings>,
}

/// The AdmAdvertiserFilterSettings contain the settings for the various
/// ADM provided partners. These are specified as a JSON formatted hash
/// that contains the components. A special "DEFAULT" setting provides
/// information that may be used as a DEFAULT, or commonly appearing set
/// of data.
/// See `impl From<Settings>` for details of the structure.
#[derive(Clone, Debug, Deserialize, Default)]
pub struct AdmAdvertiserFilterSettings {
    pub(crate) advertiser_hosts: Vec<String>,
    pub(crate) impression_hosts: Vec<String>,
    pub(crate) position: Option<u8>,
    pub(crate) include_regions: Vec<String>,
}

pub(crate) type AdmSettings = HashMap<String, AdmAdvertiserFilterSettings>;

impl AdmFilter {
    /// Check the impression URL to see if it's valid.
    ///
    /// This extends `filter_and_process`
    fn check_impression(
        &self,
        filter: &AdmAdvertiserFilterSettings,
        tile: &mut AdmTile,
    ) -> HandlerResult<()> {
        let parsed: Url = match tile.impression_url.parse() {
            Ok(v) => v,
            Err(e) => {
                return Err(HandlerErrorKind::UnexpectedImpressionHost(format!(
                    "Invalid host: {:?} {:?}",
                    e,
                    tile.impression_url.to_string()
                ))
                .into());
            }
        };
        let host = match parsed.host() {
            Some(v) => v.to_string(),
            None => {
                return Err(HandlerErrorKind::UnexpectedImpressionHost(format!(
                    "Missing impression host: {:?}",
                    tile.impression_url
                ))
                .into());
            }
        };
        if !filter.impression_hosts.contains(&host) {
            return Err(HandlerErrorKind::UnexpectedImpressionHost(host).into());
        }
        Ok(())
    }

    /// Filter and process tiles from ADM:
    ///
    /// - Returns None for tiles that shouldn't be shown to the client
    /// - Modifies tiles for output to the client (adding additional fields, etc.)
    pub fn filter_and_process(&self, tile: AdmTile) -> Option<AdmTile> {
        let parsed: Url = match tile.advertiser_url.parse() {
            Ok(v) => v,
            Err(e) => {
                error!(
                    "{:?}",
                    HandlerErrorKind::UnexpectedSiteHost(format!(
                        "Invalid host: {:?} {:?}",
                        e,
                        tile.advertiser_url.to_string()
                    ))
                );
                return None;
            }
        };
        let host = match parsed.host() {
            Some(v) => v.to_string(),
            None => {
                error!(
                    "{:?}",
                    HandlerErrorKind::Validation(format!(
                        "Missing host from advertiser URL: {:?}",
                        parsed
                    ))
                );
                return None;
            }
        };
        // Use strict matching for now, eventually, we may want to use backwards expanding domain
        // searches, (.e.g "xyz.example.com" would match "example.com")
        let mut result = tile;
        match self.filter_set.get(&host) {
            Some(filter) => {
                // Apply any additional tile filtering here.
                let none = AdmAdvertiserFilterSettings::default();
                let default = self.filter_set.get(DEFAULT).unwrap_or(&none);
                let impression_filter = if filter.impression_hosts.is_empty() {
                    default
                } else {
                    filter
                };
                match self.check_impression(impression_filter, &mut result) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("{:?}", e);
                        return None;
                    }
                }
                result.position = if filter.position.is_none() {
                    default.position
                } else {
                    filter.position
                };
                Some(result)
            }
            None => {
                error!("{:?}", HandlerErrorKind::UnexpectedSiteHost(host));
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
///     "positions": [1, 2],
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
        let mut filter_map: BTreeMap<String, AdmAdvertiserFilterSettings> = BTreeMap::new();
        for (adv, setting) in settings.adm_settings.clone() {
            dbg!("Processing records for {:?}", adv);
            // map the settings to the URL we're going to be checking
            let mut d_settings = setting.clone();
            // we already have this info, no need to duplicate it.
            d_settings.advertiser_hosts = vec![];
            for url in setting.advertiser_hosts {
                // TODO: maybe use a reference for this data instead of cloning?
                filter_map.insert(url, d_settings.clone());
            }
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

pub async fn get_tiles(
    reqwest_client: &reqwest::Client,
    adm_endpoint_url: &str,
    location: (String, String), // TODO: in lieu of Location
    stripped_ua: &str,
    placement: &str,
    state: &ServerState,
) -> Result<AdmTileResponse, HandlerError> {
    // XXX: Assumes adm_endpoint_url includes
    // ?partner=<mozilla_partner_name>&sub1=<mozilla_tag_id> (probably should
    // validate this on startup)
    // let user_loc =
    let settings = state.settings.clone();
    let adm_url = Url::parse_with_params(
        adm_endpoint_url,
        &[
            ("partner", settings.partner_id.as_str()),
            ("sub1", settings.sub1.as_str()),
            // ("sub2", placement),
            ("country-code", location.0.as_str()),
            ("region-code", location.1.as_str()),
            // ("dma-code", location.dma),
            // ("form-factor", form_factor),
            ("os-family", stripped_ua),
            ("sub2", placement),
            ("v", "1.0"),
            // XXX: some value for results seems required, it defaults to 0
            // when omitted (despite AdM claiming it would default to 1)
            ("results", "10"),
        ],
    )
    .map_err(|e| HandlerError::internal(&e.to_string()))?;
    let adm_url = adm_url.as_str();

    trace!("get_tiles GET {}", adm_url);
    // XXX: handle empty responses -- AdM sends empty json in that case
    // 'Error("missing field `tiles`"'
    let mut response: AdmTileResponse = reqwest_client
        .get(adm_url)
        .header(reqwest::header::USER_AGENT, stripped_ua)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    // TODO: Is there a better way to raise the error possibly inside of `filter_and_process`?
    response.tiles = response
        .tiles
        .into_iter()
        .filter_map(|tile| state.filter.filter_and_process(tile))
        .collect();
    Ok(response)
}
