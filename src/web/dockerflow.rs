//! Mozilla Ops dockerflow endpoints.
//!
//! These are common endpoints that are used for process management. These include
//! * `__heartbeat__` - return process status
//! * `__lbheartbeat__` - used for load balancer availability
//! * `__version__` - return the current process version (and github commit)

use std::collections::HashMap;

use actix_http::http::HeaderValue;
use actix_web::{dev::Payload, web, FromRequest, HttpRequest, HttpResponse};
use actix_web_location::Location;
use serde::Deserialize;
use serde_json::Value;

use crate::{error::HandlerError, server::ServerState};

/// Well Known DockerFlow commands for Ops callbacks
pub const DOCKER_FLOW_ENDPOINTS: [&str; 4] = [
    "/__heartbeat__",
    "/__lbheartbeat__",
    "/__version__",
    "/__error__",
];

/// Handles required Dockerflow Endpoints
pub fn service(config: &mut web::ServiceConfig) {
    config
        .service(web::resource("/__lbheartbeat__").route(web::get().to(lbheartbeat)))
        .service(web::resource("/__heartbeat__").route(web::get().to(heartbeat)))
        .service(web::resource("/__version__").route(web::get().to(version)))
        .service(web::resource("/__error__").route(web::get().to(test_error)))
        .service(web::resource("/__loc_test__").route(web::get().to(loc_test)))
        .service(web::resource("").route(web::get().to(document_boot)));
}

/// Used by the load balancer to indicate that the server can respond to
/// requests. Should just return OK.
fn lbheartbeat() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body("{}")
}

/// Return the contents of the `version.json` file created by CircleCI and stored
/// in the Docker root (or the TBD version stored in the Git repo).
fn version() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(include_str!("../../version.json"))
}

/// Returns a status message indicating the current state of the server
fn heartbeat() -> HttpResponse {
    let mut checklist = HashMap::new();
    checklist.insert(
        "version".to_owned(),
        Value::String(env!("CARGO_PKG_VERSION").to_owned()),
    );
    HttpResponse::Ok().json(checklist)
}

#[derive(Debug, Deserialize)]
pub struct ErrorParams {
    pub with_location: Option<bool>,
}

/// Returning an API error to test error handling
///
/// Optionally including location lookup information.
async fn test_error(
    req: HttpRequest,
    params: web::Query<ErrorParams>,
) -> Result<HttpResponse, HandlerError> {
    // generate an error for sentry.
    error!("Test Error");
    let mut err = HandlerError::internal("Oh Noes!");
    if matches!(params.with_location, Some(true)) {
        let location_info = match Location::from_request(&req, &mut Payload::None).await {
            Ok(location) => format!("{:#?}", location),
            Err(loce) => loce.to_string(),
        };
        err.tags.add_extra("location", &location_info);
    }
    Err(err)
}

async fn loc_test(req: HttpRequest) -> Result<HttpResponse, HandlerError> {
    let location_info = Location::from_request(&req, &mut Payload::None)
        .await
        .map_err(|e| HandlerError::internal(&e.to_string()))?;
    Ok(HttpResponse::Ok().body(format!(
        r#"{{"country":{:?}, "region": {:?}, "provider": {:?}, "ip":{:?}}}"#,
        location_info.country(),
        location_info.region.unwrap_or_else(|| "None".to_owned()),
        location_info.provider,
        req.headers()
            .get("X-FORWARDED-FOR")
            .unwrap_or(&HeaderValue::from_str("None").unwrap())
    )))
}

async fn document_boot(state: web::Data<ServerState>) -> Result<HttpResponse, HandlerError> {
    let settings = &state.settings;
    return Ok(HttpResponse::Found()
        .header("Location", settings.documentation_url.clone())
        .finish());
}
