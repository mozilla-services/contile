//! Common errors
use backtrace::Backtrace;
use std::error::Error;
use std::fmt;
use std::result;

use actix_web::http::uri::InvalidUri;

use actix_web::{
    dev::{HttpResponseBuilder, ServiceResponse},
    error::ResponseError,
    http::StatusCode,
    middleware::errhandlers::ErrorHandlerResponse,
    HttpResponse, Result,
};
use thiserror::Error;

use crate::tags::Tags;

/// The standard Result type for Contile (returns Error = [`HandlerError`])
pub type HandlerResult<T> = result::Result<T, HandlerError>;

/// The Standard Error for most of Contile
#[derive(Debug)]
pub struct HandlerError {
    kind: HandlerErrorKind,
    pub(crate) backtrace: Backtrace,
    pub tags: Tags,
}

/// The specific context types of HandlerError.
#[derive(Debug, Error)]
pub enum HandlerErrorKind {
    /// An unspecified General error, usually via an external service or crate
    #[error("General error: {:?}", _0)]
    General(String),

    /// A specific Internal error.
    #[error("Internal error: {:?}", _0)]
    Internal(String),

    /// An error fetching information from ADM
    #[error("Reqwest error: {:?}", _0)]
    Reqwest(#[from] reqwest::Error),

    /// An error validating the tile information recv'd from ADM
    #[error("Validation error: {:?}", _0)]
    Validation(String),

    /// A tile contained an invalid host url
    #[error("Invalid {} Host: {:?}", _0, _1)]
    InvalidHost(&'static str, String),

    /// A tile image is invalid
    #[error("Invalid Image: {:?}", _0)]
    BadImage(&'static str),

    /// A tile was from an unrecognized host
    #[error("Unexpected {} Host: {:?}", _0, _1)]
    UnexpectedHost(&'static str, String),

    /// A tile contained an unrecognized `advertiser_url` host
    #[error("Unexpected Advertiser: {:?}", _0)]
    UnexpectedAdvertiser(String),

    /// A tile was missing a host, or presented an unparsable one.
    #[error("Missing {} Host: {:?}", _0, _1)]
    MissingHost(&'static str, String),

    /// The Location information for the request could not be resolved
    #[error("Location error: {:?}", _0)]
    Location(String),

    /// ADM returned an invalid or unexpected response
    #[error("Bad Adm response: {:?}", _0)]
    BadAdmResponse(String),

    /// ADM Servers returned an error
    #[error("Adm Server Error")]
    AdmServerError(),

    /// Invalid UserAgent request
    #[error("Invalid user agent")]
    InvalidUA(),

    #[error("Cloud Storage error: {}", _0)]
    CloudStorage(#[from] cloud_storage::Error),
}

/// A set of Error Context utilities
impl HandlerErrorKind {
    /// Return a response Status to be rendered for an error
    pub fn http_status(&self) -> StatusCode {
        match self {
            HandlerErrorKind::Validation(_) => StatusCode::BAD_REQUEST,
            HandlerErrorKind::AdmServerError() => StatusCode::SERVICE_UNAVAILABLE,
            HandlerErrorKind::BadAdmResponse(_)
            | HandlerErrorKind::InvalidHost(_, _)
            | HandlerErrorKind::UnexpectedHost(_, _)
            | HandlerErrorKind::BadImage(_)
            | HandlerErrorKind::CloudStorage(_) => StatusCode::BAD_GATEWAY,
            &HandlerErrorKind::InvalidUA() => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Return a unique errno code
    pub fn errno(&self) -> i32 {
        match self {
            HandlerErrorKind::General(_) => 500,
            HandlerErrorKind::Internal(_) => 510,
            HandlerErrorKind::Reqwest(_) => 520,
            HandlerErrorKind::BadAdmResponse(_) => 521,
            HandlerErrorKind::AdmServerError() => 522,
            HandlerErrorKind::Location(_) => 530,
            HandlerErrorKind::Validation(_) => 600,
            HandlerErrorKind::InvalidHost(_, _) => 601,
            HandlerErrorKind::UnexpectedHost(_, _) => 602,
            HandlerErrorKind::MissingHost(_, _) => 603,
            HandlerErrorKind::UnexpectedAdvertiser(_) => 604,
            HandlerErrorKind::BadImage(_) => 605,
            HandlerErrorKind::CloudStorage(_) => 620,
            HandlerErrorKind::InvalidUA() => 700,
        }
    }

    /*
    // Optionally record metric for certain states
    pub fn is_reportable(&self) -> bool {
        match self {
            _ => true
        }
    }
    */
}

impl From<HandlerErrorKind> for actix_web::Error {
    fn from(kind: HandlerErrorKind) -> Self {
        let error: HandlerError = kind.into();
        error.into()
    }
}

impl From<InvalidUri> for HandlerErrorKind {
    fn from(err: InvalidUri) -> Self {
        HandlerErrorKind::Internal(format!("Invalid URL: {:?}", err))
    }
}

impl HandlerError {
    pub fn kind(&self) -> &HandlerErrorKind {
        &self.kind
    }

    pub fn internal(msg: &str) -> Self {
        HandlerErrorKind::Internal(msg.to_owned()).into()
    }
}

impl Error for HandlerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.kind.source()
    }
}

impl HandlerError {
    pub fn render_404<B>(res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
        // Replace the outbound error message with our own.
        let resp = HttpResponseBuilder::new(StatusCode::NOT_FOUND).json(0);
        Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
            res.request().clone(),
            resp.into_body(),
        )))
    }
}

impl<T> From<T> for HandlerError
where
    HandlerErrorKind: From<T>,
{
    fn from(item: T) -> Self {
        HandlerError {
            kind: HandlerErrorKind::from(item),
            backtrace: Backtrace::new(),
            tags: Tags::default(),
        }
    }
}

impl From<HandlerError> for HttpResponse {
    fn from(inner: HandlerError) -> Self {
        ResponseError::error_response(&inner)
    }
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl ResponseError for HandlerError {
    fn error_response(&self) -> HttpResponse {
        // To return a descriptive error response, this would work. We do not
        // unfortunately do that so that we can retain Sync 1.1 backwards compatibility
        // as the Python one does.
        // HttpResponse::build(self.status).json(self)
        //
        // So instead we translate our error to a backwards compatible one
        let mut resp = HttpResponse::build(self.status_code());
        resp.json(self.kind().errno() as i32)
    }

    fn status_code(&self) -> StatusCode {
        self.kind().http_status()
    }
}
