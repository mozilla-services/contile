use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use actix_cors::Cors;
use actix_web::{
    http::header,
    http::StatusCode,
    middleware::ErrorHandlers,
    rt, test,
    web::{self, Data},
    App, HttpRequest, HttpResponse, HttpServer,
};
use cadence::{SpyMetricSink, StatsdClient};
use crossbeam_channel::Receiver;
use futures::{channel::mpsc, StreamExt};
use regex::Regex;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use url::Url;

use crate::{
    adm::{settings::AdmAdvertiserSettings, AdmFilter, AdvertiserUrlFilter},
    build_app,
    error::{HandlerError, HandlerResult},
    server::{cache, location::location_config_from_settings, ServerState},
    settings::{test_settings, Settings},
    sov::SOVManager,
    web::{dockerflow, handlers, middleware},
};

const MOCK_RESPONSE1: &str = include_str!("mock_adm_response1.json");
const UA_91: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:72.0) Gecko/20100101 Firefox/91.0";
const UA_90: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:72.0) Gecko/20100101 Firefox/90.0";
const UA_IPHONE: &str =
    "Mozilla/5.0 (iPhone; CPU iPhone OS 14_8_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/91.0 Mobile/15E148 Safari/605.1.15";
const UA_FORM_FACTOR_OTHER: &str = "Mozilla/5.0 (Raspberry Pi 3) Gecko/20100101 Firefox/91.0";
const MMDB_LOC: &str = "mmdb/GeoLite2-City-Test.mmdb";
const TEST_ADDR: &str = "216.160.83.56";
const MOCK_SOV: &str = "eyJuYW1lIjoiU09WLTIwMjMwNTE4MjE1MzE2IiwiYWxsb2NhdGlv\
                        bnMiOlt7InBvc2l0aW9uIjoxLCJhbGxvY2F0aW9uIjpbeyJwYXJ0bmV\
                        yIjoiYW1wIiwicGVyY2VudGFnZSI6MTAwfV19LHsicG9zaXRpb24iOj\
                        IsImFsbG9jYXRpb24iOlt7InBhcnRuZXIiOiJhbXAiLCJwZXJjZW50Y\
                        WdlIjo4OH0seyJwYXJ0bmVyIjoibW96LXNhbGVzIiwicGVyY2VudGFn\
                        ZSI6MTJ9XX1dfQ";
/// customizing the settings
pub fn get_test_settings() -> Settings {
    let treq = test::TestRequest::with_uri("/").to_http_request();
    Settings {
        maxminddb_loc: Some(MMDB_LOC.into()),
        port: treq.uri().port_u16().unwrap_or(8080),
        host: treq.uri().host().unwrap_or("localhost").to_owned(),
        adm_defaults: Some(
            json!(crate::adm::AdmDefaults {
                click_hosts: [crate::adm::settings::break_hosts("example.com".to_owned())].to_vec(),
                image_hosts: [crate::adm::settings::break_hosts(
                    "cdn.example.com".to_owned()
                )]
                .to_vec(),
                impression_hosts: [crate::adm::settings::break_hosts("example.net".to_owned())]
                    .to_vec(),
                ..Default::default()
            })
            .to_string(),
        ),
        sov_source: json!({
            "name": "SOV-20230518215316",
            "allocations": [
                {
                    "position": 1,
                    "allocation": [
                        {
                            "partner": "amp",
                            "percentage": 100
                        }
                    ]
                },
                {
                    "position": 2,
                    "allocation": [
                        {
                            "partner": "amp",
                            "percentage": 88
                        },
                        {
                            "partner": "moz-sales",
                            "percentage": 12
                        }
                    ]
                }
            ]
        })
        .to_string(),
        ..test_settings()
    }
}

