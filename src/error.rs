use backtrace::Backtrace;
use std::error::Error;
use std::fmt;
use std::result;

use actix_web::{
    dev::{HttpResponseBuilder, ServiceResponse},
    error::ResponseError,
    http::StatusCode,
    middleware::errhandlers::ErrorHandlerResponse,
    HttpResponse, Result,
};
use thiserror::Error;

// pub type Result<T> = result::Result<T, HandlerError>;

pub type HandlerResult<T> = result::Result<T, HandlerError>;

#[derive(Debug)]
pub struct HandlerError {
    kind: HandlerErrorKind,
    backtrace: Backtrace,
}

#[derive(Clone, Eq, PartialEq, Debug, Error)]
pub enum HandlerErrorKind {
    #[error("General error: {:?}", _0)]
    GeneralError(String),
    #[error("Internal error: {:?}", _0)]
    InternalError(String),
}

impl HandlerErrorKind {
    /// Return a response Status to be rendered for an error
    pub fn http_status(&self) -> StatusCode {
        match self {
            // HandlerErrorKind::NotFound => Status::NotFound,
            HandlerErrorKind::InternalError(_) | HandlerErrorKind::GeneralError(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            } // _ => StatusCode::UNAUTHORIZED,
        }
    }

    /// Return a unique errno code
    pub fn errno(&self) -> i32 {
        match self {
            HandlerErrorKind::InternalError(_) => 510,
            HandlerErrorKind::GeneralError(_) => 500,
        }
    }

    /*
    // Optionally record metric for certain states
    pub fn on_response(&self, state: &ServerState) {
        if self.is_conflict() {
            Metrics::from(state).incr("storage.confict")
        }
    }
    */
}

impl ResponseError for HandlerErrorKind {
    fn error_response(&self) -> HttpResponse {
        let err = HandlerError::from(self.clone());
        err.error_response()
    }
}

impl HandlerError {
    pub fn kind(&self) -> &HandlerErrorKind {
        &self.kind
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
        write!(f, "Error: {}\nBacktrace:\n{:?}", self.kind, self.backtrace)?;

        // Go down the chain of errors
        let mut error: &dyn Error = &self.kind;
        while let Some(source) = error.source() {
            write!(f, "\n\nCaused by: {}", source)?;
            error = source;
        }

        Ok(())
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
}
