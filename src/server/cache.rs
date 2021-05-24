//! Tile cache manager
use std::{collections::HashMap, fmt::Debug, ops::Deref, sync::Arc, time::Duration};

use cadence::Counted;
use tokio::sync::RwLock;

use crate::{
    adm,
    server::location::LocationResult,
    server::ServerState,
    tags::Tags,
    web::{FormFactor, OsFamily},
};

/// AudienceKey is the primary key used to store and fetch tiles from the
/// local cache.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AudienceKey {
    /// Country in ISO 3166-1 alpha-2 format
    pub country_code: String,
    /// Region/subdivision (e.g. a US state) in ISO 3166-2 format
    pub region_code: String,
    /// The form-factor (e.g. desktop, phone) of the device
    pub form_factor: FormFactor,
    // XXX: *not* currently a targetting parameter (shouldn't be here),
    // temporarily needed for tile_cache_updater
    /// The family of Operating System (e.g. windows, macos) of the device
    pub os_family: OsFamily,
}

/// The stored Tile cache data
#[derive(Debug)]
pub struct Tiles {
    pub json: String,
}

/// The simple tile Cache
#[derive(Debug, Clone)]
pub struct TilesCache {
    inner: Arc<RwLock<HashMap<AudienceKey, Tiles>>>,
}

impl TilesCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::with_capacity(capacity))),
        }
    }
}

impl Deref for TilesCache {
    type Target = Arc<RwLock<HashMap<AudienceKey, Tiles>>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Background tile refresh process
pub fn spawn_tile_cache_updater(interval: Duration, state: ServerState) {
    actix_rt::spawn(async move {
        loop {
            tile_cache_updater(&state).await;
            actix_rt::time::delay_for(interval).await;
        }
    });
}

async fn tile_cache_updater(state: &ServerState) {
    let ServerState {
        tiles_cache,
        reqwest_client,
        adm_endpoint_url,
        metrics,
        ..
    } = state;

    trace!("tile_cache_updater..");
    let keys: Vec<_> = tiles_cache.read().await.keys().cloned().collect();
    for key in keys {
        let mut tags = Tags::default();
        let result = adm::get_tiles(
            reqwest_client,
            adm_endpoint_url,
            &LocationResult {
                country: Some(key.country_code.clone()),
                subdivision: Some(key.region_code.clone()),
                ..Default::default()
            },
            key.os_family,
            key.form_factor,
            state,
            &mut tags,
        )
        .await;

        match result {
            Ok(response) => {
                //trace!("tile_cache_updater: {:#?}", response);
                let tiles = match serde_json::to_string(&response) {
                    Ok(tiles) => tiles,
                    Err(e) => {
                        error!("tile_cache_updater: response error {}", e);
                        metrics.incr_with_tags("tile_cache_updater.error").send();
                        continue;
                    }
                };
                // XXX: not a great comparison (comparing json Strings)..
                let new_tiles = {
                    tiles_cache
                        .read()
                        .await
                        .get(&key)
                        .map_or(true, |cached_tiles| tiles != cached_tiles.json)
                };
                if new_tiles {
                    trace!("tile_cache_updater updating: {:?}", &key);
                    tiles_cache.write().await.insert(key, Tiles { json: tiles });
                    metrics.incr_with_tags("tile_cache_updater.update").send();
                }
            }
            Err(e) => {
                error!("tile_cache_updater error: {}", e);
                metrics.incr_with_tags("tile_cache_updater.error").send();
            }
        }
    }
}
