use std::{net::SocketAddr, sync::Arc};

use actix_cors::Cors;
use actix_web::{
    dev, http::header, http::StatusCode, middleware::errhandlers::ErrorHandlers, test, web, App,
    HttpRequest, HttpResponse, HttpServer,
};
use serde_json::Value;

use crate::{
    build_app,
    error::HandlerError,
    metrics::Metrics,
    server::ServerState,
    settings::{test_settings, Settings},
    web::{handlers, middleware},
};

const MOCK_RESPONSE1: &str = include_str!("mock_adm_response1.json");
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:72.0) Gecko/20100101 Firefox/72.0";

/// customizing the settings
fn get_test_settings() -> Settings {
    let treq = test::TestRequest::with_uri("/").to_http_request();
    Settings {
        port: treq.uri().port_u16().unwrap_or(8080),
        host: treq.uri().host().unwrap_or("localhost").to_owned(),
        ..test_settings()
    }
}

macro_rules! init_app {
    () => {
        async {
            let settings = get_test_settings();
            init_app!(settings).await
        }
    };
    ($settings:expr) => {
        async {
            crate::logging::init_logging(false).unwrap();
            let state = ServerState {
                metrics: Box::new(Metrics::sink()),
                port: $settings.port,
                adm_endpoint_url: $settings.adm_endpoint_url.clone(),
                adm_country_ip_map: Arc::new($settings.build_adm_country_ip_map()),
                reqwest_client: reqwest::Client::new(),
            };
            test::init_service(build_app!(state)).await
        }
    };
}

/// Bind a mock of the AdM Tiles API to a random port on localhost
fn init_mock_adm() -> (dev::Server, SocketAddr) {
    let server = HttpServer::new(move || {
        App::new().route(
            "/",
            web::get().to(|req: HttpRequest| {
                trace!(
                    "mock_adm: {:#?} {:#?} {:#?}",
                    req.path(),
                    req.connection_info(),
                    req.headers()
                );
                HttpResponse::Ok()
                    .content_type("application/json")
                    .body(MOCK_RESPONSE1)
            }),
        )
    });
    let server = server
        .bind(("127.0.0.1", 0))
        .expect("Couldn't bind mock_adm");
    let addr = server.addrs().pop().expect("No mock_adm addr");
    (server.run(), addr)
}

#[actix_rt::test]
async fn basic() {
    let (_, addr) = init_mock_adm();
    let settings = Settings {
        adm_endpoint_url: format!("http://{}:{}/?partner=foo&sub1=bar", addr.ip(), addr.port()),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles?country=UK&placement=newtab")
        .header(header::USER_AGENT, UA)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let content_type = resp.headers().get(header::CONTENT_TYPE);
    assert!(content_type.is_some());
    assert_eq!(
        content_type
            .unwrap()
            .to_str()
            .expect("Couldn't parse Content-Type"),
        "application/json"
    );

    let result: Value = test::read_body_json(resp).await;
    let tiles = result["tiles"].as_array().expect("!tiles.is_array()");
    assert!(tiles.len() > 1);
    for tile in tiles {
        let _tile = tile.as_object().expect("!tile.is_object()");
    }
}

#[actix_rt::test]
async fn invalid_placement() {
    let (_, addr) = init_mock_adm();
    let settings = Settings {
        adm_endpoint_url: format!("http://{}:{}/?partner=foo&sub1=bar", addr.ip(), addr.port()),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles?country=US&placement=foo")
        .header(header::USER_AGENT, UA)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let content_type = resp.headers().get(header::CONTENT_TYPE);
    assert!(content_type.is_some());
    assert_eq!(
        content_type
            .unwrap()
            .to_str()
            .expect("Couldn't parse Content-Type"),
        "application/json"
    );

    let _result: Value = test::read_body_json(resp).await;
    // XXX: fixup error json
    //assert_eq!(result["code"], 600);
}
