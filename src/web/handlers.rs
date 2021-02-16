//! API Handlers
use std::collections::HashMap;

use actix_web::{web, Error, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use super::user_agent;
use crate::{error::HandlerError, server::ServerState, web::extractors::TilesRequest};

#[derive(Debug, Deserialize, Serialize)]
struct AdmTileResponse {
    tiles: Vec<AdmTile>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AdmTile {
    id: u64,
    name: String,
    advertiser_url: String,
    click_url: String,
    image_url: String,
    impression_url: String,
}

pub async fn get_tiles(
    treq: TilesRequest,
    state: web::Data<ServerState>,
) -> Result<HttpResponse, HandlerError> {
    trace!("get_tiles");

    let fake_ip = if let Some(ip) = state.adm_country_ip_map.get(&treq.country) {
        ip
    } else {
        state
            .adm_country_ip_map
            .get("US")
            .expect("Invalid ADM_COUNTRY_IP_MAP setting")
    };

    let stripped_ua = user_agent::strip_ua(&treq.ua);
    // XXX: Assumes adm_endpoint_url includes
    // ?partner=<mozilla_partner_name>&sub1=<mozilla_tag_id> (probably should
    // validate this on startup)
    let adm_url = Url::parse_with_params(
        &state.adm_endpoint_url,
        &[
            ("ip", fake_ip.as_str()),
            ("ua", &stripped_ua),
            ("sub2", &treq.placement),
            ("v", "1.0"),
        ],
    )
    .map_err(|e| HandlerError::internal(&e.to_string()))?;
    let adm_url = adm_url.as_str();

    trace!("get_tiles GET {}", adm_url);
    let mut response: AdmTileResponse = state
        .reqwest_client
        .get(adm_url)
        .header(reqwest::header::USER_AGENT, &stripped_ua)
        .send()
        .await?
        .json()
        .await?;
    response.tiles = response
        .tiles
        .into_iter()
        .filter_map(filter_and_process)
        .collect();

    Ok(HttpResponse::Ok().json(response))
}

/// Filter and process tiles from ADM:
///
/// - Returns None for tiles that shouldn't be shown to the client
/// - Modifies tiles for output to the client (adding additional fields, etc.)
#[allow(unused_mut)]
#[allow(clippy::clippy::unnecessary_wraps)]
fn filter_and_process(mut tile: AdmTile) -> Option<AdmTile> {
    //if !state.valid_tile(tile.name) {
    //    return None;
    //}

    // TODO: move images to CDN
    Some(tile)
}

/// Returns a status message indicating the state of the current server
pub async fn heartbeat() -> Result<HttpResponse, Error> {
    let mut checklist = HashMap::new();
    checklist.insert(
        "version".to_owned(),
        Value::String(env!("CARGO_PKG_VERSION").to_owned()),
    );
    Ok(HttpResponse::Ok().json(checklist))
}

/// try returning an API error
pub async fn test_error() -> Result<HttpResponse, HandlerError> {
    // generate an error for sentry.
    error!("Test Error");
    Err(HandlerError::internal("Oh Noes!"))
}
