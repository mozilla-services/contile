//! Wrapper for sentry error reporting
//!
//! This sends [crate::error::HandlerError] events to Sentry

use std::{error::Error as StdError, rc::Rc};

use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    web::Data,
    Error, HttpMessage,
};
use cadence::CountedExt;
use futures::future::{self, LocalBoxFuture, TryFutureExt};
use sentry::protocol::Event;

use crate::{error::HandlerError, server::ServerState, settings::Settings, tags::Tags};

pub struct SentryWrapper;

impl Default for SentryWrapper {
    fn default() -> Self {
        Self
    }
}

impl<S, B> Transform<S, ServiceRequest> for SentryWrapper
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = SentryWrapperMiddleware<S>;
    type Future = LocalBoxFuture<'static, Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        Box::pin(future::ok(SentryWrapperMiddleware {
            service: Rc::new(service),
        }))
    }
}

#[derive(Debug)]
pub struct SentryWrapperMiddleware<S> {
    service: Rc<S>,
}

/// Report a [HandlerError] with [crate::tags::Tags] directly to sentry
pub fn report(err: &HandlerError, tags: &Tags) {
    report_event(event_from_error(err), tags)
}

/// Report an error with [crate::tags::Tags] and [Event] directly to sentry
///
/// And [Event] can be derived using `event_from_error(HandlerError)`
fn report_event(mut event: Event<'static>, tags: &Tags) {
    let tags = tags.clone();
    event.tags = tags.clone().tag_tree();
    event.extra = tags.extra_tree();
    trace!("Sentry: Sending error: {:?}", &event);
    sentry::capture_event(event);
}

impl<S, B> Service<ServiceRequest> for SentryWrapperMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, sreq: ServiceRequest) -> Self::Future {
        let settings: &Settings = (&sreq).into();
        let metrics = sreq
            .app_data::<Data<ServerState>>()
            .expect("No Server state found")
            .metrics
            .clone();
        let mut tags = Tags::from_head(sreq.head(), settings);
        sreq.extensions_mut().insert(tags.clone());

        Box::pin(self.service.call(sreq).and_then(move |mut sresp| {
            // handed an actix_error::error::Error;
            // Fetch out the tags (in case any have been added.) NOTE: request extensions
            // are NOT automatically passed to responses. You need to check both.
            if let Some(t) = sresp.request().extensions().get::<Tags>() {
                trace!("Sentry: found tags in request: {:?}", &t.tags);
                tags.extend(t.clone());
            };
            if let Some(t) = sresp.response().extensions().get::<Tags>() {
                trace!("Sentry: found tags in response: {:?}", &t.tags);
                tags.extend(t.clone());
            };
            match sresp.response().error() {
                None => {
                    // Middleware errors are eaten by current versions of Actix. Errors are now added
                    // to the extensions. Need to check both for any errors and report them.
                    if let Some(events) = sresp
                        .request()
                        .extensions_mut()
                        .remove::<Vec<Event<'static>>>()
                    {
                        for event in events {
                            trace!("Sentry: found an error stored in request: {:?}", &event);
                            report_event(event, &tags);
                        }
                    }
                    if let Some(events) = sresp
                        .response_mut()
                        .extensions_mut()
                        .remove::<Vec<Event<'static>>>()
                    {
                        for event in events {
                            trace!("Sentry: Found an error stored in response: {:?}", &event);
                            report_event(event, &tags);
                        }
                    }
                }
                Some(e) => {
                    if let Some(herr) = e.as_error::<HandlerError>() {
                        if herr.kind().is_sentry_event() {
                            tags.extend(herr.tags.as_ref().clone());
                            report(herr, &tags);
                        } else if let Some(label) = herr.kind().metric_label() {
                            metrics.incr_with_tags(label).send()
                        }
                    }
                }
            }
            future::ok(sresp)
        }))
    }
}

/// Custom `sentry::event_from_error` for `HandlerError`
///
/// `sentry::event_from_error` can't access `std::Error` backtraces as its
/// `backtrace()` method is currently Rust nightly only. This function works
/// against `HandlerError` instead to access its backtrace.
pub fn event_from_error(err: &HandlerError) -> Event<'static> {
    let mut exceptions = vec![exception_from_error_with_backtrace(err)];

    let mut source = err.source();
    while let Some(err) = source {
        let exception = if let Some(err) = err.downcast_ref() {
            exception_from_error_with_backtrace(err)
        } else {
            exception_from_error(err)
        };
        exceptions.push(exception);
        source = err.source();
    }

    exceptions.reverse();
    Event {
        exception: exceptions.into(),
        level: sentry::protocol::Level::Error,
        ..Default::default()
    }
}

/// Custom `exception_from_error` support function for `HandlerError`
///
/// Based moreso on sentry_failure's `exception_from_single_fail`.
fn exception_from_error_with_backtrace(err: &HandlerError) -> sentry::protocol::Exception {
    let mut exception = exception_from_error(err);
    // format the stack trace with alternate debug to get addresses
    let bt = format!("{:#?}", err.backtrace);
    exception.stacktrace = sentry_backtrace::parse_stacktrace(&bt);
    exception
}

/// Exact copy of sentry's unfortunately private `exception_from_error`
fn exception_from_error<E: StdError + ?Sized>(err: &E) -> sentry::protocol::Exception {
    let dbg = format!("{:?}", err);
    sentry::protocol::Exception {
        ty: sentry::parse_type_from_debug(&dbg).to_owned(),
        value: Some(err.to_string()),
        ..Default::default()
    }
}
