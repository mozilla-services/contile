//! Tile cache manager
use std::{
    collections::HashMap,
    fmt::Debug,
    ops::Deref,
    sync::Arc,
    time::{Duration, SystemTime},
};

use tokio::sync::RwLock;

use crate::{
    adm,
    metrics::Metrics,
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
    pub ttl: SystemTime,
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
        settings,
        ..
    } = state;

    trace!("tile_cache_updater..");
    let tiles = tiles_cache.read().await;
    let keys: Vec<_> = tiles.keys().cloned().collect();
    let mut cache_size = 0;
    let mut cache_count: i64 = 0;
    for key in keys {
        // proactively remove expired tiles from the cache, since we only
        // write new ones (or ones which return a value)
        if let Some(tile) = tiles.get(&key) {
            if tile.ttl <= SystemTime::now() {
                tiles_cache.write().await.remove(&key);
            }
        }
        let mut tags = Tags::default();
        let metrics = Metrics::from(state);
        let result = adm::get_tiles(
            reqwest_client,
            adm_endpoint_url,
            &LocationResult {
                country: Some(key.country_code.clone()),
                subdivision: Some(key.region_code.clone()),
                city: None,
                dma: None,
            },
            key.os_family,
            key.form_factor,
            state,
            &mut tags,
            &metrics,
            None,
        )
        .await;

        match result {
            Ok(response) => {
                //trace!("tile_cache_updater: {:#?}", response);
                let tiles = match serde_json::to_string(&response) {
                    Ok(tiles) => tiles,
                    Err(e) => {
                        error!("tile_cache_updater: response error {}", e);
                        metrics.incr_with_tags("tile_cache_updater.error", Some(&tags));
                        continue;
                    }
                };
                cache_size += tiles.len();
                cache_count += 1;
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
                    tiles_cache.write().await.insert(
                        key,
                        Tiles {
                            json: tiles,
                            ttl: SystemTime::now() + Duration::from_secs(settings.tiles_ttl as u64),
                        },
                    );
                    metrics.incr_with_tags("tile_cache_updater.update", Some(&tags));
                }
            }
            Err(e) => {
                error!("tile_cache_updater error: {}", e);
                metrics.incr_with_tags("tile_cache_updater.error", Some(&tags));
            }
        }
    }
    let metrics = Metrics::from(state);
    metrics.count("tile_cache_updater.size", cache_size as i64);
    metrics.count("tile_cache_updater.count", cache_count);
}
