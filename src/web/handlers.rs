//! API Handlers
use actix_web::{web, HttpRequest, HttpResponse};
use rand::{thread_rng, Rng};

use crate::{
    adm,
    error::{HandlerError, HandlerErrorKind},
    metrics::Metrics,
    server::{
        cache::{self, Tiles, TilesState},
        location::LocationResult,
        ServerState,
    },
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

    if !state
        .filter
        .all_include_regions
        .contains(&location.country())
    {
        trace!("get_tiles: country not included: {:?}", location.country());
        // Nothing to serve
        return Ok(HttpResponse::NoContent().finish());
    }

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
        // First make a cheap read from the cache
        if let Some(tiles_state) = state.tiles_cache.get(&audience_key) {
            match &*tiles_state {
                TilesState::Populating => {
                    // Another task is currently populating this entry and will
                    // complete shortly. 503 until then instead of queueing
                    // more redundant requests
                    trace!("get_tiles: Another task Populating");
                    metrics.incr("tiles_cache.miss.populating");
                    return Ok(HttpResponse::ServiceUnavailable().finish());
                }
                TilesState::Fresh { tiles } => {
                    expired = tiles.expired();
                    if !expired {
                        trace!("get_tiles: cache hit: {:?}", audience_key);
                        metrics.incr("tiles_cache.hit");
                        return Ok(content_response(&tiles.content));
                    }
                    // Needs refreshing
                }
                TilesState::Refreshing { tiles } => {
                    // Another task is currently refreshing this entry, just
                    // return the stale Tiles until it's completed
                    trace!(
                        "get_tiles: cache hit (expired, Refreshing): {:?}",
                        audience_key
                    );
                    metrics.incr("tiles_cache.hit.refreshing");
                    return Ok(content_response(&tiles.content));
                }
            }
        }
    }

    // Alter the cache separately from the read above: writes are more
    // expensive and these alterations occur infrequently
    if expired {
        // The cache entry's expired and we're about to refresh it
        trace!("get_tiles: Fresh now expired, Refreshing");
        state
            .tiles_cache
            .alter(&audience_key, |_, tiles_state| match tiles_state {
                TilesState::Fresh { tiles } if tiles.expired() => TilesState::Refreshing { tiles },
                _ => tiles_state,
            });
    } else {
        // We'll populate this cache entry for probably the first time
        trace!("get_tiles: Populating");
        state
            .tiles_cache
            .insert(audience_key.clone(), TilesState::Populating);
    };

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

    let handle_result = || {
        match result {
            Ok(response) => {
                let tiles = cache::Tiles::new(response, add_jitter(&state.settings))?;
                trace!(
                    "get_tiles: cache miss{}: {:?}",
                    if expired { " (expired)" } else { "" },
                    &audience_key
                );
                metrics.incr("tiles_cache.miss");
                state.tiles_cache.insert(
                    audience_key.clone(),
                    TilesState::Fresh {
                        tiles: tiles.clone(),
                    },
                );
                Ok(content_response(&tiles.content))
            }
            Err(e) => {
                // Add some kind of stats to Retrieving or RetrievingFirst?
                // do we need a kill switch if we're restricting like this already?
                match e.kind() {
                    HandlerErrorKind::BadAdmResponse(es) => {
                        warn!("Bad response from ADM: {:?}", e);
                        metrics.incr_with_tags("tiles.invalid", Some(&tags));
                        state.tiles_cache.insert(
                            audience_key.clone(),
                            TilesState::Fresh {
                                tiles: Tiles::empty(add_jitter(&state.settings)),
                            },
                        );
                        // Report directly to sentry
                        // (This is starting to become a pattern. 🤔)
                        let mut tags = Tags::from_head(request.head(), settings);
                        tags.add_extra("err", es);
                        tags.add_tag("level", "warning");
                        l_sentry::report(&tags, sentry::event_from_error(&e));
                        warn!("ADM Server error: {:?}", e);
                        Ok(HttpResponse::NoContent().finish())
                    }
                    _ => Err(e),
                }
            }
        }
    };

    let result = handle_result();
    // Cleanup the TilesState on errors
    // TODO: potential panics are not currently cleaned up
    if result.is_err() {
        if expired {
            // Back to Fresh (though the tiles are expired): so a later request
            // will retry refreshing again
            state
                .tiles_cache
                .alter(&audience_key, |_, tiles_state| match tiles_state {
                    TilesState::Refreshing { tiles } => TilesState::Fresh { tiles },
                    _ => tiles_state,
                });
        } else {
            // Clear the entry: a later request will retry populating again
            state
                .tiles_cache
                .remove_if(&audience_key, |_, tiles_state| {
                    matches!(tiles_state, TilesState::Populating)
                });
        }
    }
    result
}

fn content_response(content: &cache::TilesContent) -> HttpResponse {
    match content {
        cache::TilesContent::Json(json) => HttpResponse::Ok()
            .content_type("application/json")
            .body(json),
        cache::TilesContent::Empty => HttpResponse::NoContent().finish(),
    }
}
