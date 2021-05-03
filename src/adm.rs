use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{HandlerError, HandlerErrorKind};

#[derive(Debug, Deserialize, Serialize)]
pub struct AdmTileResponse {
    pub tiles: Vec<AdmTile>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
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
    let mut response: AdmTileResponse = reqwest_client
        .get(adm_url)
        .header(reqwest::header::USER_AGENT, stripped_ua)
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
        .filter_map(filter_and_process)
        .collect();
    Ok(response)
}

/// Filter and process tiles from ADM:
///
/// - Returns None for tiles that shouldn't be shown to the client
/// - Modifies tiles for output to the client (adding additional fields, etc.)
#[allow(clippy::unnecessary_wraps, unused_mut)]
fn filter_and_process(mut tile: AdmTile) -> Option<AdmTile> {
    //if !state.valid_tile(tile.name) {
    //    return None;
    //}

    // TODO: move images to CDN
    Some(tile)
}