/// Create a test application with a `SpyMetricSink`
macro_rules! init_app_with_spy {
    () => {
        async {
            let mut settings = get_test_settings();
            init_app_with_spy!(settings).await
        }
    };
    ($settings:expr) => {
        async {
            crate::logging::init_logging(false).unwrap();
            let (spy, sink) = SpyMetricSink::new();
            let metrics = Arc::new(StatsdClient::builder("contile", sink).build());
            let excluded_dmas = if let Some(exclude_dmas) = &$settings.exclude_dma {
                serde_json::from_str(exclude_dmas).expect("Invalid exclude_dma field")
            } else {
                None
            };
            let state = ServerState {
                metrics: Arc::clone(&metrics),
                reqwest_client: reqwest::Client::builder()
                    .connect_timeout(Duration::from_secs(3))
                    .build()
                    .unwrap(),
                tiles_cache: cache::TilesCache::new(10),
                settings: $settings.clone(),
                partner_filter: Arc::new(RwLock::new(
                    HandlerResult::<AdmFilter>::from(&mut $settings).unwrap(),
                )),
                sov_manager: Arc::new(RwLock::new(
                    HandlerResult::<SOVManager>::from(&mut $settings).unwrap(),
                )),
                img_store: None,
                excluded_dmas,
                start_up: std::time::Instant::now(),
            };
            let location_config = location_config_from_settings(&$settings, metrics);

            let service = test::init_service(build_app!(state, location_config)).await;
            (service, spy)
        }
    };
}

/// Create a test application, ignoring the `SpyMetricSink`
macro_rules! init_app {
    ($( $args:expr )*) => {
        async {
            let (app, _) = init_app_with_spy!($( $args )*).await;
            app
        }
    }
}

struct MockAdm {
    pub endpoint_url: String,
    pub request_rx: mpsc::UnboundedReceiver<String>,
}

impl MockAdm {
    /// Return the passed in query params
    async fn params(&mut self) -> HashMap<String, String> {
        let query_string = self.request_rx.next().await.expect("No request_rx result");
        Url::parse(&format!("{}?{}", self.endpoint_url, query_string))
            .expect("Couldn't parse request_rx result")
            .query_pairs()
            .into_owned()
            .collect()
    }

    /// Set the mock AdM to respond with a 5xx error
    fn set_response_error(&mut self) {
        self.request_rx.close();
    }
}

/// Bind a mock of the AdM Tiles API to a random port on localhost
fn init_mock_adm(response: String) -> MockAdm {
    async fn adm_tiles(
        req: HttpRequest,
        resp: web::Data<String>,
        tx: web::Data<futures::channel::mpsc::UnboundedSender<String>>,
    ) -> actix_web::error::Result<HttpResponse> {
        trace!(
            "mock_adm: path: {:#?} query_string: {:#?} {:#?} {:#?}",
            req.path(),
            req.query_string(),
            req.connection_info(),
            req.headers()
        );
        // TODO: pass more data for validation
        tx.unbounded_send(req.query_string().to_owned())
            // set_response_error called
            .map_err(actix_web::error::ErrorServiceUnavailable)?;
        Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(resp.get_ref().to_owned()))
    }

    let (tx, request_rx) = mpsc::unbounded::<String>();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(response.clone()))
            .app_data(Data::new(tx.clone()))
            .route("/", web::get().to(adm_tiles))
    });
    let server = server
        .bind(("127.0.0.1", 0))
        .expect("Couldn't bind mock_adm");
    let addr = server.addrs().pop().expect("No mock_adm addr");
    rt::spawn(server.run());
    MockAdm {
        endpoint_url: format!("http://{}:{}/", addr.ip(), addr.port()),
        request_rx,
    }
}

pub fn advertiser_filters() -> AdmAdvertiserSettings {
    serde_json::from_str(
        &json!({"adm_advertisers":{
            "Acme": {
                "US": [{ "host": "www.acme.biz" }],
            },
            "Dunder Mifflin": {
                "US": [{ "host": "www.dunderm.biz" }],
            },
            "Los Pollos Hermanos": {
                "US": [{ "host": "www.lph-nm.biz" }],
            },
            "Lasagna Come Out Tomorrow": {
                "US": [{ "host": "www.lasagna.restaurant" }],
            }
        }})
        .to_string(),
    )
    .unwrap()
}

/// Find all metric lines emitted from spy with matching prefixes
pub fn find_metrics(spy: &Receiver<Vec<u8>>, prefixes: &[&str]) -> Vec<String> {
    spy.try_iter()
        .filter_map(|m| {
            let m = String::from_utf8(m).unwrap();
            prefixes
                .iter()
                .any(|prefix| m.starts_with(prefix))
                .then_some(m)
        })
        .collect()
}

