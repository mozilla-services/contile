//! Main application server
use std::{collections::HashMap, sync::Arc};

use actix_cors::Cors;
use actix_web::{
    dev, http::StatusCode, middleware::errhandlers::ErrorHandlers, web, App, HttpRequest,
    HttpResponse, HttpServer,
};
use cadence::StatsdClient;

use crate::error::HandlerError;
use crate::metrics;
use crate::settings::Settings;
use crate::web::{handlers, middleware};

/// This is the global HTTP state object that will be made available to all
/// HTTP API calls.
#[derive(Debug)]
pub struct ServerState {
    /// Metric reporting
    pub metrics: Box<StatsdClient>,
    pub port: u16,
    pub adm_endpoint_url: String,
    pub adm_country_ip_map: Arc<HashMap<String, String>>,
    pub reqwest_client: reqwest::Client,
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
        let metrics = metrics::metrics_from_opts(&settings)?;
        let host = settings.host.clone();
        let port = settings.port;
        let adm_endpoint_url = settings.adm_endpoint_url.clone();
        let adm_country_ip_map = Arc::new(settings.build_adm_country_ip_map());
        let reqwest_client = reqwest::Client::new();

        let mut server = HttpServer::new(move || {
            // Setup the server state
            let state = ServerState {
                metrics: Box::new(metrics.clone()),
                port,
                adm_endpoint_url: adm_endpoint_url.clone(),
                reqwest_client: reqwest_client.clone(),
                adm_country_ip_map: Arc::clone(&adm_country_ip_map),
            };

            build_app!(state)
        });
        if let Some(keep_alive) = settings.actix_keep_alive {
            server = server.keep_alive(keep_alive as usize);
        }
        let server = server
            .bind(format!("{}:{}", host, port))
            .expect("Could not get Server in Server::with_settings")
            .run();
        Ok(server)
    }
}
