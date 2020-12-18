//! Request header/body/query extractors
//!
//! Handles ensuring the header's, body, and query parameters are correct, extraction to
//! relevant types, and failing correctly with the appropriate errors if issues arise.
use actix_web::{dev::Payload, http::header, web, Error, FromRequest, HttpRequest};
use futures::future::{FutureExt, LocalBoxFuture};
use serde::Deserialize;

use crate::error::HandlerErrorKind;

const VALID_PLACEMENTS: &[&str] = &["urlbar", "newtab", "search"];

#[derive(Debug, Deserialize)]
pub struct TilesParams {
    country: String,
    placement: String,
}

#[derive(Debug)]
pub struct TilesRequest {
    pub country: String,
    pub placement: String,
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
                .ok_or_else(|| {
                    HandlerErrorKind::Validation("Missing User-Agent Header".to_owned())
                })?
                .to_str()
                .map_err(|e| {
                    HandlerErrorKind::Validation(format!("Invalid User-Agent Header: {}", e))
                })?;

            let params = web::Query::<TilesParams>::from_request(&req, &mut Payload::None).await?;
            let placement = params.placement.to_lowercase();
            if !validate_placement(&placement) {
                Err(HandlerErrorKind::Validation(
                    "Invalid placement parameter".to_owned(),
                ))?;
            }

            Ok(Self {
                country: params.country.to_uppercase(),
                placement,
                ua: ua.to_owned(),
            })
        }
        .boxed_local()
    }
}

fn validate_placement(placement: &str) -> bool {
    VALID_PLACEMENTS.contains(&placement)
}
