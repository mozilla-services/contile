use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
    fmt::Debug,
    fs::read_to_string,
    path::Path,
};

use config::ConfigError;
use serde::{ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::AdmFilter;
use crate::{
    error::{HandlerError, HandlerErrorKind, HandlerResult},
    settings::Settings,
    web::DeviceInfo,
};

/// The AdvertiserUrlFilter describes the filtering rule for the `advertiser_url`.
///
/// Each rule consists of a host and optionally a list of PathFilters.
///
/// Examples:
///
/// ```json
///     {
///         "host": "foo.com",
///         "paths": [
///             { "value": "/", "matching": "exact" },
///             { "value": "/bar/", "matching": "prefix" },
///             { "value": "/baz/spam/", "matching": "prefix" },
///         ]
///     }
/// ```
/// For each `advertiser_url` (assume its host is `host` and path is `path`),
/// the matching rule is defined as follows:
///
/// * Check if the host in the advertiser URL exactly matches with the `"host"`
///   value in this filter.  If not, this URL is rejected by this filter.
///   For example `https://foo.com` would match, however `https://www.foo.com`
///   would *not* match and would be rejected. If you wish to include both
///   hosts, you will need to duplicate the `"paths"`.
/// * If the host matches, and there is no `"paths"` specified in this filter,
///   then the URL is accepted by this filter.
/// * If the `"paths"` filter list is present, then proceed with path filtering.
///   There are two matching strategies:
///   * `"exact"` for exact path matching, which compares the `"path"`
///     character-by-character with the `"value"` filed of this path filter.
///   * "prefix" for prefix path matching, which checks if the `value` is a
///     prefix of the `"path"`. Note that we always make sure `"path"` and `"value"`
///     are compared with the trailing '/' to avoid the accidental
///     matches. In particular, when loading filters from the settings file,
///     Contile will panic if it detects that a prefix filter doesn't have
///     the trailing '/' in the `"value"`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AdvertiserUrlFilter {
    pub(crate) host: String,
    #[serde(skip_serializing_if = "check_paths")]
    pub(crate) paths: Option<Vec<PathFilter>>,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PathMatching {
    Prefix,
    Exact,
}

fn check_paths(paths: &Option<Vec<PathFilter>>) -> bool {
    if let Some(s_paths) = paths {
        s_paths.is_empty()
    } else {
        true
    }
}

impl TryFrom<&str> for PathMatching {
    type Error = ConfigError;

    fn try_from(string: &str) -> Result<Self, Self::Error> {
        match string.to_lowercase().as_str() {
            "prefix" => Ok(Self::Prefix),
            "exact" => Ok(Self::Exact),
            _ => Err(ConfigError::Message(format!(
                "Invalid Path Filter Type {}",
                string
            ))),
        }
    }
}

impl From<PathMatching> for &'static str {
    fn from(pm: PathMatching) -> &'static str {
        match pm {
            PathMatching::Prefix => "prefix",
            PathMatching::Exact => "exact",
        }
    }
}

/// PathFilter describes how path filtering is conducted. See more details in
/// AdvertiserUrlFilter.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PathFilter {
    pub(crate) value: String,
    pub(crate) matching: PathMatching,
}

impl Default for PathFilter {
    fn default() -> Self {
        Self {
            value: "/".to_owned(),
            matching: PathMatching::Exact,
        }
    }
}

/// The AdmAdvertiserFilterSettings contain the settings for the various
/// ADM provided partners.
///
/// These are specified as a JSON formatted hash
/// that contains the components.
#[derive(Clone, Debug, Deserialize, Default, Serialize)]
pub struct AdmAdvertiserFilterSettings {
    pub(crate) countries: HashMap<String, Vec<AdvertiserUrlFilter>>,
    #[serde(default)]
    pub(crate) delete: bool,
}

pub fn break_hosts(host: String) -> Vec<String> {
    host.split('.').map(ToOwned::to_owned).collect()
}

fn make_host(split_host: &[String]) -> String {
    split_host.join(".")
}

/// Parse JSON:
/// ["example.com", "foo.net"]
/// into:
/// [["example", "com"], ["foo", "net"]]
fn deserialize_hosts<'de, D>(d: D) -> Result<Vec<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Deserialize::deserialize(d)
        .map(|hosts: Vec<String>| hosts.into_iter().map(break_hosts).collect())
}

/// Serialize:
/// [["example", "com"], ["foo", "net"]]
/// into:
/// ["example.com", "foo.net"]
fn serialize_hosts<S>(hosts: &[Vec<String>], s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let hosts: Vec<_> = hosts.iter().map(|v| make_host(v)).collect();
    let mut seq = s.serialize_seq(Some(hosts.len()))?;
    for host in hosts {
        seq.serialize_element(&host)?;
    }
    seq.end()
}

