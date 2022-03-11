//! Provide a useful, standardized way to pass meta information for
//! Sentry and Metrics.
//!
use core::cell::RefMut;
use std::collections::{BTreeMap, HashMap};

use actix_http::Extensions;
use actix_web::{
    dev::{Payload, RequestHead},
    http::header::USER_AGENT,
    Error, FromRequest, HttpRequest,
};
use futures::future;
use futures::future::Ready;
use serde::{
    ser::{SerializeMap, Serializer},
    Serialize,
};
use serde_json::value::Value;
use slog::{Key, Record, KV};

use crate::{settings::Settings, web::get_device_info};

/// Tags are a set of meta information passed along with sentry errors and metrics.
///
/// Not all tags are distributed out. `tags` are searchable and may cause cardinality issues.
/// `extra` are not searchable, but may not be sent to [crate::metrics::Metrics].
#[derive(Clone, Debug, Default)]
pub struct Tags {
    // All tags (both metric and sentry)
    pub tags: HashMap<String, String>,
    // Sentry only "extra" data.
    pub extra: HashMap<String, String>,
    // metric only supplemental tags.
    pub metric: HashMap<String, String>,
}

impl Serialize for Tags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_map(Some(self.tags.len()))?;
        for tag in self.tags.clone() {
            if !tag.1.is_empty() {
                seq.serialize_entry(&tag.0, &tag.1)?;
            }
        }
        seq.end()
    }
}

impl Tags {
    pub fn from_head(req_head: &RequestHead, settings: &Settings) -> Self {
        // Return an Option<> type because the later consumers (HandlerErrors) presume that
        // tags are optional and wrapped by an Option<> type.
        let mut tags = HashMap::new();
        let mut extra = HashMap::new();
        if let Some(ua) = req_head.headers().get(USER_AGENT) {
            if let Ok(uas) = ua.to_str() {
                if let Ok(device_info) = get_device_info(uas) {
                    tags.insert("ua.os.family".to_owned(), device_info.os_family.to_string());
                    tags.insert(
                        "ua.form_factor".to_owned(),
                        device_info.form_factor.to_string(),
                    );
                }
                extra.insert("ua".to_owned(), uas.to_string());
            }
        }
        if let Some(tracer) = settings.trace_header.clone() {
            if let Some(header) = req_head.headers().get(tracer) {
                if let Ok(val) = header.to_str() {
                    if !val.is_empty() {
                        extra.insert("header.trace".to_owned(), val.to_owned());
                    }
                }
            }
        }
        tags.insert("uri.method".to_owned(), req_head.method.to_string());
        // `uri.path` causes too much cardinality for influx but keep it in
        // extra for sentry
        extra.insert("uri.path".to_owned(), req_head.uri.to_string());
        Tags {
            tags,
            extra,
            metric: HashMap::new(),
        }
    }
}

impl From<HttpRequest> for Tags {
    fn from(request: HttpRequest) -> Self {
        let settings = (&request).into();
        match request.extensions().get::<Self>() {
            Some(v) => v.clone(),
            None => Tags::from_head(request.head(), settings),
        }
    }
}

/// Convenience function to bulk load `extra`
impl Tags {
    pub fn from_extra(map: Vec<(&'static str, String)>) -> Self {
        let mut extra = HashMap::new();
        for (key, val) in map {
            extra.insert(key.to_owned(), val);
        }
        Self {
            tags: HashMap::new(),
            extra,
            metric: HashMap::new(),
        }
    }
}

// Tags are extra data to be recorded in metric and logging calls.
/// If additional tags are required or desired, you will need to add them to the
/// mutable extensions, e.g.
/// ```compile_fail
///      use contile::tags::Tags;
///
///      let mut tags = Tags::default();
///      tags.add_tag("SomeLabel", "whatever");
///      tags.commit(&mut request.extensions_mut());
/// ```
impl Tags {
    /// Generate a new Tag struct from a Hash of values.
    pub fn with_tags(tags: HashMap<String, String>) -> Tags {
        if tags.is_empty() {
            return Tags::default();
        }
        Tags {
            tags,
            extra: HashMap::new(),
            metric: HashMap::new(),
        }
    }

    /// Add an element to the "extra" data.
    ///
    /// Extra data is non-key storage used by sentry. It is not
    /// distributed to metrics.
    pub fn add_extra(&mut self, key: &str, value: &str) {
        if !value.is_empty() {
            self.extra.insert(key.to_owned(), value.to_owned());
        }
    }

    /// Add an element to the "tag" data.
    ///
    /// Tag data is keyed info. Be careful to not include too many
    /// unique values here otherwise you run the risk of excess
    /// cardinality.
    pub fn add_tag(&mut self, key: &str, value: &str) {
        if !value.is_empty() {
            self.tags.insert(key.to_owned(), value.to_owned());
        }
    }

    /// Add an element to the "extra" data.
    ///
    /// Extra data is non-key storage used by sentry. It is not
    /// distributed to metrics.
    pub fn add_metric(&mut self, key: &str, value: &str) {
        if !value.is_empty() {
            self.metric.insert(key.to_owned(), value.to_owned());
        }
    }

    /// Get a tag value.
    pub fn get(&self, label: &str) -> String {
        let none = "None".to_owned();
        self.tags.get(label).map(String::from).unwrap_or(none)
    }

    /// Extend the current tag set using another tag set.
    ///
    /// Useful for collating tags before distribution.
    pub fn extend(&mut self, tags: Self) {
        self.tags.extend(tags.tags);
        self.extra.extend(tags.extra);
        self.metric.extend(tags.metric);
    }

    /// Convert tag hash to a Binary Tree map (used by cadence and sentry)
    pub fn tag_tree(self) -> BTreeMap<String, String> {
        self.tags.into_iter().collect()
    }

    /// Convert extra hash to a Binary Tree map (used by cadence and sentry)
    pub fn extra_tree(self) -> BTreeMap<String, Value> {
        self.extra
            .into_iter()
            .map(|(k, v)| (k, Value::from(v)))
            .collect()
    }

    /// Write the current tag info to the Extensions.
    ///
    /// Actix provides extensions for requests and responses. These allow
    /// for arbitrary data to be stored, however note that these are
    /// separate, and that `response.request()` is not `request`.
    pub fn commit(self, exts: &mut RefMut<'_, Extensions>) {
        match exts.get_mut::<Tags>() {
            Some(t) => t.extend(self),
            None => exts.insert(self),
        }
    }
}

impl FromRequest for Tags {
    type Config = ();
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let settings = req.into();
        let tags = {
            let exts = req.extensions();
            match exts.get::<Tags>() {
                Some(t) => t.clone(),
                None => Tags::from_head(req.head(), settings),
            }
        };

        future::ok(tags)
    }
}

impl From<Tags> for BTreeMap<String, String> {
    fn from(tags: Tags) -> Self {
        tags.tags.into_iter().collect()
    }
}

impl KV for Tags {
    fn serialize(&self, _rec: &Record<'_>, serializer: &mut dyn slog::Serializer) -> slog::Result {
        for (key, val) in &self.tags {
            serializer.emit_str(Key::from(key.clone()), val)?;
        }
        Ok(())
    }
}
