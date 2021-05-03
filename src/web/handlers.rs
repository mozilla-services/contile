//! API Handlers
use actix_http::http::Uri;
use actix_web::{web, HttpRequest, HttpResponse};

use super::user_agent;
use crate::{
    adm,
    error::{HandlerError, HandlerErrorKind},
    metrics::Metrics,
    server::{cache, ServerState},
    tags::Tags,
    web::extractors::TilesRequest,
    web::middleware::sentry as l_sentry,
};

pub async fn get_image(
    _req: HttpRequest,
    _metrics: Metrics,
    _state: web::Data<ServerState>,
) -> Result<HttpResponse, HandlerError> {
    trace!("Testing image");

    // pick something arbitrary to play with...
    let target = "https://unitedheroes.net/icons/JRS_128x128.jpg";
    let target_uri: Uri = target.parse()?;

    // if we need to create a bucket (really probably should use the admin panel)
    // just make sure that "allUsers" have read access and whatever user runs this
    // has `Storage Legacy Bucket Writer` and `Storage Object Creator` access.
    //
    // let storage = crate::server::img_storage::StoreImage::create(&state.settings).await?;
    let storage = crate::server::img_storage::StoreImage::default();

    // fetch a remote URL and store it's contents into Google
    match storage.store(&target_uri).await {
        Ok(sr) => {
            dbg!(sr);
        }
        Err(e) => {
            dbg!(HandlerErrorKind::Internal(e.to_string()));
        }
    }

    // Fetch an existing resource. Ideally, the one we just stored.
    if let Some(res) = storage.fetch(&target_uri).await? {
        Ok(HttpResponse::Ok().body(res.url.to_string()))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

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

    {
        // for demonstration purposes
        let mut tags = Tags::default();
        tags.add_extra("ip", fake_ip.as_str());
        tags.add_extra("ua", &stripped_ua);
        tags.add_extra("sub2", &treq.placement);
        // Add/modify the existing request tags.
        tags.commit(&mut request.extensions_mut());
    }

    let audience_key = cache::AudienceKey {
        country: treq.country,
        fake_ip: fake_ip.clone(),
        platform: stripped_ua.clone(),
        placement: treq.placement.clone(),
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
        fake_ip,
        &stripped_ua,
        &treq.placement,
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
                tags.add_extra("err", es);
                tags.add_tag("level", "warning");
                l_sentry::report(&tags, sentry::event_from_error(&e));
                //TODO: probably should do: json!(vec![adm::AdmTile::default()]).to_string()
                warn!("ADM Server error: {:?}", e);
                "[]".to_owned()
            }
            _ => return Err(e),
        },
    };

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(tiles))
}
