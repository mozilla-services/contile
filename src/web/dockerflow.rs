//! Mozilla Ops dockerflow endpoints.
//!
//! These are common endpoints that are used for process management. These include
//! * `__heartbeat__` - return process status
//! * `__lbheartbeat__` - used for load balancer availability
//! * `__version__` - return the current process version (and github commit)

use std::collections::HashMap;

use actix_web::{web, HttpRequest, HttpResponse};
use serde_json::Value;

use crate::error::HandlerError;
use crate::server::ServerState;

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
        .service(web::resource("").route(web::get().to(document_boot)));
}

/// Used by the load balancer to indicate that the server can respond to
/// requests. Should just return OK.
fn lbheartbeat(_: HttpRequest) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body("{}")
}

/// Return the contents of the `version.json` file created by CircleCI and stored
/// in the Docker root (or the TBD version stored in the Git repo).
fn version(_: HttpRequest) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(include_str!("../../version.json"))
}

/// Returns a status message indicating the current state of the server
fn heartbeat(_: HttpRequest) -> HttpResponse {
    let mut checklist = HashMap::new();
    checklist.insert(
        "version".to_owned(),
        Value::String(env!("CARGO_PKG_VERSION").to_owned()),
    );
    HttpResponse::Ok().json(checklist)
}

/// Returning an API error to test error handling
async fn test_error(_: HttpRequest) -> Result<HttpResponse, HandlerError> {
    // generate an error for sentry.
    error!("Test Error");
    Err(HandlerError::internal("Oh Noes!"))
}

async fn document_boot(
    _: HttpRequest,
    state: web::Data<ServerState>,
) -> Result<HttpResponse, HandlerError> {
    let settings = &state.settings;
    return Ok(HttpResponse::SeeOther()
        .header("Location", settings.documentation_url.clone())
        .finish());
}
