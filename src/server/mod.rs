//! Main application server
use std::{collections::HashMap, sync::Arc, time::Duration};

use actix_cors::Cors;
use actix_web::{
    dev, http::StatusCode, middleware::errhandlers::ErrorHandlers, web, App, HttpRequest,
    HttpResponse, HttpServer,
};
use cadence::StatsdClient;

use crate::{
    error::HandlerError,
    metrics::metrics_from_opts,
    settings::Settings,
    web::{handlers, middleware},
};

pub mod cache;

/// This is the global HTTP state object that will be made available to all
/// HTTP API calls.
#[derive(Clone, Debug)]
pub struct ServerState {
    /// Metric reporting
    pub metrics: Box<StatsdClient>,
    pub settings: Settings,
    pub adm_country_ip_map: Arc<HashMap<String, String>>,
    pub reqwest_client: reqwest::Client,
    pub tiles_cache: cache::TilesCache,
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
            .service(web::resource("/v1/tiles").route(web::get().to(handlers::get_tiles)))
            // Dockerflow
            // Remember to update .::web::middleware::DOCKER_FLOW_ENDPOINTS
            // when applying changes to endpoint names.
            .service(web::resource("/__heartbeat__").route(web::get().to(handlers::heartbeat)))
            .service(
                web::resource("/__lbheartbeat__").route(web::get().to(|_: HttpRequest| {
                    // used by the load balancers, just return OK.
                    HttpResponse::Ok()
                        .content_type("application/json")
                        .body("{}")
                })),
            )
            .service(
                web::resource("/__version__").route(web::get().to(|_: HttpRequest| {
                    // return the contents of the version.json file created by circleci
                    // and stored in the docker root
                    HttpResponse::Ok()
                        .content_type("application/json")
                        .body(include_str!("../../version.json"))
                })),
            )
            .service(web::resource("/__error__").route(web::get().to(handlers::test_error)))
    };
}

impl Server {
    pub async fn with_settings(settings: Settings) -> Result<dev::Server, HandlerError> {
        let state = ServerState {
            metrics: Box::new(metrics_from_opts(&settings)?),
            settings: settings.clone(),
            adm_country_ip_map: Arc::new(settings.build_adm_country_ip_map()),
            reqwest_client: reqwest::Client::new(),
            tiles_cache: cache::TilesCache::new(75),
        };
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