/// Basic integration test
///
/// This is a baseline test ensuring that we can read data returned from the ADM server.
/// Since we may not want to hit the ADM server directly, we use a mock response.
#[actix_web::test]
async fn basic() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let adm_settings = AdmFilter::advertisers_to_string(advertiser_filters());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings,
        adm_defaults: Some(
            json!(crate::adm::AdmDefaults {
                click_hosts: [crate::adm::break_hosts("example.com".to_owned())].to_vec(),
                image_hosts: [crate::adm::break_hosts("cdn.example.com".to_owned())].to_vec(),
                impression_hosts: [
                    crate::adm::break_hosts("example.net".to_owned()),
                    crate::adm::break_hosts("example.com".to_owned())
                ]
                .to_vec(),
                ..Default::default()
            })
            .to_string(),
        ),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
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

#[actix_web::test]
async fn basic_old_ua() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let valid = ["acme", "los pollos hermanos"];
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        adm_has_legacy_image: Some(json!(valid).to_string()),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_90))
        .to_request();
    let resp = test::call_service(&app, req).await;
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

#[actix_web::test]
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
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
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

#[actix_web::test]
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
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[actix_web::test]
async fn basic_filtered() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut adm_settings = advertiser_filters();
    adm_settings.adm_advertisers.insert(
        "Example".to_owned(),
        HashMap::from([(
            "US".to_owned(),
            [AdvertiserUrlFilter {
                host: "www.example.ninja".to_owned(),
                ..Default::default()
            }]
            .to_vec(),
        )]),
    );
    adm_settings.adm_advertisers.remove("dunder mifflin");

    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: AdmFilter::advertisers_to_string(adm_settings),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
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
    // for desktop, we cap at `settings.adm_max_tiles` (currently 3)
    assert_eq!(tiles.len(), 3);
    // Ensure the tile order from adM is preserved
    let tile1 = &tiles[0];
    assert_eq!(tile1["name"], "Acme");
    let tile2 = &tiles[1];
    assert_eq!(tile2["name"].as_str().unwrap(), "Los Pollos Hermanos");
    let sov = result["sov"].as_str();
    assert_eq!(sov, Some(MOCK_SOV))
}

#[actix_web::test]
async fn basic_filtered2() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    // Ensure the filtering process returns unfiltered Tiles
    let filters = json!({"adm_advertisers":{
        "Acme": {
            "US": [{ "host": "www.acme.biz" }],
         },
        "Dunder Mifflin": {
        },
        "Los Pollos Hermanos": {
        },
        "Lasagna Come Out Tomorrow": {
        },
    }

    })
    .to_string();
    let adm_settings = filters;
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings,
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let result: Value = test::read_body_json(resp).await;
    let tiles = result["tiles"].as_array().expect("!tiles.is_array()");
    assert_eq!(tiles.len(), 1);
    let tile1 = &tiles[0];
    assert_eq!(tile1["name"], "Acme");
    let sov = result["sov"].as_str();
    assert_eq!(sov, Some(MOCK_SOV))
}

#[actix_web::test]
async fn basic_default() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());

    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
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
    // for desktop, we cap at `settings.adm_max_tiles` (currently 3)
    assert_eq!(tiles.len(), 3);
    assert!(!tiles
        .iter()
        .any(|tile| tile["name"].as_str().unwrap() == "Lasagna Come Out Tomorrow"));
}

#[actix_web::test]
async fn basic_mobile() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());

    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_IPHONE))
        .to_request();
    let resp = test::call_service(&app, req).await;
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
    // for mobile, we cap at `settings.adm_mobile_max_tiles` (currently 2)
    assert_eq!(tiles.len(), 2);
    assert!(!tiles
        .iter()
        .any(|tile| tile["name"].as_str().unwrap() == "Los Pollos Hermanos"));

    // mobile should not carry the `sov` payload.
    assert!(result["sov"].is_null());
}

#[actix_web::test]
async fn fallback_country() {
    let mut adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let params = adm.params().await;
    assert_eq!(params.get("country-code"), Some(&"US".to_owned()));
    assert_eq!(params.get("region-code"), Some(&"".to_owned()));
}

#[actix_web::test]
async fn maxmind_lookup() {
    let mut adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .insert_header(("X-Forwarded-For", TEST_ADDR))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let params = adm.params().await;
    assert_eq!(params.get("country-code"), Some(&"US".to_owned()));
    assert_eq!(params.get("region-code"), Some(&"WA".to_owned()));
}