#[derive(Debug, Default, Clone)]
pub struct AdmPse {
    pub partner_id: String,
    pub sub1: String,
    pub endpoint: String,
}

/// ADM Partner/Sub1/Endpoint.
/// These change depending on the type of device requesting the tile.
///
/// Currently, we only need to check for two patterns "mobile" and "default",
/// but there's no guarantee that will always be the case. Hopefully this
/// pattern provides flexibility for future changes.
impl AdmPse {
    /// Return the information for a mobile connection
    pub fn mobile_from_settings(settings: &Settings) -> Self {
        let default = Self::default_from_settings(settings);
        AdmPse {
            partner_id: settings
                .adm_mobile_partner_id
                .clone()
                .unwrap_or(default.partner_id),
            sub1: settings.adm_mobile_sub1.clone().unwrap_or(default.sub1),
            endpoint: settings
                .adm_mobile_endpoint_url
                .clone()
                .unwrap_or(default.endpoint),
        }
    }

    /// Return the information for a generic connection
    pub fn default_from_settings(settings: &Settings) -> Self {
        AdmPse {
            partner_id: settings.adm_partner_id.clone().unwrap_or_default(),
            sub1: settings.adm_sub1.clone().unwrap_or_default(),
            endpoint: settings.adm_endpoint_url.clone(),
        }
    }

    /// Determine the correct type of information to return based on device info.
    pub fn appropriate_from_settings(device_info: &DeviceInfo, settings: &Settings) -> Self {
        if device_info.is_mobile() {
            return Self::mobile_from_settings(settings);
        }
        Self::default_from_settings(settings)
    }
}

#[derive(Clone, Debug, Deserialize, Default, Serialize)]
pub struct AdmDefaults {
    /// Required set of valid hosts and paths for the `advertiser_url`
    #[serde(default)]
    pub(crate) advertiser_urls: Vec<AdvertiserUrlFilter>,
    /// Optional set of valid hosts for the `impression_url`
    #[serde(
        deserialize_with = "deserialize_hosts",
        serialize_with = "serialize_hosts",
        default
    )]
    pub(crate) impression_hosts: Vec<Vec<String>>,
    /// Optional set of valid hosts for the `click_url`
    #[serde(
        deserialize_with = "deserialize_hosts",
        serialize_with = "serialize_hosts",
        default
    )]
    pub(crate) click_hosts: Vec<Vec<String>>,
    #[serde(
        deserialize_with = "deserialize_hosts",
        serialize_with = "serialize_hosts",
        default
    )]
    pub(crate) image_hosts: Vec<Vec<String>>,
    /// valid position for the tile
    pub(crate) position: Option<u8>,
    /// Optional set of valid countries for the tile (e.g ["US", "GB"])
    //#[serde(default)]
    //pub(crate) include_regions: Vec<String>,
    pub(crate) ignore_advertisers: Option<Vec<String>>,
    pub(crate) ignore_dmas: Option<Vec<u8>>,
}

/*
#[derive(Debug, Default, Clone)]
pub struct AdmFilterSettings {
    bucket: Option<url::Url>,
    pub advertisers: HashMap<String, AdmAdvertiserFilterSettings>,
}

impl Serialize for AdmFilterSettings {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_map(self.advertisers.clone())
    }
}
*/

