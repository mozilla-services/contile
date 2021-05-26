use std::{net::SocketAddr, sync::Arc};

use actix_cors::Cors;
use actix_web::{
    dev, http::header, http::StatusCode, middleware::errhandlers::ErrorHandlers, test, web, App,
    HttpRequest, HttpResponse, HttpServer,
};
use serde_json::{json, Value};

use crate::{
    adm::{AdmAdvertiserFilterSettings, AdmFilter, AdmSettings, DEFAULT},
    build_app,
    error::{HandlerError, HandlerResult},
    metrics::Metrics,
    server::{cache, location::Location, ServerState},
    settings::{test_settings, Settings},
    web::{dockerflow, handlers, middleware},
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
                adm_endpoint_url: $settings.adm_endpoint_url.clone(),
                adm_country_ip_map: Arc::new($settings.build_adm_country_ip_map()),
                reqwest_client: reqwest::Client::new(),
                tiles_cache: cache::TilesCache::new(10),
                mmdb: Location::default(),
                settings: $settings.clone(),
                filter: HandlerResult::<AdmFilter>::from(&$settings).unwrap(),
            };
            test::init_service(build_app!(state)).await
        }
    };
}

/// Bind a mock of the AdM Tiles API to a random port on localhost
fn init_mock_adm(response: String) -> (dev::Server, SocketAddr) {
    let server = HttpServer::new(move || {
        App::new().data(response.clone()).route(
            "/",
            web::get().to(|req: HttpRequest, resp: web::Data<String>| {
                trace!(
                    "mock_adm: {:#?} {:#?} {:#?}",
                    req.path(),
                    req.connection_info(),
                    req.headers()
                );
                HttpResponse::Ok()
                    .content_type("application/json")
                    .body(resp.get_ref())
            }),
        )
    });
    let server = server
        .bind(("127.0.0.1", 0))
        .expect("Couldn't bind mock_adm");
    let addr = server.addrs().pop().expect("No mock_adm addr");
    (server.run(), addr)
}

fn adm_settings() -> AdmSettings {
    let mut adm_settings = AdmSettings::default();
    adm_settings.insert(
        "Acme".to_owned(),
        AdmAdvertiserFilterSettings {
            advertiser_hosts: ["www.acme.biz".to_owned()].to_vec(),
            position: Some(0),
            include_regions: vec![],
            impression_hosts: vec![],
            click_hosts: vec![],
        },
    );
    adm_settings.insert(
        "Dunder Mifflin".to_owned(),
        AdmAdvertiserFilterSettings {
            advertiser_hosts: ["www.dunderm.biz".to_owned()].to_vec(),
            position: Some(1),
            include_regions: vec![],
            impression_hosts: [].to_vec(),
            click_hosts: vec![],
        },
    );
    adm_settings.insert(
        "Los Pollos Hermanos".to_owned(),
        AdmAdvertiserFilterSettings {
            advertiser_hosts: ["www.lph-nm.biz".to_owned()].to_vec(),
            position: Some(2),
            include_regions: vec![],
            impression_hosts: vec![],
            click_hosts: vec![],
        },
    );
    // This is the "default" setting definitions.
    adm_settings.insert(
        DEFAULT.to_owned(),
        AdmAdvertiserFilterSettings {
            advertiser_hosts: vec![],
            position: None,
            include_regions: vec![],
            impression_hosts: ["example.net".to_owned()].to_vec(),
            click_hosts: ["example.com".to_owned()].to_vec(),
        },
    );
    adm_settings
}

#[actix_rt::test]
async fn basic() {
    let (_, addr) = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let settings = Settings {
        adm_endpoint_url: format!("http://{}:{}/?partner=foo&sub1=bar", addr.ip(), addr.port()),
        adm_settings: json!(adm_settings()).to_string(),
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
        let tile = tile.as_object().expect("!tile.is_object()");
        assert!(tile["url"].is_string());
        assert!(tile.get("advertiser_url").is_none());
    }
}