#[actix_web::test]
async fn location_test_header() {
    let mut adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        location_test_header: Some("x-test-location".to_owned()),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .insert_header(("X-Forwarded-For", TEST_ADDR))
        .insert_header(("X-Test-Location", "US, CA"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let params = adm.params().await;
    assert_eq!(params.get("country-code"), Some(&"US".to_owned()));
    assert_eq!(params.get("region-code"), Some(&"CA".to_owned()));
    assert_eq!(params.get("dma-code"), Some(&"".to_owned()));
}

#[actix_web::test]
async fn empty_tiles() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    // test empty responses of an included country (US)
    let adm_settings_json = json!({
        "adm_advertisers":{
            "Foo": {
                "US": [
                    { "host": "www.foo.bar" }
                ]
            }
        }

    });
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: adm_settings_json.to_string(),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Ensure same result from cache
    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[actix_web::test]
async fn empty_tiles_excluded_country() {
    // A response where all candidate tiles have been filtered out returns a
    // 204 response.
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    // Specify valid advertisers with no per country information. This will
    // "exclude" US locations.
    let filters = json!({"adm_advertisers":{
        "Acme": {
         },
        "Dunder Mifflin": {
        },
        "Los Pollos Hermanos": {
        },
    }

    })
    .to_string();
    let adm_settings = filters;
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings,
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Ensure same result from cache
    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[actix_web::test]
async fn include_regions() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());

    let mut adm_settings = advertiser_filters();
    adm_settings
        .adm_advertisers
        .remove(&"Los Pollos Hermanos".to_lowercase());
    // set Dunder Mifflin to only serve Mexico.
    let a_s = adm_settings
        .adm_advertisers
        .get_mut(&"Dunder Mifflin".to_lowercase())
        .expect("No Dunder Mifflin tile");
    a_s.insert("MX".into(), a_s.get("US").unwrap().clone());
    a_s.remove("US");
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: AdmFilter::advertisers_to_string(adm_settings),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // "Dunder Mifflin" should be filtered out
    let result: Value = test::read_body_json(resp).await;
    let tiles = result["tiles"].as_array().expect("!tiles.is_array()");
    assert_eq!(tiles.len(), 2);
    assert_eq!(&tiles[0]["name"], "Acme");
}

#[actix_web::test]
async fn empty_tiles_unknown_form_factor_204() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_FORM_FACTOR_OTHER))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Ensure same result from cache
    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_FORM_FACTOR_OTHER))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[actix_web::test]
async fn loc_test() {
    let app = init_app!().await;

    let req = test::TestRequest::get()
        .uri("/__loc_test__")
        .insert_header(("X-FORWARDED-FOR", TEST_ADDR))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let result: Value = test::read_body_json(resp).await;
    assert_eq!(result["country"], "US");
    assert_eq!(result["region"], "WA");
}

#[actix_web::test]
async fn metrics() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        ..get_test_settings()
    };
    let (app, spy) = init_app_with_spy!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let prefixes = &[
        "contile.tiles.get:1",
        "contile.tiles.adm.request:1",
        "contile.tiles.adm.response.tiles_count:4",
    ];
    let metrics = find_metrics(&spy, prefixes);
    assert_eq!(metrics.len(), 3);
    let get_metric = &metrics[0];
    assert!(get_metric.contains("ua.form_factor:desktop"));
    assert!(get_metric.contains("ua.os.family:windows"));
    assert!(!&metrics[1].contains("endpoint:mobile"));
    assert!(&metrics[1].contains("geo.country_code"));

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_IPHONE))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let metrics = find_metrics(&spy, prefixes);
    assert_eq!(metrics.len(), 3);
    let get_metric = &metrics[0];
    assert!(get_metric.contains("ua.form_factor:phone"));
    assert!(get_metric.contains("ua.os.family:ios"));
    assert!(&metrics[1].contains("endpoint:mobile"));
    assert!(&metrics[1].contains("geo.country_code"));
}

#[actix_web::test]
async fn not_found() {
    let app = init_app!().await;

    let req = test::TestRequest::get().uri("/non-existent").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let result: Value = test::read_body_json(resp).await;
    assert_eq!(result["code"], 404);
    assert_eq!(result["errno"], 404);
}

