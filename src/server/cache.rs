//! Tile cache manager
use std::{
    fmt::Debug,
    ops::Deref,
    sync::Arc,
    time::{Duration, SystemTime},
};

use cadence::StatsdClient;
use dashmap::DashMap;

use crate::{metrics::Metrics, web::FormFactor};

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
}

#[derive(Debug, Clone)]
pub struct TilesCache {
    inner: Arc<DashMap<AudienceKey, Tiles>>,
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
                tiles_cache_periodic_reporter(&cache, &metrics).await;
                actix_rt::time::delay_for(interval).await;
            }
        });
    }
}

impl Deref for TilesCache {
    type Target = Arc<DashMap<AudienceKey, Tiles>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug)]
pub struct Tiles {
    pub json: String,
    expiry: SystemTime,
}

impl Tiles {
    pub fn new(json: String, ttl: u32) -> Self {
        Self {
            json,
            expiry: SystemTime::now() + Duration::from_secs(ttl as u64),
        }
    }

    pub fn expired(&self) -> bool {
        self.expiry <= SystemTime::now()
    }
}

async fn tiles_cache_periodic_reporter(cache: &TilesCache, metrics: &Metrics) {
    trace!("tiles_cache_periodic_reporter");
    // calculate the size and GC (for seldomly used Tiles) while we're at it
    let mut cache_count = 0;
    let mut cache_size = 0;
    cache.retain(|_, tiles| {
        if !tiles.expired() {
            cache_count += 1;
            cache_size += tiles.json.len();
            return true;
        }
        false
    });

    metrics.count("tiles_cache.count", cache_count);
    metrics.count("tiles_cache.size", cache_size as i64);
}
