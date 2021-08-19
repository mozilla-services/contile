//! Tile cache manager
use std::{
    fmt::Debug,
    ops::Deref,
    sync::Arc,
    time::{Duration, SystemTime},
};

use cadence::StatsdClient;
use dashmap::DashMap;

use crate::{adm::TileResponse, error::HandlerError, metrics::Metrics, web::FormFactor};

/// AudienceKey is the primary key used to store and fetch tiles from the
/// local cache.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AudienceKey {
    /// Country in ISO 3166-1 alpha-2 format
    pub country_code: String,
    /// Region/subdivision (e.g. a US state) in ISO 3166-2 format
    pub region_code: Option<String>,
    /// The DMA code (u16)
    pub dma_code: Option<u16>,
    /// The form-factor (e.g. desktop, phone) of the device
    pub form_factor: FormFactor,
    /// Only serve legacy
    pub legacy_only: bool,
}

#[derive(Debug, Clone)]
pub struct TilesCache {
    inner: Arc<DashMap<AudienceKey, TilesState>>,
}

impl TilesCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(DashMap::with_capacity(capacity)),
        }
    }

    pub fn spawn_periodic_reporter(&self, interval: Duration, metrics: StatsdClient) {
        let cache = self.clone();
        let metrics = Metrics::from(&metrics);
        actix_rt::spawn(async move {
            loop {
                tiles_cache_garbage_collect(&cache, &metrics).await;
                actix_rt::time::delay_for(interval).await;
            }
        });
    }
}

impl Deref for TilesCache {
    type Target = Arc<DashMap<AudienceKey, TilesState>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Clone, Debug)]
/// Wrapper around Tiles with additional state about any outstanding partner
/// requests
pub enum TilesState {
    /// A task is currently populating this entry (via [crate::adm::get_tiles])
    Populating,
    /// Tiles that haven't expired (or been identified as expired) yet
    Fresh { tiles: Tiles },
    /// A task is currently refreshing this expired entry (via
    /// [crate::adm::get_tiles])
    Refreshing { tiles: Tiles },
}

impl TilesState {
    fn size(&self) -> usize {
        match self {
            TilesState::Populating { .. } => 0,
            TilesState::Fresh { tiles } | TilesState::Refreshing { tiles } => tiles.content.size(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Tiles {
    pub content: TilesContent,
    expiry: SystemTime,
}

impl Tiles {
    pub fn new(tile_response: TileResponse, ttl: u32) -> Result<Self, HandlerError> {
        let empty = Self::empty(ttl);
        if tile_response.tiles.is_empty() {
            return Ok(empty);
        }
        let json = serde_json::to_string(&tile_response)
            .map_err(|e| HandlerError::internal(&format!("Response failed to serialize: {}", e)))?;
        Ok(Self {
            content: TilesContent::Json(json),
            ..empty
        })
    }

    pub fn empty(ttl: u32) -> Self {
        Self {
            content: TilesContent::Empty,
            expiry: SystemTime::now() + Duration::from_secs(ttl as u64),
        }
    }

    pub fn expired(&self) -> bool {
        self.expiry <= SystemTime::now()
    }
}

#[derive(Clone, Debug)]
pub enum TilesContent {
    Json(String),
    Empty,
}

impl TilesContent {
    fn size(&self) -> usize {
        match self {
            Self::Json(json) => json.len(),
            _ => 0,
        }
    }
}

async fn tiles_cache_garbage_collect(cache: &TilesCache, metrics: &Metrics) {
    trace!("tiles_cache_garbage_collect");
    // calculate the size and GC (for seldomly used Tiles) while we're at it
    let mut cache_count = 0;
    let mut cache_size = 0;
    for refm in cache.iter() {
        cache_count += 1;
        cache_size += refm.value().size();
    }

    metrics.count("tiles_cache.count", cache_count);
    metrics.count("tiles_cache.size", cache_size as i64);
}
