//! API Handlers
use actix_web::{web, HttpRequest, HttpResponse};

use super::user_agent;
use crate::{
    adm,
    error::HandlerError,
    metrics::Metrics,
    server::{cache, ServerState},
    tags::Tags,
    web::extractors::TilesRequest,
};

pub async fn get_tiles(
    treq: TilesRequest,
    metrics: Metrics,
    state: web::Data<ServerState>,
    request: HttpRequest,
) -> Result<HttpResponse, HandlerError> {
    trace!("get_tiles");

    let location = (treq.country, "region".to_string()); // TODO: In lieu of Location
                                                         /*
                                                         let fake_ip = if let Some(ip) = state.adm_country_ip_map.get(&treq.country) {
                                                             ip
                                                         } else {
                                                             state
                                                                 .adm_country_ip_map
                                                                 .get("US")
                                                                 .expect("Invalid ADM_COUNTRY_IP_MAP setting")
                                                         };
                                                         */
    let stripped_ua = user_agent::strip_ua(&treq.ua);

    {
        // for demonstration purposes
        let mut tags = Tags::default();
        tags.add_extra("country", &location.0);
        tags.add_extra("region", &location.1);
        tags.add_extra("ua", &stripped_ua);
        tags.add_extra("sub2", &treq.placement);
        // Add/modify the existing request tags.
        tags.commit(&mut request.extensions_mut());
    }

    let audience_key = cache::AudienceKey {
        country: location.0.clone(),
        // fake_ip: fake_ip.clone(),
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

    let response = adm::get_tiles(
        &state.reqwest_client,
        &state.adm_endpoint_url,
        location,
        &stripped_ua,
        &treq.placement,
        &state,
    )
    .await?;
    let tiles = serde_json::to_string(&response)
        .map_err(|e| HandlerError::internal(&format!("Response failed to serialize: {}", e)))?;
    trace!("get_tiles: cache miss: {:?}", audience_key);
    metrics.incr("tiles_cache.miss");
    state.tiles_cache.write().await.insert(
        audience_key,
        cache::Tiles {
            json: tiles.clone(),
        },
    );

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(tiles))
}
