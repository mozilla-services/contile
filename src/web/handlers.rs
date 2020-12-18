//! API Handlers
use std::collections::HashMap;

use actix_web::{Error, HttpRequest, HttpResponse};
use serde_json::Value;

use crate::{
    error::{HandlerError, HandlerErrorKind},
    web::extractors::{HeartbeatRequest, TestErrorRequest},
};

pub const ONE_KB: f64 = 1024.0;

/** Returns a status message indicating the state of the current server
 *
 */
pub async fn heartbeat(_hb: HeartbeatRequest) -> Result<HttpResponse, Error> {
    let mut checklist = HashMap::new();
    checklist.insert(
        "version".to_owned(),
        Value::String(env!("CARGO_PKG_VERSION").to_owned()),
    );

    // Add optional values to checklist
    // checklist.insert("quota".to_owned(), serde_json::to_value(hb.quota)?);

    /*
    // Perform whatever additional checks you prefer
    match db.check().await {
        Ok(result) => {
            if result {
                checklist.insert("database".to_owned(), Value::from("Ok"));
            } else {
                checklist.insert("database".to_owned(), Value::from("Err"));
                checklist.insert(
                    "database_msg".to_owned(),
                    Value::from("check failed without error"),
                );
            };
            let status = if result { "Ok" } else { "Err" };
            checklist.insert("status".to_owned(), Value::from(status));

        }
        Err(e) => {
            error!("Heartbeat error: {:?}", e);
            checklist.insert("status".to_owned(), Value::from("Err"));
            checklist.insert("database".to_owned(), Value::from("Unknown"));
            return Ok(HttpResponse::ServiceUnavailable().json(checklist))
        }
    }
    */

    Ok(HttpResponse::Ok().json(checklist))
}

// try returning an API error
pub async fn test_error(
    _req: HttpRequest,
    _ter: TestErrorRequest,
) -> Result<HttpResponse, HandlerError> {
    // generate an error for sentry.

    // HandlerError will call the middleware layer to auto-append the tags.
    error!("Test Error");
    let err = HandlerError::from(HandlerErrorKind::InternalError("Oh Noes!".to_owned()));

    Err(err)
}
