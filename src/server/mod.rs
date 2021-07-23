//! Main application server
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
    server::img_storage::StoreImage,
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
#[derive(Clone)]
pub struct ServerState {
    /// Metric reporting
    pub metrics: Box<StatsdClient>,
    pub adm_endpoint_url: String,
    pub reqwest_client: reqwest::Client,
    pub tiles_cache: cache::TilesCache,
    pub mmdb: location::Location,
    pub settings: Settings,
    pub filter: AdmFilter,
    pub img_store: Option<StoreImage>,
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mmdb_status = if self.mmdb.is_available() {
            "is set"
        } else {
            "is not set"
        };
        fmt.debug_struct("ServerState")
            .field("metrics", &self.metrics)
            .field("adm_endpoint_url", &self.adm_endpoint_url)
            .field("reqwest_client", &self.reqwest_client)
            .field("tiles_cache", &self.tiles_cache)
            .field("mmdb", &mmdb_status.to_owned())
            .finish()
    }
}

/// The main Actix server
pub struct Server;

/// Simplified Actix app builder (used by both the app and unit test)
#[macro_export]
macro_rules! build_app {
    ($state: expr) => {
        App::new()
            .data($state)
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
        let filter = HandlerResult::<AdmFilter>::from(&mut settings)?;
        let metrics = metrics_from_opts(&settings)?;
        let tiles_cache = cache::TilesCache::new(TILES_CACHE_INITIAL_CAPACITY);
        let req = reqwest::Client::builder()
            .user_agent(REQWEST_USER_AGENT)
            .build()?;
        let img_store = StoreImage::create(&settings, &req).await?;
        let state = ServerState {
            metrics: Box::new(metrics.clone()),
            adm_endpoint_url: settings.adm_endpoint_url.clone(),
            reqwest_client: req,
            tiles_cache: tiles_cache.clone(),
            mmdb: (&settings).into(),
            settings: settings.clone(),
            filter,
            img_store,
        };

        tiles_cache.spawn_periodic_reporter(Duration::from_secs(60), metrics);

        let mut server = HttpServer::new(move || build_app!(state.clone()));
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
