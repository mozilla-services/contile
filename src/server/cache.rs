//! Tile cache manager
use std::{
    fmt::Debug,
    sync::Arc,
    time::{Duration, SystemTime},
};

use actix_web::{
    http::header::{CacheControl, CacheDirective, TryIntoHeaderPair},
    rt, HttpResponse,
};
use cadence::StatsdClient;
use dashmap::DashMap;

use crate::web::handlers::EMPTY_TILES;
use crate::{
    adm::TileResponse,
    error::HandlerError,
    metrics::Metrics,
    web::{FormFactor, OsFamily},
};

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
    /// Platform OS
    pub os_family: OsFamily,
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

    pub fn spawn_periodic_reporter(&self, interval: Duration, metrics: Arc<StatsdClient>) {
        let cache = self.clone();
        let metrics = Metrics::from(metrics);
        rt::spawn(async move {
            loop {
                tiles_cache_periodic_reporter(&cache, &metrics).await;
                rt::time::sleep(interval).await;
            }
        });
    }

    /// Get an immutable reference to an entry in the cache
    pub fn get(
        &self,
        audience_key: &AudienceKey,
    ) -> Option<dashmap::mapref::one::Ref<'_, AudienceKey, TilesState>> {
        self.inner.get(audience_key)
    }

    /// Prepare to write to the cache.
    ///
    /// Sets the cache entry to the Refreshing/Populating states.
    /// `WriteHandle` resets those states when it goes out of scope if no
    /// `insert` call was issued (due to errors or panics).
    pub fn prepare_write<'a>(
        &'a self,
        audience_key: &'a AudienceKey,
        expired: bool,
    ) -> WriteHandle<'a, impl FnOnce(()) + '_> {
        let mut fallback_tiles = None;

        if expired {
            // The cache entry's expired and we're about to refresh it
            trace!("prepare_write: Fresh now expired, Refreshing");
            self.inner
                .alter(audience_key, |_, tiles_state| match tiles_state {
                    TilesState::Fresh { tiles } if tiles.expired() => {
                        // In case an error occurs while doing the write work
                        // we'll render the current value as a fallback
                        fallback_tiles = Some(tiles.clone());
                        TilesState::Refreshing { tiles }
                    }
                    _ => tiles_state,
                });
        } else {
            // We'll populate this cache entry for probably the first time
            trace!("prepare_write: Populating");
            self.inner
                .insert(audience_key.clone(), TilesState::Populating);
        };

        let guard = scopeguard::guard((), move |_| {
            trace!("prepare_write (ScopeGuard cleanup): Resetting state");
            if expired {
                // Back to Fresh (though the tiles are expired): so a later
                // request will retry refreshing again
                self.inner
                    .alter(audience_key, |_, tiles_state| match tiles_state {
                        TilesState::Refreshing { tiles } => TilesState::Fresh { tiles },
                        _ => tiles_state,
                    });
            } else {
                // Clear the entry: a later request will retry populating again
                self.inner.remove_if(audience_key, |_, tiles_state| {
                    matches!(tiles_state, TilesState::Populating)
                });
            }
        });
        WriteHandle {
            cache: self,
            audience_key,
            guard,
            fallback_tiles,
        }
    }
}

/// Manages a write to a specific `TilesCache` entry.
///
/// This will reset the temporary state set by `prepare_write` when it's gone
/// out of scope and no `insert` was issued (e.g. in the case of errors or
/// panics).
pub struct WriteHandle<'a, F>
where
    F: FnOnce(()),
{
    cache: &'a TilesCache,
    audience_key: &'a AudienceKey,
    guard: scopeguard::ScopeGuard<(), F>,
    pub fallback_tiles: Option<Tiles>,
}

impl<F> WriteHandle<'_, F>
where
    F: FnOnce(()),
{
    /// Insert a value into the cache for our audience_key
    pub fn insert(self, tiles: TilesState) {
        self.cache.inner.insert(self.audience_key.clone(), tiles);
        // With the write completed cancel scopeguard's cleanup
        scopeguard::ScopeGuard::into_inner(self.guard);
        trace!("WriteHandle: ScopeGuard defused (cancelled)");
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
    /// When this is in need of a refresh (the `Cache-Control` `max-age`)
    expiry: SystemTime,
    /// After expiry we'll continue serving the stale version of these Tiles
    /// until they're successfully refreshed (acting as a fallback during
    /// upstream service outages). `fallback_expiry` is when we stop serving
    /// this stale Tiles completely
    fallback_expiry: SystemTime,
    /// Return OK instead of NoContent
    always_ok: bool,
}

impl Tiles {
    pub fn new(
        tile_response: TileResponse,
        ttl: Duration,
        fallback_ttl: Duration,
        always_ok: bool,
    ) -> Result<Self, HandlerError> {
        let empty = Self::empty(ttl, fallback_ttl, always_ok);
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

    pub fn empty(ttl: Duration, fallback_ttl: Duration, always_ok: bool) -> Self {
        Self {
            content: TilesContent::Empty,
            expiry: SystemTime::now() + ttl,
            fallback_expiry: SystemTime::now() + fallback_ttl,
            always_ok,
        }
    }

    pub fn expired(&self) -> bool {
        self.expiry <= SystemTime::now()
    }

    pub fn fallback_expired(&self) -> bool {
        self.fallback_expiry <= SystemTime::now()
    }

    pub fn to_response(&self, cache_control_header: bool) -> HttpResponse {
        match &self.content {
            TilesContent::Json(json) => {
                let mut builder = HttpResponse::Ok();
                if cache_control_header {
                    builder.insert_header(self.cache_control_header());
                }
                builder
                    .content_type("application/json")
                    .body(json.to_owned())
            }
            TilesContent::Empty => {
                let mut builder = if self.always_ok {
                    HttpResponse::Ok()
                } else {
                    HttpResponse::NoContent()
                };
                if cache_control_header {
                    builder.insert_header(self.cache_control_header());
                }
                if self.always_ok {
                    builder.body(EMPTY_TILES.as_str())
                } else {
                    builder.finish()
                }
            }
        }
    }

    /// Return the Tiles' `Cache-Control` header
    fn cache_control_header(&self) -> impl TryIntoHeaderPair {
        let max_age = (self.expiry.duration_since(SystemTime::now()))
            .unwrap_or_default()
            .as_secs();
        let stale_if_error = (self.fallback_expiry.duration_since(SystemTime::now()))
            .unwrap_or_default()
            .as_secs();
        let header_value = CacheControl(vec![
            CacheDirective::Private,
            CacheDirective::MaxAge(max_age as u32),
            CacheDirective::Extension(
                "stale-if-error".to_owned(),
                Some(stale_if_error.to_string()),
            ),
        ]);
        ("Cache-Control", header_value)
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

async fn tiles_cache_periodic_reporter(cache: &TilesCache, metrics: &Metrics) {
    trace!("tiles_cache_periodic_reporter");
    // calculate the size and GC (for seldomly used Tiles) while we're at it
    let mut cache_count = 0;
    let mut cache_size = 0;
    for refm in cache.inner.iter() {
        cache_count += 1;
        cache_size += refm.value().size();
    }

    metrics.count("tiles_cache.count", cache_count);
    metrics.count("tiles_cache.size", cache_size as i64);
}
