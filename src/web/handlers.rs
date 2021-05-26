//! API Handlers
use actix_web::{web, HttpRequest, HttpResponse};

use super::user_agent;
use crate::{
    adm,
    error::{HandlerError, HandlerErrorKind},
    metrics::Metrics,
    server::{cache, location::LocationResult, ServerState},
    tags::Tags,
    web::{extractors::TilesRequest, middleware::sentry as l_sentry},
};

/// Handler for `.../v1/tiles` endpoint
///
/// Normalizes User Agent info and searches cache for possible tile suggestions.
/// On a miss, it will attempt to fetch new tiles from ADM.
pub async fn get_tiles(
    treq: TilesRequest,
    metrics: Metrics,
    state: web::Data<ServerState>,
    request: HttpRequest,
) -> Result<HttpResponse, HandlerError> {
    trace!("get_tiles");

    let cinfo = request.connection_info();
    let ip_addr_str = cinfo.remote_addr().unwrap_or({
        let default = state
            .adm_country_ip_map
            .get("US")
            .expect("Invalid ADM_COUNTRY_IP_MAP settting");
        if let Some(country) = &treq.country {
            state.adm_country_ip_map.get(country).unwrap_or(default)
        } else {
            default
        }
    });
    let (os_family, form_factor) = user_agent::get_device_info(&treq.ua);

    let header = request.head();
    let location = if state.mmdb.is_available() {
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
            .unwrap_or_else(|_| Some(LocationResult::from_header(header, &state.settings)))
    } else {
        Some(LocationResult::from_header(header, &state.settings))
    }
    .unwrap_or_default();

    let mut tags = Tags::default();
    {
        tags.add_extra("country", &location.country());
        tags.add_extra("region", &location.region());
        tags.add_extra("ip", ip_addr_str);
        // Add/modify the existing request tags.
        // tags.clone().commit(&mut request.extensions_mut());
    }

    let audience_key = cache::AudienceKey {
        country_code: location.country(),
        region_code: location.region(),
        form_factor,
        os_family,
    };
    if !state.settings.test_mode {
        if let Some(tiles) = state.tiles_cache.read().await.get(&audience_key) {
            trace!("get_tiles: cache hit: {:?}", audience_key);
            metrics.incr("tiles_cache.hit");
            return Ok(HttpResponse::Ok()
                .content_type("application/json")
                .body(&tiles.json));
        }
    }
    let tiles = match adm::get_tiles(
        &state.reqwest_client,
        &state.adm_endpoint_url,
        &location,
        os_family,
        form_factor,
        &state,
        &mut tags,
        // be aggressive about not passing headers unless we absolutely need to
        if state.settings.test_mode {
            Some(request.head().headers())
        } else {
            None
        },
    )
    .await
    {
        Ok(response) => {
            // adM sometimes returns an invalid response. We don't want to cache that.
            if response.tiles.is_empty() {
                return Ok(HttpResponse::NoContent().finish());
            };
            let tiles = serde_json::to_string(&response).map_err(|e| {
                HandlerError::internal(&format!("Response failed to serialize: {}", e))
            })?;
            trace!("get_tiles: cache miss: {:?}", audience_key);
            metrics.incr("tiles_cache.miss");
            state.tiles_cache.write().await.insert(
                audience_key,
                cache::Tiles {
                    json: tiles.clone(),
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
                tags.add_extra("err", &es);
                tags.add_tag("level", "warning");
                l_sentry::report(&tags, sentry::event_from_error(&e));
                //TODO: probably should do: json!(vec![adm::AdmTile::default()]).to_string()
                warn!("ADM Server error: {:?}", e);
                return Ok(HttpResponse::NoContent().finish());
            }
            _ => return Err(e),
        },
    };

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(tiles))
}
