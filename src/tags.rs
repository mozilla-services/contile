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
use woothee::parser::{Parser, WootheeResult};

use crate::settings::Settings;

/*
/// List of valid user-agent attributes to keep, anything not in this
/// list is considered 'Other'. We log the user-agent on connect always
/// to retain the full string, but for DD more tags are expensive so we
/// limit to these.
/// Note: We currently limit to only Firefox UA.
//
// const VALID_UA_BROWSER: &[&str] = &["Chrome", "Firefox", "Safari", "Opera"];
*/

/// See dataset.rs in https://github.com/woothee/woothee-rust for the
/// full list (WootheeResult's 'os' field may fall back to its 'name'
/// field). Windows has many values and we only care that its Windows
const VALID_UA_OS: &[&str] = &["Firefox OS", "Linux", "Mac OSX"];

/// Primative User Agent parser.
///
/// We only care about a subset of the results for this (to avoid cardinality with
/// metrics and logging).
pub fn parse_user_agent(agent: &str) -> (WootheeResult<'_>, &str) {
    let parser = Parser::new();
    let wresult = parser.parse(&agent).unwrap_or_else(|| WootheeResult {
        name: "",
        category: "",
        os: "",
        os_version: "".into(),
        browser_type: "",
        version: "",
        vendor: "",
    });

    // Determine a base os/browser for metrics' tags
    let metrics_os = if wresult.os.starts_with("Windows") {
        "Windows"
    } else if VALID_UA_OS.contains(&wresult.os) {
        wresult.os
    } else {
        "Other"
    };
    // We currently limit to only Firefox UA.
    /*
    let metrics_browser = if VALID_UA_BROWSER.contains(&wresult.name) {
        wresult.name
    } else {
        "Other"
    };
    */
    (wresult, metrics_os)
}

/// Tags are a set of meta information passed along with sentry errors and metrics.
///
/// Not all tags are distributed out. `tags` are searchable and may cause cardinality issues.
/// `extra` are not searchable, but may not be sent to [crate::metrics::Metrics].
#[derive(Clone, Debug)]
pub struct Tags {
    pub tags: HashMap<String, String>,
    pub extra: HashMap<String, String>,
}

impl Default for Tags {
    fn default() -> Tags {
        Tags {
            tags: HashMap::new(),
            extra: HashMap::new(),
        }
    }
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

/// Insert a into a hashmap only if the `val` is not empty.
fn insert_if_not_empty(label: &str, val: &str, tags: &mut HashMap<String, String>) {
    if !val.is_empty() {
        tags.insert(label.to_owned(), val.to_owned());
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
                // if you wanted to parse out the user agent using some out-of-scope user agent parser like woothee
                let (ua_result, metrics_os) = parse_user_agent(uas);
                insert_if_not_empty("ua.os.family", metrics_os, &mut tags);
                insert_if_not_empty("ua.os.ver", &ua_result.os_version.to_owned(), &mut tags);
                insert_if_not_empty("ua.browser.ver", ua_result.version, &mut tags);
                extra.insert("ua".to_owned(), uas.to_string());
            }
        }
        if let Some(tracer) = settings.trace_header.clone() {
            if let Some(header) = req_head.headers().get(tracer) {
                insert_if_not_empty(
                    "header.trace",
                    header.to_str().unwrap_or_default(),
                    &mut tags,
                );
            }
        }
        tags.insert("uri.method".to_owned(), req_head.method.to_string());
        // `uri.path` causes too much cardinality for influx but keep it in
        // extra for sentry
        extra.insert("uri.path".to_owned(), req_head.uri.to_string());
        Tags { tags, extra }
    }
}

impl From<HttpRequest> for Tags {
    fn from(request: HttpRequest) -> Self {
        //let settings = &Settings::from(&request);
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
    }

    /// Convert tag hash to a Binary Tree map (used by cadence and sentry)
    pub fn tag_tree(self) -> BTreeMap<String, String> {
        let mut result = BTreeMap::new();

        for (k, v) in self.tags {
            result.insert(k.clone(), v.clone());
        }
        result
    }

    /// Convert extra hash to a Binary Tree map (used by cadence and sentry)
    pub fn extra_tree(self) -> BTreeMap<String, Value> {
        let mut result = BTreeMap::new();

        for (k, v) in self.extra {
            result.insert(k.clone(), Value::from(v));
        }
        result
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
        //let settings = Settings::from(req);
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
        let mut result = BTreeMap::new();

        for (k, v) in tags.tags {
            result.insert(k.clone(), v.clone());
        }

        result
    }
}

impl KV for Tags {
    fn serialize(&self, _rec: &Record<'_>, serializer: &mut dyn slog::Serializer) -> slog::Result {
        for (key, val) in &self.tags {
            serializer.emit_str(Key::from(key.clone()), &val)?;
        }
        Ok(())
    }
}
