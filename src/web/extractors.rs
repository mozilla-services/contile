//! Request header/body/query extractors
//!
//! Handles ensuring the header's, body, and query parameters are correct, extraction to
//! relevant types, and failing correctly with the appropriate errors if issues arise.
use actix_web::{
    dev::Payload, http::header, http::header::HeaderValue, web, Error, FromRequest, HttpRequest,
};
use futures::future::{self, FutureExt, LocalBoxFuture};
use lazy_static::lazy_static;
use serde::Deserialize;

use crate::{error::HandlerErrorKind, metrics::Metrics};

lazy_static! {
    static ref EMPTY_HEADER: HeaderValue = HeaderValue::from_static("");
}

const VALID_PLACEMENTS: &[&str] = &["urlbar", "newtab", "search"];

#[derive(Debug, Deserialize)]
pub struct TilesParams {
    country: Option<String>,
    placement: Option<String>,
}

#[derive(Debug)]
pub struct TilesRequest {
    pub country: Option<String>,
    pub placement: Option<String>,
    pub ua: String,
}

impl FromRequest for TilesRequest {
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

            let params = web::Query::<TilesParams>::from_request(&req, &mut Payload::None).await?;
            let placement = match &params.placement {
                Some(v) => {
                    let placement = v.to_lowercase();
                    if !validate_placement(&v) {
                        Err(HandlerErrorKind::Validation(
                            "Invalid placement parameter".to_owned(),
                        ))?;
                    };
                    Some(placement)
                }
                None => None,
            };
            Ok(Self {
                country: params.country.clone().map(|v| v.to_uppercase()),
                placement,
                ua: ua.to_owned(),
            })
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

fn validate_placement(placement: &str) -> bool {
    VALID_PLACEMENTS.contains(&placement)
}
