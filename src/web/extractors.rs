//! Request header/body/query extractors
//!
//! Handles ensuring the header's, body, and query parameters are correct, extraction to
//! relevant types, and failing correctly with the appropriate errors if issues arise.
use actix_web::{
    dev::Payload, http::header::HeaderMap, web::Data, Error, FromRequest, HttpRequest,
};

use futures::future::{self, FutureExt, LocalBoxFuture};

use serde::Deserialize;

use crate::error::HandlerErrorKind;
use crate::server::ServerState;

#[derive(Deserialize)]
pub struct UidParam {
    #[allow(dead_code)] // Not really dead, but Rust can't see the deserialized use.
    uid: u64,
}

#[derive(Clone, Debug)]
pub struct HeartbeatRequest {
    pub headers: HeaderMap,
}

impl FromRequest for HeartbeatRequest {
    type Config = ();
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();

        async move {
            let headers = req.headers().clone();
            let _state = match req.app_data::<Data<ServerState>>() {
                Some(s) => s,
                None => {
                    error!("⚠️ Could not load the app state");
                    return Err(HandlerErrorKind::GeneralError("Bad state".to_owned()).into());
                }
            };
            Ok(HeartbeatRequest { headers })
        }
        .boxed_local()
    }
}

#[derive(Debug)]
pub struct TestErrorRequest {
    pub headers: HeaderMap,
}

impl FromRequest for TestErrorRequest {
    type Config = ();
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let headers = req.headers().clone();

        Box::pin(future::ok(TestErrorRequest { headers }))
    }
}
