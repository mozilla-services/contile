//! API Handlers
use actix_web::{web, HttpRequest, HttpResponse};
use serde_json::json;

use super::user_agent;
use crate::{
    adm,
    error::{HandlerError, HandlerErrorKind},
    metrics::Metrics,
    server::{cache, location::LocationResult, ServerState},
    tags::Tags,
    web::{extractors::TilesRequest, middleware::sentry as l_sentry},
};

pub async fn get_tiles(
    treq: TilesRequest,
    metrics: Metrics,
    state: web::Data<ServerState>,
    request: HttpRequest,
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
    let (os_family, form_factor) = user_agent::get_device_info(&treq.ua);

    let header = request.head();
    let c_info = request.connection_info();
    let ip_addr_str = c_info.remote_addr().unwrap_or(fake_ip);
    let mut location = if state.mmdb.is_available() {
        let addr = match ip_addr_str.parse() {
            Ok(v) => v,
            Err(e) => {
                return Err(HandlerErrorKind::General(format!("Invalid remote IP {:?}", e)).into());
            }
        };
        state
            .mmdb
            .mmdb_locate(addr, &["en".to_owned()])
            .await
            .unwrap_or_else(|_| Some(LocationResult::from(header)))
    } else {
        Some(LocationResult::from(header))
    }
    .unwrap_or_default();
    location.fake_ip = fake_ip.to_owned(); //TODO: remove once final ADM API is live.

    let mut tags = Tags::default();
    {
        tags.add_extra("country", &location.country());
        tags.add_extra("region", &location.region());
        tags.add_extra("ip", ip_addr_str);
        tags.add_extra("ua", &stripped_ua);
        // Add/modify the existing request tags.
        // tags.clone().commit(&mut request.extensions_mut());
    }

    let audience_key = cache::AudienceKey {
        country: location.country(),
        region: location.region(),
        platform: stripped_ua.clone(),
        os_family,
        form_factor,
    };
    if let Some(tiles) = state.tiles_cache.read().await.get(&audience_key) {
        trace!("get_tiles: cache hit: {:?}", audience_key);
        metrics.incr("tiles_cache.hit");
        return Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(&tiles.json));
    }

    let tiles = match adm::get_tiles(
        &state.reqwest_client,
        &state.adm_endpoint_url,
        &location,
        &stripped_ua,
        os_family,
        form_factor,
        &state,
        &mut tags,
    )
    .await
    {
        Ok(response) => {
            // adM sometimes returns an invalid response. We don't want to cache that.
            let tiles = serde_json::to_string(&response).map_err(|e| {
                HandlerError::internal(&format!("Response failed to serialize: {}", e))
            })?;
            trace!("get_tiles: cache miss: {:?}", audience_key);
            metrics.incr("tiles_cache.miss");
            let mut hash_val = 0;
            for tile in &response.tiles {
                // adding would overflow so do a xor instead.
                hash_val ^= tile.hash()
            }
            state.tiles_cache.write().await.insert(
                audience_key,
                cache::Tiles {
                    json: tiles.clone(),
                    hash: hash_val,
                },
            );
            tiles
        }
        Err(e) => match e.kind() {
            HandlerErrorKind::BadAdmResponse(es) => {
                warn!("Bad response from ADM: {:?}", e);
                // Report directly to sentry
                // (This is starting to become a pattern. 🤔)
                let mut tags = Tags::from(request.head());
                tags.add_extra("err", es);
                tags.add_tag("level", "warning");
                l_sentry::report(&tags, sentry::event_from_error(&e));
                //TODO: probably should do: json!(vec![adm::AdmTile::default()]).to_string()
                warn!("ADM Server error: {:?}", e);
                json!({"tiles":[]}).to_string()
            }
            _ => return Err(e),
        },
    };

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(tiles))
}
