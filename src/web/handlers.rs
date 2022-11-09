//! API Handlers
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_location::Location;
use lazy_static::lazy_static;

use crate::{
    adm,
    error::{HandlerErrorKind, HandlerResult},
    metrics::Metrics,
    server::{
        cache::{self, Tiles, TilesState},
        ServerState,
    },
    settings::Settings,
    tags::Tags,
    web::{middleware::sentry as l_sentry, DeviceInfo},
};

lazy_static! {
    pub static ref EMPTY_TILES: String =
        serde_json::to_string(&adm::TileResponse { tiles: vec![] })
            .expect("Couldn't serialize EMPTY_TILES");
}

/// Handler for `.../v1/tiles` endpoint
///
/// Normalizes User Agent info and searches cache for possible tile suggestions.
/// On a miss, it will attempt to fetch new tiles from ADM.
pub async fn get_tiles(
    location: Location,
    device_info: DeviceInfo,
    metrics: Metrics,
    state: web::Data<ServerState>,
    request: HttpRequest,
) -> HandlerResult<HttpResponse> {
    trace!("get_tiles");
    metrics.incr("tiles.get");

    let settings = &state.settings;
    if !state
        .partner_filter
        .read()
        .await
        .all_include_regions
        .contains(&location.country())
    {
        trace!("get_tiles: country not included: {:?}", location.country());
        // Nothing to serve. We typically send a 204 for empty tiles but
        // optionally send 200 to resolve
        // https://github.com/mozilla-services/contile/issues/284
        let response = if settings.excluded_countries_200 {
            HttpResponse::Ok()
                .content_type("application/json")
                .body(EMPTY_TILES.as_str())
        } else {
            HttpResponse::NoContent().finish()
        };
        return Ok(response);
    }
    let audience_key = cache::AudienceKey {
        country_code: location.country(),
        region_code: if location.region() != "" {
            Some(location.region())
        } else {
            None
        },
        dma_code: location.dma,
        form_factor: device_info.form_factor,
        os_family: device_info.os_family,
        legacy_only: device_info.legacy_only(),
    };

    let mut tags = Tags::from_head(request.head(), settings);
    {
        tags.add_extra("audience_key", &format!("{:#?}", audience_key));
        // Add/modify the existing request tags.
        // tags.clone().commit(&mut request.extensions_mut());
    }

    let mut expired = false;

    if settings.test_mode != crate::settings::TestModes::TestFakeResponse {
        // First make a cheap read from the cache
        if let Some(tiles_state) = state.tiles_cache.get(&audience_key) {
            match &*tiles_state {
                TilesState::Populating => {
                    // Another task is currently populating this entry and will
                    // complete shortly. 304 until then instead of queueing
                    // more redundant requests
                    trace!("get_tiles: Another task Populating");
                    metrics.incr("tiles_cache.miss.populating");
                    return Ok(HttpResponse::NotModified().finish());
                }
                TilesState::Fresh { tiles } => {
                    expired = tiles.expired();
                    if !expired {
                        trace!("get_tiles: cache hit: {:?}", audience_key);
                        metrics.incr("tiles_cache.hit");
                        return Ok(tiles.to_response(settings.cache_control_header));
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
                    // expired() and maybe fallback_expired()
                    return Ok(fallback_response(settings, tiles));
                }
            }
        }
    }

    // Alter the cache separately from the read above: writes are more
    // expensive and these alterations occur infrequently

    // Prepare to write: temporarily set the cache entry to
    // Refreshing/Populating until we've completed our write, notifying other
    // requests in flight during this time to return stale data/204 No Content
    // instead of making duplicate/redundant writes. The handle will reset the
    // temporary state if no write occurs (due to errors/panics)
    let handle = state.tiles_cache.prepare_write(&audience_key, expired);

    let result = adm::get_tiles(
        &state,
        &location,
        device_info,
        &mut tags,
        &metrics,
        // be aggressive about not passing headers unless we absolutely need to
        if settings.test_mode != crate::settings::TestModes::NoTest {
            Some(request.head().headers())
        } else {
            None
        },
    )
    .await;

    match result {
        Ok(response) => {
            let tiles = cache::Tiles::new(
                response,
                settings.tiles_ttl_with_jitter(),
                settings.tiles_fallback_ttl_with_jitter(),
                settings.excluded_countries_200,
            )?;
            trace!(
                "get_tiles: cache miss{}: {:?}",
                if expired { " (expired)" } else { "" },
                &audience_key
            );
            metrics.incr("tiles_cache.miss");
            handle.insert(TilesState::Fresh {
                tiles: tiles.clone(),
            });
            Ok(tiles.to_response(settings.cache_control_header))
        }
        Err(e) => {
            if matches!(e.kind(), HandlerErrorKind::BadAdmResponse(_)) {
                // Handle a bad response from ADM specially.
                // Report it to metrics and sentry, but also store an empty record
                // into the cache so that we don't stampede the ADM servers.
                warn!("Bad response from ADM: {:?}", e);
                // Merge in the error tags, which should already include the
                // error string as `error`
                tags.extend(e.tags.clone());
                tags.add_tag("level", "warning");
                metrics.incr_with_tags("tiles.invalid", Some(&tags));
                // write an empty tile set into the cache for this result.
                handle.insert(TilesState::Fresh {
                    tiles: Tiles::empty(
                        settings.tiles_ttl_with_jitter(),
                        settings.tiles_fallback_ttl_with_jitter(),
                        settings.excluded_countries_200,
                    ),
                });
                // Report the error directly to sentry
                l_sentry::report(&e, &tags);
                warn!("ADM Server error: {:?}", e);
                // Return a 204 to the client.
                return Ok(HttpResponse::NoContent().finish());
            }

            match e.kind() {
                HandlerErrorKind::Reqwest(e) if e.is_timeout() => tags.add_tag("reason", "timeout"),
                HandlerErrorKind::Reqwest(e) if e.is_connect() => tags.add_tag("reason", "connect"),
                _ => (),
            }
            if handle.fallback_tiles.is_some() {
                tags.add_tag("fallback", "true");
            }
            metrics.incr_with_tags("tiles.get.error", Some(&tags));

            // A general error occurred, try rendering fallback Tiles
            if let Some(tiles) = handle.fallback_tiles {
                return Ok(fallback_response(settings, &tiles));
            }
            Err(e)
        }
    }
}
/// Render stale (`expired`) fallback tiles
fn fallback_response(settings: &Settings, tiles: &cache::Tiles) -> HttpResponse {
    if tiles.fallback_expired() {
        // Totally expired so no `Cache-Control` header
        HttpResponse::NoContent().finish()
    } else {
        tiles.to_response(settings.cache_control_header)
    }
}