/// Create AdmSettings from a string serialized JSON format
impl AdmFilter {
    /// Parse a JSON string containing the ADM settings. These will be generated by shepherd and
    /// would have a format similar to the following:
    /// ```json
    /// {
    ///     "Example": {
    ///         "US": [
    ///             {"host": "www.example.com",
    ///              "paths": [
    ///                 {"value": "/here",
    ///                  "matching": "exact"}
    ///              ]
    ///             }
    ///         ],
    ///        "MX": [
    ///             {"host": "www.example.mx",
    ///              "paths": [
    ///                 {"value": "/aqui",
    ///                  "matching": "exact"}
    ///              ]
    ///             }
    ///         ]
    ///     },
    ///     "Obsolete": {
    ///         "delete": True
    ///     }
    /// }
    /// ```
    /// See [AdmFilter] for details.
    ///
    /// The data can be read from a Google Cloud Storage bucket by passing a `gs://...` URL. The data will be read and
    /// updated later by the automatic bucket reader, so we skip processing of that for now.
    pub fn advertisers_from_string(
        settings_str: &str,
    ) -> Result<HashMap<String, AdmAdvertiserFilterSettings>, ConfigError> {
        // because of the unstructured JSON format, decode this by hand.
        // Note, Contile will only read this data. It will never write it so serialization is less important.
        //
        //
        if settings_str.is_empty() {
            return Err(ConfigError::Message(
                "No ADM settings string specified".to_string(),
            ));
        }
        let parsed: HashMap<String, Value> = serde_json::from_str(settings_str)
            .map_err(|e| ConfigError::Message(format!("ADM Settings parse error: {:?}", e)))?;
        let mut advertisers: HashMap<String, AdmAdvertiserFilterSettings> = HashMap::new();
        // first level `ADVERTISER: AdvertiserFilterSettings`
        for (advertiser, value) in parsed {
            // second level `COUNTRY:Vec<AdvertiserUrlFilter> | "delete": bool`
            let mut countries: HashMap<String, Vec<AdvertiserUrlFilter>> = HashMap::new();
            let mut delete = false;
            for (key, value) in value.as_object().ok_or_else(|| {
                ConfigError::Message(format!("Invalid advertiser info {}", &advertiser))
            })? {
                // Delete allows us to delete this advertiser. It may appear at the same
                // level as the country listings.
                if key.to_lowercase() == "delete" {
                    delete = value.as_bool().unwrap_or_default();
                    continue;
                }
                if key.len() > 2 {
                    warn!("Invalid country detected: {}", key);
                    continue;
                }
                // */
                let mut filters: Vec<AdvertiserUrlFilter> = Vec::new();
                // filters consist of a list of hashes
                for candidate in value.as_array().ok_or_else(|| {
                    ConfigError::Message(format!(
                        "Invalid country information for {}:{}",
                        &advertiser, &key
                    ))
                })? {
                    let mut filter = AdvertiserUrlFilter::default();
                    for (field, value) in candidate.as_object().ok_or_else(|| {
                        ConfigError::Message(format!(
                            "Invalid advertiser info for {}:{}",
                            &advertiser, &key
                        ))
                    })? {
                        match field.to_lowercase().as_str() {
                            "host" => {
                                filter.host = value
                                    .as_str()
                                    .ok_or_else(|| {
                                        ConfigError::Message(format!(
                                            "Invalid host for path declaration for {}:{}",
                                            &advertiser, &key
                                        ))
                                    })?
                                    .to_owned()
                            }
                            "paths" => {
                                let mut paths: Vec<PathFilter> = Vec::new();
                                for path_filter in value.as_array().ok_or_else(|| ConfigError::Message(format!("missing list of path filters for path declaration for {}:{}", &advertiser, &key)))? {
                                    let filter = path_filter.as_object().ok_or_else(|| ConfigError::Message(format!("Invalid path filter for path declaration for {}:{}", &advertiser, &key)))?;
                                    let pfilter = PathFilter {
                                        value: filter.get("value").ok_or_else(|| ConfigError::Message(format!("Missing 'value' for path declaration for {}:{}", &advertiser, &key)))?.as_str().unwrap_or_default().to_owned(),
                                        matching: PathMatching::try_from(filter.get("matching").ok_or_else(|| ConfigError::Message(format!("Missing 'matching' for path declaration for {}:{}", &advertiser, &key)))?.as_str().ok_or_else(|| ConfigError::Message(format!("Invalid string for 'matching' for path declaration for {}:{}", &advertiser, &key)))?).map_err(|_| ConfigError::Message(format!("Invalid string for 'matching' for path declaration for {}:{}", &advertiser, &key)))?,
                                    };
                                    if pfilter.value == *"" {
                                        return Err(ConfigError::Message(format!("Invalid or unparsable 'value' declaration for {}:{}", &advertiser, &key)));
                                    }
                                    if let PathMatching::Prefix = pfilter.matching {
                                        if !pfilter.value.ends_with('/'){
                                            return Err(ConfigError::Message(format!("Advertiser {:?} advertiser_urls contain invalid prefix PathFilter (missing trailing '/')", &filter)));
                                        }

                                    }

                                    paths.push(pfilter);
                                }
                                if !paths.is_empty() {
                                    filter.paths = Some(paths);
                                }
                            }
                            _ => {
                                warn!("Invalid filter field detected: {}", field);
                            }
                        }
                    }

                    filters.push(filter);
                }
                countries.insert(key.clone(), filters);
            }
            advertisers.insert(
                advertiser.to_lowercase(),
                AdmAdvertiserFilterSettings { countries, delete },
            );
        }

        trace!("Parsed Advertiser list: {:?}", &advertisers);

        Ok(advertisers)
    }

