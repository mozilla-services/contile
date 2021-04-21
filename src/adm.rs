use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use url::Url;

use crate::error::HandlerError;
use crate::settings::Settings;

#[derive(Debug, Deserialize, Serialize)]
pub struct AdmTileResponse {
    pub tiles: Vec<AdmTile>,
}

/// Filter criteria for adm Tiles
#[derive(Default, Clone, Debug)]
pub struct AdmFilter {
    /// list of allowed base host strings.
    pub allowed_url_hosts: BTreeSet<String>,
    pub field_defaults: HashMap<String, String>,
}

impl AdmFilter {
    /// Filter and process tiles from ADM:
    ///
    /// - Returns None for tiles that shouldn't be shown to the client
    /// - Modifies tiles for output to the client (adding additional fields, etc.)
    pub fn filter_and_process(&self, tile: AdmTile) -> Option<AdmTile> {
        let host: Url = match tile.advertiser_url.parse() {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    "Could not parse advertiser URL {:?} : {:?}",
                    tile.advertiser_url, e
                );
                return None;
            }
        };
        // Use strict matching for now, eventually, we may want to use backwards expanding domain
        // searches, (.e.g "xyz.example.com" would match "example.com")
        if self
            .allowed_url_hosts
            .contains(host.host_str().unwrap_or("UNKNOWN"))
        {
            return Some(tile);
        }
        None
    }
}

/// Construct the AdmFilter from the provided settings.
/// This uses `allowed_vendors` (a JSON formatted list of strings),
///
impl From<&Settings> for AdmFilter {
    fn from(settings: &Settings) -> Self {
        let mut allowed_url_hosts: BTreeSet<String> = BTreeSet::new();
        if let Some(hosts) = settings.clone().allowed_vendors {
            for host in hosts {
                allowed_url_hosts.insert(host);
            }
        };

        AdmFilter {
            allowed_url_hosts,
            ..Default::default()
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AdmTile {
    pub id: u64,
    pub name: String,
    pub advertiser_url: String,
    pub click_url: String,
    pub image_url: String,
    pub impression_url: String,
}

pub async fn get_tiles(
    reqwest_client: &reqwest::Client,
    adm_endpoint_url: &str,
    fake_ip: &str,
    stripped_ua: &str,
    placement: &str,
    filters: &AdmFilter,
) -> Result<AdmTileResponse, HandlerError> {
    // XXX: Assumes adm_endpoint_url includes
    // ?partner=<mozilla_partner_name>&sub1=<mozilla_tag_id> (probably should
    // validate this on startup)
    let adm_url = Url::parse_with_params(
        adm_endpoint_url,
        &[
            ("ip", fake_ip),
            ("ua", &stripped_ua),
            ("sub2", &placement),
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
        .filter_map(|tile| filters.filter_and_process(tile))
        .collect();
    Ok(response)
}
