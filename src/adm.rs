use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::HandlerError;

#[derive(Debug, Deserialize, Serialize)]
pub struct TileResponse<T> {
    pub tiles: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub struct AdmTile {
    pub id: u64,
    pub name: String,
    pub advertiser_url: String,
    pub click_url: String,
    pub image_url: String,
    pub impression_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MozTile {
    pub id: u64,
    pub name: String,
    pub advertiser_url: String,
    pub click_url: String,
    pub image_url: String,
    pub impression_url: String,

    /// Index this tile is shown at in the browser
    // Option<u16>?
    pub sponsored_position: u16,
}

pub async fn get_tiles(
    reqwest_client: &reqwest::Client,
    adm_endpoint_url: &str,
    fake_ip: &str,
    stripped_ua: &str,
    placement: &str,
    count: u16,
) -> Result<TileResponse<MozTile>, HandlerError> {
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
            ("results", &count.to_string()),
        ],
    )
    .map_err(|e| HandlerError::internal(&e.to_string()))?;
    let adm_url = adm_url.as_str();

    trace!("get_tiles GET {}", adm_url);
    // XXX: handle empty responses -- AdM sends empty json in that case
    // 'Error("missing field `tiles`"'
    let response: TileResponse<AdmTile> = reqwest_client
        .get(adm_url)
        .header(reqwest::header::USER_AGENT, stripped_ua)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let tiles = response
        .tiles
        .into_iter()
        .filter_map(filter_and_process)
        .collect();
    Ok(TileResponse { tiles })
}

/// Filter and process tiles from ADM:
///
/// - Returns None for tiles that shouldn't be shown to the client
/// - Modifies tiles for output to the client (adding additional fields, etc.)
#[allow(clippy::unnecessary_wraps)]
fn filter_and_process(tile: AdmTile) -> Option<MozTile> {
    //if !state.valid_tile(tile.name) {
    //    return None;
    //}

    // TODO: move images to CDN
    let tile = MozTile {
        id: tile.id,
        name: tile.name,
        advertiser_url: tile.advertiser_url,
        click_url: tile.click_url,
        image_url: tile.image_url,
        impression_url: tile.impression_url,

        sponsored_position: 0,
    };
    Some(tile)
}
