//! Main application server
use std::sync::{Arc, RwLock};
use std::time::Duration;

use actix_cors::Cors;
use actix_web::{
    dev, http::StatusCode, middleware::errhandlers::ErrorHandlers, web, App, HttpServer,
};
use cadence::StatsdClient;

use crate::{
    adm::AdmFilter,
    error::{HandlerError, HandlerResult},
    metrics::metrics_from_opts,
    server::{img_storage::StoreImage, location::location_config_from_settings},
    settings::Settings,
    web::{dockerflow, handlers, middleware},
};

pub mod cache;
pub mod img_storage;
pub mod location;

/// Arbitrary initial cache size based on the expected mean, feel free to
/// adjust
const TILES_CACHE_INITIAL_CAPACITY: usize = 768;

/// User-Agent sent to adM
const REQWEST_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// This is the global HTTP state object that will be made available to all
/// HTTP API calls.
pub struct ServerState {
    /// Metric reporting
    pub metrics: Box<StatsdClient>,
    pub adm_endpoint_url: String,
    pub reqwest_client: reqwest::Client,
    pub tiles_cache: cache::TilesCache,
    pub settings: Settings,
    pub filter: Arc<RwLock<AdmFilter>>,
    pub img_store: Option<StoreImage>,
    pub excluded_dmas: Option<Vec<u16>>,
}

impl Clone for ServerState {
    fn clone(&self) -> Self {
        Self {
            metrics: self.metrics.clone(),
            adm_endpoint_url: self.adm_endpoint_url.clone(),
            reqwest_client: self.reqwest_client.clone(),
            tiles_cache: self.tiles_cache.clone(),
            settings: self.settings.clone(),
            filter: self.filter.clone(),
            img_store: self.img_store.clone(),
            excluded_dmas: self.excluded_dmas.clone(),
        }
    }
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("ServerState")
            .field("metrics", &self.metrics)
            .field("adm_endpoint_url", &self.adm_endpoint_url)
            .field("reqwest_client", &self.reqwest_client)
            .field("tiles_cache", &self.tiles_cache)
            .finish()
    }
}

/// The main Actix server
pub struct Server;

/// Simplified Actix app builder (used by both the app and unit test)
#[macro_export]
macro_rules! build_app {
    ($state: expr, $location_config: expr) => {
        App::new()
            .data($state)
            .data($location_config.clone())
            // Middleware is applied LIFO
            // These will wrap all outbound responses with matching status codes.
            .wrap(ErrorHandlers::new().handler(StatusCode::NOT_FOUND, HandlerError::render_404))
            // These are our wrappers
            .wrap(middleware::sentry::SentryWrapper::default())
            // Followed by the "official middleware" so they run first.
            // actix is getting increasingly tighter about CORS headers. Our server is
            // not a huge risk but does deliver XHR JSON content.
            // For now, let's be permissive and use NGINX (the wrapping server)
            // for finer grained specification.
            .wrap(Cors::permissive())
            // Next, the API we are implementing
            .service(web::resource("/v1/tiles").route(web::get().to(handlers::get_tiles)))
            // image cache tester...
            //.service(web::resource("/v1/test").route(web::get().to(handlers::get_image)))
            // And finally the behavior necessary to satisfy Dockerflow
            .service(web::scope("/").configure(dockerflow::service))
    };
}

impl Server {
    /// initialize a new instance of the server from [Settings]
    pub async fn with_settings(mut settings: Settings) -> Result<dev::Server, HandlerError> {
        let metrics = metrics_from_opts(&settings)?;
        let filter = HandlerResult::<AdmFilter>::from(&mut settings)?;
        filter.spawn_updater().await;
        let tiles_cache = cache::TilesCache::new(TILES_CACHE_INITIAL_CAPACITY);
        let req = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(settings.connect_timeout))
            .user_agent(REQWEST_USER_AGENT)
            .build()?;
        let img_store = StoreImage::create(&settings, &req).await?;
        let excluded_dmas = if let Some(exclude_dmas) = &settings.exclude_dma {
            serde_json::from_str(exclude_dmas).map_err(|e| {
                HandlerError::internal(&format!("Invalid exclude_dma field: {:?}", e))
            })?
        } else {
            None
        };
        let state = ServerState {
            metrics: Box::new(metrics.clone()),
            adm_endpoint_url: settings.adm_endpoint_url.clone(),
            reqwest_client: req,
            tiles_cache: tiles_cache.clone(),
            settings: settings.clone(),
            filter: Arc::new(RwLock::new(filter)),
            img_store,
            excluded_dmas,
        };
        let location_config = location_config_from_settings(&settings, &metrics);

        tiles_cache.spawn_periodic_reporter(Duration::from_secs(60), metrics.clone());

        let mut server = HttpServer::new(move || build_app!(state.clone(), location_config));
        if let Some(keep_alive) = settings.actix_keep_alive {
            server = server.keep_alive(keep_alive as usize);
        }
        let server = server
            .bind((settings.host, settings.port))
            .expect("Could not get Server in Server::with_settings")
            .run();
        Ok(server)
    }
}