    #[cfg(test)]
    pub fn advertisers_to_string(filters: HashMap<String, AdmAdvertiserFilterSettings>) -> String {
        let mut result: serde_json::Map<String, Value> = serde_json::Map::new();
        for (advertiser, settings) in filters {
            let mut adv_value: serde_json::Map<String, Value> = serde_json::Map::new();
            for (country_name, country_paths) in settings.countries {
                adv_value.insert(country_name, serde_json::json!(country_paths));
            }
            if settings.delete {
                adv_value.insert("delete".to_owned(), Value::Bool(settings.delete));
            }
            result.insert(advertiser, Value::Object(adv_value));
        }
        Value::Object(result).to_string()
    }

    /// Try to fetch the ADM settings from a Google Storage bucket url.
    pub async fn advertisers_from_settings_bucket(
        cloud_storage: &cloud_storage::Client,
        settings_bucket: &url::Url,
    ) -> Result<HashMap<String, AdmAdvertiserFilterSettings>, ConfigError> {
        let settings_str = settings_bucket.as_str();
        if settings_bucket.scheme() != "gs" {
            return Err(ConfigError::Message(format!(
                "Improper bucket URL: {:?}",
                settings_str
            )));
        }
        let bucket_name = settings_bucket
            .host()
            .ok_or_else(|| {
                ConfigError::Message(format!("Invalid adm settings bucket name {}", settings_str))
            })?
            .to_string();
        let path = settings_bucket.path().trim_start_matches('/');
        let contents = cloud_storage
            .object()
            .download(&bucket_name, path)
            .await
            .map_err(|e| ConfigError::Message(format!("Could not download settings: {:?}", e)))?;
        AdmFilter::advertisers_from_string(
            &String::from_utf8(contents).map_err(|e| {
                ConfigError::Message(format!("Could not read ADM Settings: {:?}", e))
            })?,
        )
    }
}

/// Attempt to read the AdmSettings as either a path to a JSON file, or as a JSON string.
///
/// This allows `CONTILE_ADM_SETTINGS` to either be specified as inline JSON, or if the
/// Settings are too large to fit into an ENV string, specified in a path to where the
/// settings more comfortably fit.