#[actix_web::test]
async fn zero_ttl() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        tiles_ttl: 0,
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn zero_jitter() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        tiles_ttl: 1,
        jitter: 0,
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn cache_header() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        location_test_header: Some("x-test-location".to_owned()),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .insert_header(("X-Forwarded-For", TEST_ADDR))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let cache_header = resp
        .headers()
        .get("Cache-Control")
        .expect("No Cache-Control header")
        .to_str()
        .expect("Invalid Cache-Control header");
    let directives: Vec<_> = cache_header.split(", ").collect();
    assert_eq!(directives.len(), 3);
    assert_eq!(directives[0], "private");

    /// Parse a numeric value of a header directive into a u32
    ///
    /// E.g.:
    /// find_directive(&vec!["private", "max-age=5"], "max-age") -> 5
    fn find_directive(directives: &[&str], name: &str) -> u32 {
        let re = Regex::new(&format!(r"{}=(\d+)", name)).unwrap();
        directives
            .iter()
            .filter_map(|directive| {
                re.captures(directive)
                    .and_then(|captures| captures.get(1))
                    .and_then(|value| value.as_str().parse::<u32>().ok())
            })
            .next()
            .unwrap()
    }

    assert!(directives
        .iter()
        .any(|directive| directive.starts_with("max-age=")));
    let max_age = find_directive(&directives, "max-age");
    assert!(max_age > 0);
    // less than tiles_ttl plus jitter
    assert!(max_age < settings.tiles_ttl * 2);

    assert!(directives
        .iter()
        .any(|directive| directive.starts_with("stale-if-error=")));
    let stale_if_error = find_directive(&directives, "stale-if-error");
    assert!(stale_if_error > settings.tiles_ttl);
    // less than fallback_tiles_ttl plus jitter
    assert!(stale_if_error < settings.tiles_fallback_ttl * 2);

    let result: Value = test::read_body_json(resp).await;
    let tiles = result["tiles"].as_array().expect("!tiles.is_array()");
    assert_eq!(tiles.len(), 3);
}

#[actix_web::test]
async fn fallback_on_error() {
    let mut adm = init_mock_adm(MOCK_RESPONSE1.to_owned());
    let tiles_ttl = 2;
    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url.clone(),
        adm_settings: AdmFilter::advertisers_to_string(advertiser_filters()),
        location_test_header: Some("x-test-location".to_owned()),
        tiles_ttl,
        ..get_test_settings()
    };
    let (app, spy) = init_app_with_spy!(settings).await;

    // Load the cache
    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .insert_header(("X-Forwarded-For", TEST_ADDR))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get("Cache-Control").is_some());

    // Set adM to return an error then trigger a refresh (as the tiles expired)
    adm.set_response_error();
    rt::time::sleep(Duration::from_secs(tiles_ttl as u64)).await;
    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .insert_header(("X-Forwarded-For", TEST_ADDR))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let cache_header = resp
        .headers()
        .get("Cache-Control")
        .expect("No Cache-Control header")
        .to_str()
        .expect("Invalid Cache-Control header");
    let directives: Vec<_> = cache_header.split(", ").collect();
    assert_eq!(directives[0], "private");
    // We should fall back on errors, so max-age=0
    assert!(directives
        .iter()
        .any(|directive| directive.starts_with("max-age=0")));
    assert!(directives
        .iter()
        .any(|directive| directive.starts_with("stale-if-error=")));

    let metrics: Vec<_> = find_metrics(&spy, &["contile.tiles."])
        .into_iter()
        .map(|m| m.split_once('|').unwrap().0.to_owned())
        .collect();
    assert_eq!(
        metrics,
        vec![
            "contile.tiles.get:1",
            "contile.tiles.adm.request:1",
            "contile.tiles.adm.response.tiles_count:4",
            "contile.tiles.get:1",
            "contile.tiles.adm.request:1",
            "contile.tiles.get.error:1"
        ]
    );
}

#[actix_web::test]
async fn no_sov() {
    let adm = init_mock_adm(MOCK_RESPONSE1.to_owned());

    let mut settings = Settings {
        adm_endpoint_url: adm.endpoint_url,
        sov_source: "gs://bad.bucket".to_owned(),
        ..get_test_settings()
    };
    let app = init_app!(settings).await;

    let req = test::TestRequest::get()
        .uri("/v1/tiles")
        .insert_header((header::USER_AGENT, UA_91))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}
