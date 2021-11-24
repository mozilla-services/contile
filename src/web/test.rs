use std::collections::HashMap;
use std::convert::TryFrom;

use actix_cors::Cors;
use actix_web::{
    http::header, http::StatusCode, middleware::errhandlers::ErrorHandlers, test, web, App,
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
    server::{cache, location::location_config_from_settings, ServerState},
    settings::{test_settings, Settings},
    web::{dockerflow, handlers, middleware},
};

const MOCK_RESPONSE1: &str = include_str!("mock_adm_response1.json");
const UA_91: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:72.0) Gecko/20100101 Firefox/91.0";
const UA_90: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:72.0) Gecko/20100101 Firefox/90.0";
const MMDB_LOC: &str = "mmdb/GeoLite2-City-Test.mmdb";
const TEST_ADDR: &str = "216.160.83.56";

/// customizing the settings
fn get_test_settings() -> Settings {
    let treq = test::TestRequest::with_uri("/").to_http_request();
    Settings {
        maxminddb_loc: Some(MMDB_LOC.into()),
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
            let metrics = Metrics::sink();
            let excluded_dmas = if let Some(exclude_dmas) = &$settings.exclude_dma {
                serde_json::from_str(exclude_dmas).expect("Invalid exclude_dma field")
            } else {
                None
            };
            let state = ServerState {
                metrics: Box::new(metrics.clone()),
                adm_endpoint_url: $settings.adm_endpoint_url.clone(),
                reqwest_client: reqwest::Client::new(),
                tiles_cache: cache::TilesCache::new(10),
                settings: $settings.clone(),
                filter: HandlerResult::<AdmFilter>::from(&mut $settings).unwrap(),
                img_store: None,
                excluded_dmas,
            };
            let location_config = location_config_from_settings(&$settings, &metrics);

            test::init_service(build_app!(state, location_config)).await
        }
    };
}

struct MockAdm {
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
    server.run();
    MockAdm {
        endpoint_url: format!("http://{}:{}/?partner=foo&sub1=bar", addr.ip(), addr.port()),
        request_rx,
    }
}

pub fn adm_settings() -> AdmSettings {
    let adm_settings = json!({
        "Acme": {
            "advertiser_urls": [{ "host": "www.acme.biz" }],
            "impression_hosts": [],
            "click_hosts": [],
            "position": 0,
            "include_regions": ["US"]
        },
        "Dunder Mifflin": {
            "advertiser_urls": [{ "host": "www.dunderm.biz" }],
            "impression_hosts": ["example.com", "example.net"],
            "click_hosts": [],
            "position": 1,
            "include_regions": ["US"]
        },
        "Los Pollos Hermanos": {
            "advertiser_urls": [{ "host": "www.lph-nm.biz" }],
            "impression_hosts": [],
            "click_hosts": [],
            "position": 2,
            "include_regions": ["US"]
        },
        DEFAULT: {
            "advertiser_urls": [],
            "impression_hosts": ["example.net"],
            "click_hosts": ["example.com"],
            "position": null,
            "include_regions": []
        }
    });
    AdmSettings::try_from(adm_settings.as_str().unwrap().to_owned()).unwrap()
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
        .header(header::USER_AGENT, UA_91)
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
async fn basic_old_ua() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let valid = ["acme", "los pollos hermanos"];
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: json!(adm_settings()).to_string(),
        adm_has_legacy_image: Some(json!(valid).to_string()),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA_90)
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
    assert!(tiles.len() == 2);
    let mut previous: String = "".to_owned();
    for tile in tiles {
        let tile = tile.as_object().expect("!tile.is_object()");
        assert!(tile["url"].is_string());
        assert!(tile.get("advertiser_url").is_none());
        let this = tile["name"].as_str().unwrap().to_lowercase();
        assert!(this != previous);
        assert!(valid.contains(&this.as_str()));
        previous = this;
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
        .header(header::USER_AGENT, UA_91)
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
    assert_eq!(tiles.len(), 1);
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
        .header(header::USER_AGENT, UA_91)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[actix_rt::test]
async fn basic_filtered() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());

    let mut adm_settings = adm_settings();
    adm_settings.advertisers.insert(
        "Example".to_owned(),
        serde_json::from_value(json!({
            "advertiser_urls": [{ "host": "www.example.ninja" }],
            "impression_hosts": ["example.net"],
            "click_hosts": ["example.com"],
            "position": 100,
            "include_regions": []
        }))
        .unwrap(),
    );
    adm_settings.advertisers.remove("Dunder Mifflin");

    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: json!(adm_settings).to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA_91)
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
    assert_eq!(tiles.len(), 2);
    // Ensure the tile order from adM is preserved
    let tile1 = &tiles[0];
    assert_eq!(tile1["name"], "Acme");
    assert_eq!(tile1["position"], 0);
    let tile2 = &tiles[1];
    assert_eq!(tile2["name"], "Los Pollos Hermanos");
    assert_eq!(tile2["position"], 2);
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
        .header(header::USER_AGENT, UA_91)
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
    assert_eq!(tiles.len(), 2);
    assert!(!tiles
        .iter()
        .any(|tile| tile["name"].as_str().unwrap() == "Los Pollos Hermanos"));
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
        .header(header::USER_AGENT, UA_91)
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
        .header(header::USER_AGENT, UA_91)
        .header("X-Forwarded-For", TEST_ADDR)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let params = adm.params().await;
    assert_eq!(params.get("country-code"), Some(&"US".to_owned()));
    assert_eq!(params.get("region-code"), Some(&"WA".to_owned()));
}

