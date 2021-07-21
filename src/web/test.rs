use std::collections::HashMap;

use actix_cors::Cors;
use actix_web::{
    dev, http::header, http::StatusCode, middleware::errhandlers::ErrorHandlers, test, web, App,
    HttpRequest, HttpResponse, HttpServer,
};
use futures::{channel::mpsc, StreamExt};
use serde_json::{json, Value};
use url::Url;

use crate::{
    adm::{AdmFilter, AdmSettings, DEFAULT},
    build_app,
    error::{HandlerError, HandlerResult},
    metrics::Metrics,
    server::{
        cache,
        location::{
            test::{MMDB_LOC, TEST_ADDR},
            Location,
        },
        ServerState,
    },
    settings::{test_settings, Settings},
    web::{dockerflow, handlers, middleware},
};

const MOCK_RESPONSE1: &str = include_str!("mock_adm_response1.json");
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:72.0) Gecko/20100101 Firefox/72.0";

/// customizing the settings
fn get_test_settings() -> Settings {
    let treq = test::TestRequest::with_uri("/").to_http_request();
    Settings {
        maxminddb_loc: Some(MMDB_LOC.to_owned()),
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
                reqwest_client: reqwest::Client::new(),
                tiles_cache: cache::TilesCache::new(10),
                mmdb: Location::from(&$settings),
                settings: $settings.clone(),
                filter: HandlerResult::<AdmFilter>::from(&mut $settings).unwrap(),
            };
            test::init_service(build_app!(state)).await
        }
    };
}

struct MockAdm {
    pub server: dev::Server,
    pub endpoint_url: String,
    pub request_rx: mpsc::UnboundedReceiver<String>,
}

impl MockAdm {
    /// Return the passed in query params
    async fn params(&mut self) -> HashMap<String, String> {
        let query_string = self.request_rx.next().await.expect("No request_rx result");
        Url::parse(&format!("{}{}", self.endpoint_url, query_string))
            .expect("Couldn't parse request_rx result")
            .query_pairs()
            .into_owned()
            .collect()
    }
}

/// Bind a mock of the AdM Tiles API to a random port on localhost
fn init_mock_adm(response: String) -> MockAdm {
    let (tx, request_rx) = mpsc::unbounded::<String>();
    let server = HttpServer::new(move || {
        let tx = tx.clone();
        App::new().data(response.clone()).route(
            "/",
            web::get().to(move |req: HttpRequest, resp: web::Data<String>| {
                trace!(
                    "mock_adm: path: {:#?} query_string: {:#?} {:#?} {:#?}",
                    req.path(),
                    req.query_string(),
                    req.connection_info(),
                    req.headers()
                );
                // TODO: pass more data for validation
                tx.unbounded_send(req.query_string().to_owned())
                    .expect("Failed to send");
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
    MockAdm {
        server: server.run(),
        endpoint_url: format!("http://{}:{}/?partner=foo&sub1=bar", addr.ip(), addr.port()),
        request_rx,
    }
}

fn adm_settings() -> AdmSettings {
    let adm_settings = json!({
        "Acme": {
            "advertiser_hosts": ["www.acme.biz"],
            "impression_hosts": [],
            "click_hosts": [],
            "position": 0,
            "include_regions": ["US"]
        },
        "Dunder Mifflin": {
            "advertiser_hosts": ["www.dunderm.biz"],
            "impression_hosts": [],
            "click_hosts": [],
            "position": 1,
            "include_regions": ["US"]
        },
        "Los Pollos Hermanos": {
            "advertiser_hosts": ["www.lph-nm.biz"],
            "impression_hosts": [],
            "click_hosts": [],
            "position": 2,
            "include_regions": ["US"]
        },
        DEFAULT: {
            "advertiser_hosts": [],
            "impression_hosts": ["example.net"],
            "click_hosts": ["example.com"],
            "position": null,
            "include_regions": []
        }
    });
    serde_json::from_value(adm_settings).unwrap()
}

/// Basic integration test
///
/// This is a baseline test ensuring that we can read data returned from the ADM server.
/// Since we may not want to hit the ADM server directly, we use a mock response.
#[actix_rt::test]
async fn basic() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: json!(adm_settings()).to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
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
    let adm = init_mock_adm(missing_ci.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: json!(adm_settings()).to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
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
    assert_eq!("Dunder Mifflin", &tiles[0]["name"]);
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
    let adm = init_mock_adm(missing_ci.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: json!(adm_settings()).to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[actix_rt::test]
async fn basic_filtered() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());

    let mut adm_settings = adm_settings();
    adm_settings.insert(
        "Example".to_owned(),
        serde_json::from_value(json!({
            "advertiser_hosts": ["www.example.ninja"],
            "impression_hosts": ["example.net"],
            "click_hosts": ["example.com"],
            "position": 100,
            "include_regions": []
        }))
        .unwrap(),
    );
    adm_settings.remove("Dunder Mifflin");

    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: json!(adm_settings).to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
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
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());

    let adm_settings = adm_settings();
    trace!("Settings: {:?}", &adm_settings);

    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: json!(adm_settings).to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
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
        .map(|tile| tile["name"].as_str().unwrap())
        .collect();
    assert!(!names.contains(&"Dunder Mifflin"));
}

#[actix_rt::test]
async fn fallback_country() {
    let mut adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        adm_settings: json!(adm_settings()).to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let params = adm.params().await;
    assert_eq!(params.get("country-code"), Some(&"US".to_owned()));
    assert_eq!(params.get("region-code"), Some(&"".to_owned()));
}

#[actix_rt::test]
async fn maxmind_lookup() {
    let mut adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        adm_settings: json!(adm_settings()).to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA)
        .header("X-Forwarded-For", TEST_ADDR)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let params = adm.params().await;
    assert_eq!(params.get("country-code"), Some(&"US".to_owned()));
    assert_eq!(params.get("region-code"), Some(&"WA".to_owned()));
}

#[actix_rt::test]
async fn empty_tiles() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    // no adm_settings filters everything out
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Ensure same result from cache
    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[actix_rt::test]
async fn include_regions() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut adm_settings = adm_settings();
    adm_settings.remove("Los Pollos Hermanos");
    let mut acme = adm_settings
        .get_mut("Acme")
        .expect("No Acme tile");
    // ensure case insensitive matching
    acme.include_regions = vec!["us".to_owned()];
    let mut dunderm = adm_settings
        .get_mut("Dunder Mifflin")
        .expect("No Dunder Mifflin tile");
    dunderm.include_regions = vec!["MX".to_owned()];
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        adm_settings: json!(adm_settings).to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // "Dunder Mifflin" should be filtered out
    let result: Value = test::read_body_json(resp).await;
    let tiles = result["tiles"].as_array().expect("!tiles.is_array()");
    assert!(tiles.len() == 1);
    assert_eq!(&tiles[0]["name"], "Acme");
}
