//! Request header/body/query extractors
//!
//! Handles ensuring the header's, body, and query parameters are correct, extraction to
//! relevant types, and failing correctly with the appropriate errors if issues arise.
use std::net::{IpAddr, SocketAddr};

use actix_web::{
    dev::Payload,
    http::header,
    http::{HeaderName, HeaderValue},
    web, Error, FromRequest, HttpRequest,
};
use futures::future::{self, FutureExt, LocalBoxFuture};
use lazy_static::lazy_static;

use crate::{
    metrics::Metrics,
    server::{location::LocationResult, ServerState},
    web::user_agent::{get_device_info, DeviceInfo},
};

lazy_static! {
    static ref EMPTY_HEADER: HeaderValue = HeaderValue::from_static("");
    static ref X_FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");
}

impl FromRequest for DeviceInfo {
    type Config = ();
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        async move {
            let ua = req
                .headers()
                .get(header::USER_AGENT)
                .unwrap_or(&EMPTY_HEADER)
                .to_str()
                .unwrap_or_default();
            Ok(get_device_info(ua)?)
        }
        .boxed_local()
    }
}

impl FromRequest for LocationResult {
    type Config = ();
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        async move {
            let state = req.app_data::<web::Data<ServerState>>().expect("No State!");
            let settings = &state.settings;
            let metrics = Metrics::from(&req);

            let mut addr = None;
            if let Some(header) = req.headers().get(&*X_FORWARDED_FOR) {
                if let Ok(value) = header.to_str() {
                    // Expect a typical X-Forwarded-For where the first address is the
                    // client's, the front ends should ensure this
                    addr = value
                        .split(',')
                        .next()
                        .map(|addr| addr.trim())
                        .and_then(|addr| {
                            // Fallback to parsing as SocketAddr for when a port
                            // number's included
                            addr.parse::<IpAddr>()
                                .or_else(|_| addr.parse::<SocketAddr>().map(|socket| socket.ip()))
                                .ok()
                        });
                }
            }

            if let Some(addr) = addr {
                if state.mmdb.is_available() {
                    let result = state
                        .mmdb
                        .mmdb_locate(addr, &["en".to_owned()], &metrics)
                        .await?;
                    if let Some(location) = result {
                        return Ok(location);
                    }
                }
            } else {
                metrics.incr("location.unknown.ip");
            }
            Ok(LocationResult::from_header(req.head(), settings, &metrics))
        }
        .boxed_local()
    }
}

impl FromRequest for Metrics {
    type Config = ();
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        future::ok(Metrics::from(req)).boxed_local()
    }
}