/// Construct the AdmFilter from the provided settings.
///
/// This uses a JSON construct of settings, e.g.
/// ```javascript
/// /* for the Example Co advertiser... */
/// {"Example": {
///     /* region and paths for the advertiser */
///     "US":[
///         {
///             "host": "www.example.com",
///             "paths": [
///                 {
///                     "value": "/sample/",
///                     "matching": "prefix"
///                 },
///                 {
///                     "value", "/alternate_exact",
///                     "matching": "exact"
///                 }
///             ]
///         }
///         ]
///     },
///     ...,
/// }
/// ```
/// Each advertiser `"Example"` has a list of countries that it supports.
/// Each country has a list of domains and allowed paths.
/// Each path is an object listing the path value and the type of matching to perform,
/// either "exact" where only the exact path is allowed, or "prefix" where the path must
/// begin with the specified string.
/// There is a special case for an advertiser having a `"deleted": true` flag indicating
/// that this advertiser should be removed.
impl From<&mut Settings> for HandlerResult<AdmFilter> {
    fn from(settings: &mut Settings) -> Self {
        if settings.adm_sub1.is_none() ^ settings.adm_partner_id.is_none() {
            return Err(HandlerErrorKind::Internal(
                "Missing argument args for adm_sub1 or adm_partner_id".to_owned(),
            )
            .into());
        }
        if settings.adm_mobile_sub1.is_none() ^ settings.adm_mobile_partner_id.is_none() {
            return Err(HandlerErrorKind::Internal(
                "Missing argument args for adm_mobile_sub1 or adm_mobile_partner_id".to_owned(),
            )
            .into());
        }
        let refresh_rate = settings.adm_refresh_rate_secs;
        let ignore_list = settings
            .adm_ignore_advertisers
            .clone()
            .unwrap_or_else(|| "[]".to_owned())
            .to_lowercase();
        let legacy_list = settings
            .adm_has_legacy_image
            .clone()
            .unwrap_or_else(|| "[]".to_owned())
            .to_lowercase();

        let source = settings.adm_settings.clone();

        let source_url = if source.starts_with("gs://") {
            match source.parse::<url::Url>() {
                Ok(v) => Some(v),
                Err(e) => {
                    warn!(
                        "Source may be path or unparsable URL: {:?} {:?}",
                        &source, e
                    );
                    None
                }
            }
        } else {
            None
        };
        let defaults = if let Some(default_str) = &settings.adm_defaults {
            serde_json::from_str::<AdmDefaults>(default_str)
                .map_err(|e| HandlerError::internal(&e.to_string()))?
        } else {
            Default::default()
        };
        let excluded_countries_200 = settings.excluded_countries_200;

        let settings_str = if Path::new(&settings.adm_settings).exists() {
            read_to_string(&settings.adm_settings)
                .map_err(|e| {
                    HandlerError::internal(&format!(
                        "Could not read {}: {:?}",
                        settings.adm_settings, e
                    ))
                })
                .unwrap_or_else(|_| settings.adm_settings.clone())
        } else {
            debug!("{}/{} ... Not a valid path, presuming a string.",
                std::env::current_dir().expect("could not get current path").display(),
                &settings.adm_settings
            );
            settings.adm_settings.clone()
        };

        let advertiser_filters =
            if source_url.is_some() || (settings.adm_settings.is_empty() && settings.debug) {
                HashMap::new()
            } else {
                AdmFilter::advertisers_from_string(&settings_str)
                    .map_err(|e| HandlerError::internal(&format!("Configuration error reading AdmFilter advertisers: {:?}", e)))?
            };
        let ignore_list: HashSet<String> = serde_json::from_str(&ignore_list).map_err(|e| {
            HandlerError::internal(&format!("Invalid ADM Ignore list specification: {:?}", e))
        })?;
        let legacy_list: HashSet<String> = serde_json::from_str(&legacy_list).map_err(|e| {
            HandlerError::internal(&format!("Invalid ADM Legacy list specification: {:?}", e))
        })?;
        Ok(AdmFilter {
            advertiser_filters,
            ignore_list,
            legacy_list,
            last_updated: source.starts_with("gs://").then(chrono::Utc::now),
            source: Some(source),
            source_url,
            refresh_rate: std::time::Duration::from_secs(refresh_rate),
            defaults,
            excluded_countries_200,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    pub fn test_lower_ignore() {
        // ideally, this should verify that a given advertiser with an ignored name is
        // ignored, but no error is sent to sentry. Unfortunately, sentry 0.19 doesn't
        // support the introspection that later versions offer, so we have no way to
        // easily verify that no error is sent. For now, just make sure that the
        // data is lower cased.
        let mut result_list = HashSet::<String>::new();
        result_list.insert("example".to_owned());
        result_list.insert("invalid".to_owned());

        env::set_var(
            "CONTILE_ADM_IGNORE_ADVERTISERS",
            r#"["Example", "INVALID"]"#,
        );
        let mut settings = Settings::with_env_and_config_file(&None, true).unwrap();
        let result = HandlerResult::<AdmFilter>::from(&mut settings).unwrap();
        assert!(result.ignore_list == result_list);
    }

    #[test]
    pub fn test_valid_path_filters() {
        let adm_settings = r#"{"test-adv": {
            "US": [
                {
                    "host": "foo.com",
                    "paths": [
                        {
                            "value": "/bar/",
                            "matching": "prefix"
                        },
                        {
                            "value": "/gorp/",
                            "matching": "exact"
                        }
                    ]
                },
                {
                    "host": "foo.org"
                }
            ],
            "MX": [
                {
                    "host": "foo.mx"
                }
            ]
        }}"#;
        let result = AdmFilter::advertisers_from_string(adm_settings);
        debug!("result: {:?}", &result);
        assert!(result.is_ok());
    }

    #[test]
    pub fn test_invalid_prefix_path_filters() {
        let adm_settings = r#"{"test-adv": {
            "US": [
                {
                    "host": "foo.com",
                    "paths": [
                        {
                            "value": "/bar",
                            "matching": "prefix"
                        },
                        {
                            "value": "/gorp/",
                            "matching": "exact"
                        }
                    ]
                }
            ]
        }}"#;
        assert!(AdmFilter::advertisers_from_string(adm_settings).is_err());
    }

    #[test]
    pub fn test_invalid_path_filters() {
        let adm_settings = r#"{"test-adv": {
            "US": [
                {
                    "host": "foo.com",
                    "paths": [
                        {
                            "value": "/bar",
                            "matching": "prefix"
                        },
                        {
                            "value": "/gorp/",
                            "matching": "exact"
                        }
                    ]
                },
                {
                    "host": "foo.org",
                }
            ],
            "MX": [
                {
                    "host": "foo.mx",
                }
            ]
        }}"#;
        assert!(AdmFilter::advertisers_from_string(adm_settings).is_err());
    }
}
