//! Request header/body/query extractors
//!
//! Handles ensuring the header's, body, and query parameters are correct, extraction to
//! relevant types, and failing correctly with the appropriate errors if issues arise.

use actix_web::{
    dev::Payload,
    http::header::{HeaderValue, USER_AGENT},
    Error, FromRequest, HttpRequest,
};
use futures::future::{self, FutureExt, LocalBoxFuture};
use lazy_static::lazy_static;

use crate::{
    metrics::Metrics,
    web::user_agent::{get_device_info, DeviceInfo},
};

lazy_static! {
    static ref EMPTY_HEADER: HeaderValue = HeaderValue::from_static("");
}

impl FromRequest for DeviceInfo {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        async move {
            let ua = req
                .headers()
                .get(USER_AGENT)
                .unwrap_or(&EMPTY_HEADER)
                .to_str()
                .unwrap_or_default();
            Ok(get_device_info(ua)?)
        }
        .boxed_local()
    }
}

impl FromRequest for Metrics {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        future::ok(Metrics::from(req)).boxed_local()
    }
}
