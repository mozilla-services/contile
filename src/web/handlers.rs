//! API Handlers
use actix_web::{web, HttpRequest, HttpResponse};
use rand::{thread_rng, Rng};

use crate::{
    adm,
    error::{HandlerError, HandlerErrorKind},
    metrics::Metrics,
    server::{cache, location::LocationResult, ServerState},
    settings::Settings,
    tags::Tags,
    web::{middleware::sentry as l_sentry, DeviceInfo},
};

/// Calculate the ttl from the settings by taking the tiles_ttl
/// and calculating a jitter that is no more than 50% of the total TTL.
/// It is recommended that "jitter" be 10%.
pub fn add_jitter(settings: &Settings) -> u32 {
    let mut rng = thread_rng();
    let ftl = settings.tiles_ttl as f32;
    let offset = ftl * (std::cmp::min(settings.jitter, 50) as f32 * 0.01);
    let jit = rng.gen_range(0.0 - offset..offset);
    (ftl + jit) as u32
}

/// Handler for `.../v1/tiles` endpoint
///
/// Normalizes User Agent info and searches cache for possible tile suggestions.
/// On a miss, it will attempt to fetch new tiles from ADM.
pub async fn get_tiles(
    location: LocationResult,
    device_info: DeviceInfo,
    metrics: Metrics,
    state: web::Data<ServerState>,
    request: HttpRequest,
) -> Result<HttpResponse, HandlerError> {
    trace!("get_tiles");
    metrics.incr("tiles.get");

    let settings = &state.settings;
    let mut tags = Tags::default();
    {
        tags.add_extra("country", &location.country());
        tags.add_extra("region", &location.region());
        // Add/modify the existing request tags.
        // tags.clone().commit(&mut request.extensions_mut());
    }

    let audience_key = cache::AudienceKey {
        country_code: location.country(),
        region_code: location.region(),
        form_factor: device_info.form_factor,
    };
    let mut expired = false;
    if !settings.test_mode {
        if let Some(tiles) = state.tiles_cache.get(&audience_key) {
            expired = tiles.expired();
            if !expired {
                trace!("get_tiles: cache hit: {:?}", audience_key);
                metrics.incr("tiles_cache.hit");
                return Ok(content_response(&tiles.content, &metrics));
            }
        }
    }

    let result = adm::get_tiles(
        &state,
        &location,
        device_info,
        &mut tags,
        &metrics,
        // be aggressive about not passing headers unless we absolutely need to
        if settings.test_mode {
            Some(request.head().headers())
        } else {
            None
        },
    )
    .await;

    match result {
        Ok(response) => {
            let tiles = cache::Tiles::new(response, add_jitter(&state.settings))?;
            trace!(
                "get_tiles: cache miss{}: {:?}",
                if expired { " (expired)" } else { "" },
                &audience_key
            );
            metrics.incr("tiles_cache.miss");
            state.tiles_cache.insert(audience_key, tiles.clone());
            Ok(content_response(&tiles.content, &metrics))
        }
        Err(e) => {
            match e.kind() {
                HandlerErrorKind::BadAdmResponse(es) => {
                    warn!("Bad response from ADM: {:?}", e);
                    metrics.incr_with_tags("tiles.invalid", Some(&tags));
                    // Report directly to sentry
                    // (This is starting to become a pattern. 🤔)
                    let mut tags = Tags::from_head(request.head(), &settings);
                    tags.add_extra("err", &es);
                    tags.add_tag("level", "warning");
                    l_sentry::report(&tags, sentry::event_from_error(&e));
                    warn!("ADM Server error: {:?}", e);
                    Ok(HttpResponse::NoContent().finish())
                }
                _ => Err(e),
            }
        }
    }
}

fn content_response(content: &cache::TilesContent, metrics: &Metrics) -> HttpResponse {
    match content {
        cache::TilesContent::Json(json) => HttpResponse::Ok()
            .content_type("application/json")
            .body(json),
        cache::TilesContent::Empty => {
            metrics.incr("tiles.empty");
            HttpResponse::NoContent().finish()
        }
    }
}
