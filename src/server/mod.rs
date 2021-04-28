//! Main application server
use std::{collections::BTreeMap, sync::Arc, time::Duration};

use actix_cors::Cors;
use actix_web::{
    dev, http::StatusCode, middleware::errhandlers::ErrorHandlers, web, App, HttpServer,
};
use cadence::StatsdClient;

use crate::{
    adm::AdmFilter,
    error::{HandlerError, HandlerResult},
    metrics::metrics_from_opts,
    settings::Settings,
    web::{dockerflow, handlers, middleware},
};

pub mod cache;
pub mod img_storage;

/// This is the global HTTP state object that will be made available to all
/// HTTP API calls.
#[derive(Clone, Debug)]
pub struct ServerState {
    /// Metric reporting
    pub metrics: Box<StatsdClient>,
    pub adm_endpoint_url: String,
    pub adm_country_ip_map: Arc<BTreeMap<String, String>>,
    pub reqwest_client: reqwest::Client,
    pub tiles_cache: cache::TilesCache,
    pub settings: Settings,
    pub filter: AdmFilter,
}

pub struct Server;

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
    pub async fn with_settings(settings: Settings) -> Result<dev::Server, HandlerError> {
        let filter = HandlerResult::<AdmFilter>::from(&settings)?;
        let state = ServerState {
            metrics: Box::new(metrics_from_opts(&settings)?),
            adm_endpoint_url: settings.adm_endpoint_url.clone(),
            adm_country_ip_map: Arc::new(settings.build_adm_country_ip_map()),
            reqwest_client: reqwest::Client::new(),
            tiles_cache: cache::TilesCache::new(75),
            settings: settings.clone(),
            filter,
        };

        // causing panic in arbiter thread
        cache::spawn_tile_cache_updater(
            Duration::from_secs(settings.tiles_ttl as u64),
            state.clone(),
        );

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
