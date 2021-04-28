use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use url::{Host, Url};

use crate::error::{HandlerError, HandlerResult};
use crate::server::ServerState;
use crate::settings::Settings;
//use crate::server::img_storage;

#[derive(Debug, Deserialize, Serialize)]
pub struct AdmTileResponse {
    pub tiles: Vec<AdmTile>,
}

/// Filter criteria for adm Tiles
#[derive(Default, Clone, Debug)]
pub struct AdmFilter {
    /// list of allowed base host strings.
    pub filter_map: BTreeMap<String, AdmAdvertiserFilterSettings>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AdmAdvertiserFilterSettings {
    // valid
    pub(crate) advertiser_url: Vec<String>,
    pub(crate) position: Option<u8>,
    pub(crate) include_regions: Vec<String>,
}

pub(crate) type AdmSettings = HashMap<String, AdmAdvertiserFilterSettings>;

impl AdmFilter {
    /// Filter and process tiles from ADM:
    ///
    /// - Returns None for tiles that shouldn't be shown to the client
    /// - Modifies tiles for output to the client (adding additional fields, etc.)
    pub fn filter_and_process(&self, tile: AdmTile) -> Option<AdmTile> {
        let parsed: Url = match tile.advertiser_url.parse() {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    "Could not parse advertiser URL {:?} : {:?}",
                    tile.advertiser_url, e
                );
                return None;
            }
        };
        let host = parsed.host().unwrap_or_else(|| {
            error!("Could not get host from parsed url: {:?}", parsed);
            Host::Domain("UNKNOWN")
        });
        // Use strict matching for now, eventually, we may want to use backwards expanding domain
        // searches, (.e.g "xyz.example.com" would match "example.com")
        match self.filter_map.get(&host.to_string()) {
            Some(filter) => {
                // Apply any additional tile filtering here.
                let mut result = tile;
                result.position = filter.position;
                dbg!("👍🏻", host);
                Some(result)
            }
            None => {
                dbg!("👎🏻", host);
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
///     "advertiser_url": ["www.example.org", "example.org"],
///     /* Valid tile positions for this advertiser (empty for "all") */
///     "positions": [1, 2],
///     /* Valid target regions for this advertiser
///        (use "en-US" for "all in english speaking United States") */
///     "include_regions": ["en-US/TX", "en-US/CA"],
///     },
///     ...
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
            d_settings.advertiser_url = vec![];
            for url in setting.advertiser_url {
                // TODO: maybe use a reference for this data instead of cloning?
                filter_map.insert(url, d_settings.clone());
            }
        }
        Ok(AdmFilter { filter_map })
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
    response.tiles = response
        .tiles
        .into_iter()
        .filter_map(|tile| state.filter.filter_and_process(tile))
        .collect();
    Ok(response)
}