#[actix_rt::test]
async fn basic_bad_reply() {
    let missing_ci = r#"{
        "tiles": [
            {
                "id": 601,
                "name": "Acme",
                "click_url": "https://example.com/ctp?version=16.0.0&key=22.1&ctag=1612376952400200000",
                "image_url": "https://cdn.example.com/601.jpg",
                "advertiser_url": "https://www.acme.biz/?foo=1&device=Computers&cmpgn=123601",
                "impression_url": "https://example.net/static?id=0000"
            },
            {
                "id": 703,
                "name": "Dunder Mifflin",
                "click_url": "https://example.com/ctp?version=16.0.0&key=7.2&ci=8.9&ctag=E1DE38C8972D0281F5556659A",
                "image_url": "https://cdn.example.com/703.jpg",
                "advertiser_url": "https://www.dunderm.biz/?tag=bar&ref=baz",
                "impression_url": "https://example.net/static?id=DEADB33F"
            }
        ]}"#;
    let (_, addr) = init_mock_adm(missing_ci.to_owned());
    let settings = Settings {
        adm_endpoint_url: format!("http://{}:{}/?partner=foo&sub1=bar", addr.ip(), addr.port()),
        adm_settings: json!(adm_settings()).to_string(),
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
    assert!(tiles.len() == 1);
    assert!(
        &tiles[0]
            .as_object()
            .unwrap()
            .get("name")
            .unwrap()
            .to_string()
            == "\"Dunder Mifflin\""
    );
}

#[actix_rt::test]
async fn basic_all_bad_reply() {
    let missing_ci = r#"{
        "tiles": [
            {
                "id": 601,
                "name": "Acme",
                "click_url": "https://example.com/ctp?version=16.0.0&key=22.1&ctag=1612376952400200000",
                "image_url": "https://cdn.example.com/601.jpg",
                "advertiser_url": "https://www.acme.biz/?foo=1&device=Computers&cmpgn=123601",
                "impression_url": "https://example.net/static?id=0000"
            },
            {
                "id": 703,
                "name": "Dunder Mifflin",
                "click_url": "https://example.com/ctp?version=16.0.0&key=7.2&ci=8.9",
                "image_url": "https://cdn.example.com/703.jpg",
                "advertiser_url": "https://www.dunderm.biz/?tag=bar&ref=baz",
                "impression_url": "https://example.net/static?id=DEADB33F"
            }
        ]}"#;
    let (_, addr) = init_mock_adm(missing_ci.to_owned());
    let settings = Settings {
        adm_endpoint_url: format!("http://{}:{}/?partner=foo&sub1=bar", addr.ip(), addr.port()),
        adm_settings: json!(adm_settings()).to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles?country=US&placement=newtab")
        .header(header::USER_AGENT, UA)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[actix_rt::test]
async fn basic_filtered() {
    let (_, addr) = init_mock_adm(MOCK_RESPONSE1.to_owned());

    let mut adm_settings = adm_settings();
    adm_settings.insert(
        "Example".to_owned(),
        AdmAdvertiserFilterSettings {
            advertiser_hosts: ["www.example.ninja".to_owned()].to_vec(),
            position: Some(100),
            include_regions: Vec::new(),
            impression_hosts: ["example.net".to_owned()].to_vec(),
            click_hosts: ["example.com".to_owned()].to_vec(),
        },
    );
    adm_settings.remove("Dunder Mifflin");

    let settings = Settings {
        adm_endpoint_url: format!("http://{}:{}/?partner=foo&sub1=bar", addr.ip(), addr.port()),
        adm_settings: json!(adm_settings).to_string(),
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
    // remember, we cap at `settings.adm_max_tiles` (currently 2)
    assert!(tiles.len() == 2);
    for tile in tiles {
        let tile = tile.as_object().expect("!tile.is_object()");
        match tile.get("name").unwrap().as_str() {
            Some("Acme") => assert!(tile.get("position") == Some(&Value::from(0))),
            Some("Los Pollos Hermanos") => assert!(tile.get("position") == Some(&Value::from(2))),
            _ => panic!("Unknown result"),
        }
    }
}

#[actix_rt::test]
async fn basic_default() {
    let (_, addr) = init_mock_adm(MOCK_RESPONSE1.to_owned());

    let adm_settings = adm_settings();
    dbg!(&adm_settings);

    let settings = Settings {
        adm_endpoint_url: format!("http://{}:{}/?partner=foo&sub1=bar", addr.ip(), addr.port()),
        adm_settings: json!(adm_settings).to_string(),
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
    let names: Vec<&str> = tiles
        .iter()
        .map(|tile| {
            tile.as_object()
                .unwrap()
                .get("name")
                .unwrap()
                .as_str()
                .unwrap()
        })
        .collect();
    assert!(!names.contains(&"Dunder Mifflin"));
}

#[actix_rt::test]
async fn invalid_placement() {
    let (_, addr) = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let settings = Settings {
        adm_endpoint_url: format!("http://{}:{}/?partner=foo&sub1=bar", addr.ip(), addr.port()),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles?country=US&placement=bus12")
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