#[actix_rt::test]
async fn location_test_header() {
    let mut adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        adm_settings: json!(adm_settings()).to_string(),
        location_test_header: Some("x-test-location".to_owned()),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA_91)
        .header("X-Forwarded-For", TEST_ADDR)
        .header("X-Test-Location", "US, CA")
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let params = adm.params().await;
    assert_eq!(params.get("country-code"), Some(&"US".to_owned()));
    assert_eq!(params.get("region-code"), Some(&"CA".to_owned()));
    assert_eq!(params.get("dma-code"), Some(&"".to_owned()));
}

#[actix_rt::test]
async fn empty_tiles() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    // test empty responses of an included country (US)
    let adm_settings_json = json!({
        "Foo": {
            "advertiser_urls": [{ "host": "www.foo.bar" }],
            "impression_hosts": [],
            "click_hosts": [],
            "position": 0,
            "include_regions": ["US"]
        }
    });
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        adm_settings: adm_settings_json.to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA_91)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Ensure same result from cache
    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA_91)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[actix_rt::test]
async fn empty_tiles_excluded_country() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    // no adm_settings filters everything out, the client's country (US) is
    // considered "excluded"
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA_91)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let result: Value = test::read_body_json(resp).await;
    let tiles = result["tiles"].as_array().expect("!tiles.is_array()");
    assert_eq!(tiles.len(), 0);

    // Ensure same result from cache
    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA_91)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let result: Value = test::read_body_json(resp).await;
    let tiles = result["tiles"].as_array().expect("!tiles.is_array()");
    assert_eq!(tiles.len(), 0);
}

#[actix_rt::test]
async fn empty_tiles_excluded_country_204() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    // no adm_settings filters everything out, the client's country (US) is
    // considered "excluded"
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        excluded_countries_200: false,
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA_91)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Ensure same result from cache
    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA_91)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[actix_rt::test]
async fn include_regions() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());

    let mut adm_settings = adm_settings();
    adm_settings.advertisers.remove("Los Pollos Hermanos");
    adm_settings
        .advertisers
        .get_mut("Dunder Mifflin")
        .expect("No Dunder Mifflin tile")
        .include_regions = vec!["MX".to_owned()];
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        adm_settings: json!(adm_settings).to_string(),
        ..get_test_settings()
    };
    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .header(header::USER_AGENT, UA_91)
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // "Dunder Mifflin" should be filtered out
    let result: Value = test::read_body_json(resp).await;
    let tiles = result["tiles"].as_array().expect("!tiles.is_array()");
    assert_eq!(tiles.len(), 1);
    assert_eq!(&tiles[0]["name"], "Acme");
}

#[actix_rt::test]
async fn test_loc() {
    let mut settings = get_test_settings();

    let mut app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/__loc_test__")
        .header("X-FORWARDED-FOR", TEST_ADDR)
        .to_request();

    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let result: Value = test::read_body_json(resp).await;
    assert_eq!(result["country"], "US");
    assert_eq!(result["region"], "WA");
}
