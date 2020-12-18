//! API Handlers
use std::collections::HashMap;

use actix_web::{web, Error, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

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
            .ok_or_else(|| HandlerError::internal("Invalid ADM_COUNTRY_IP_MAP"))?
    };

    let ua = strip_ua(&treq.ua);
    // XXX: Assumes adm_endpoint_url includes
    // ?partner=<mozilla_partner_name>&sub1=<mozilla_tag_id> (probably should
    // validate this on startup)
    let adm_url = Url::parse_with_params(
        &state.adm_endpoint_url,
        &[
            ("ip", fake_ip.as_str()),
            ("ua", &ua),
            ("sub2", &treq.placement),
            ("v", "1.0"),
        ],
    )
    .map_err(|e| HandlerError::internal(&e.to_string()))?;
    let adm_url = adm_url.as_str();

    trace!("get_tiles GET {}", adm_url);
    let mut tresponse: AdmTileResponse = state
        .reqwest_client
        .get(adm_url)
        .header(reqwest::header::USER_AGENT, &ua)
        .send()
        .await?
        .json()
        .await?;
    tresponse.tiles = tresponse
        .tiles
        .into_iter()
        .filter_map(filter_and_process)
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok().json(tresponse))
}

/// Strip a Firefox User-Agent string, returning a version only varying in Base
/// OS (e.g. Mac, Windows, Linux) and Firefox major version number
fn strip_ua(ua: &str) -> String {
    // XXX:
    ua.to_owned()
}

/// Filter and process tiles from ADM:
///
/// - Returns None for tiles that shouldn't be shown to the client
/// - Modifies tiles for output to the client (adding additional fields, etc.)
#[allow(unused_mut)]
fn filter_and_process(mut tile: AdmTile) -> Option<AdmTile> {
    //if !state.valid_tile(tile.name) {
    //    return None;
    //}

    // TODO: move images to CDN
    //tile.image_url = "https://fail.fail".to_owned();
    Some(tile)
}

/// Returns a status message indicating the state of the current server
pub async fn heartbeat() -> Result<HttpResponse, Error> {
    let mut checklist = HashMap::new();
    checklist.insert(
        "version".to_owned(),
        Value::String(env!("CARGO_PKG_VERSION").to_owned()),
    );

    // Add optional values to checklist
    // checklist.insert("quota".to_owned(), serde_json::to_value(hb.quota)?);

    /*
    // Perform whatever additional checks you prefer
    match db.check().await {
        Ok(result) => {
            if result {
                checklist.insert("database".to_owned(), Value::from("Ok"));
            } else {
                checklist.insert("database".to_owned(), Value::from("Err"));
                checklist.insert(
                    "database_msg".to_owned(),
                    Value::from("check failed without error"),
                );
            };
            let status = if result { "Ok" } else { "Err" };
            checklist.insert("status".to_owned(), Value::from(status));

        }
        Err(e) => {
            error!("Heartbeat error: {:?}", e);
            checklist.insert("status".to_owned(), Value::from("Err"));
            checklist.insert("database".to_owned(), Value::from("Unknown"));
            return Ok(HttpResponse::ServiceUnavailable().json(checklist))
        }
    }
    */

    Ok(HttpResponse::Ok().json(checklist))
}

/// try returning an API error
pub async fn test_error() -> Result<HttpResponse, HandlerError> {
    // generate an error for sentry.
    error!("Test Error");
    Err(HandlerError::internal("Oh Noes!"))
}
