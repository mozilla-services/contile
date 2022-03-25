use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
    fmt::Debug,
    fs::File,
    io::Read,
    path::Path,
};

use config::ConfigError;
use serde::{ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer};

use super::AdmFilter;
use crate::{
    error::{HandlerError, HandlerResult},
    settings::Settings,
    web::DeviceInfo,
};

/// The name of the "Default" node, which is used as a fall back if no data
/// is defined for a given partner.
pub(crate) const DEFAULT: &str = "DEFAULT";

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
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AdvertiserUrlFilter {
    pub(crate) host: String,
    pub(crate) paths: Option<Vec<PathFilter>>,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PathMatching {
    Prefix,
    Exact,
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

/// The AdmAdvertiserFilterSettings contain the settings for the various
/// ADM provided partners.
///
/// These are specified as a JSON formatted hash
/// that contains the components. A special "DEFAULT" setting provides
/// information that may be used as a DEFAULT, or commonly appearing set
/// of data. Any Optional value that is not defined will use the value
/// defined in DEFAULT.
#[derive(Clone, Debug, Deserialize, Default, Serialize)]
pub struct AdmAdvertiserFilterSettings {
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
    /// TODO: could support country + subdivision, e.g. "USOK"
    #[serde(default)]
    pub(crate) include_regions: Vec<String>,
    pub(crate) ignore_advertisers: Option<Vec<String>>,
    pub(crate) ignore_dmas: Option<Vec<u8>>,
    #[serde(default)]
    pub(crate) delete: bool,
}

/// Parse JSON:
/// ["example.com", "foo.net"]
/// into:
/// [["example", "com"], ["foo", "net"]]
fn deserialize_hosts<'de, D>(d: D) -> Result<Vec<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Deserialize::deserialize(d).map(|hosts: Vec<String>| {
        hosts
            .into_iter()
            .map(|host| -> Vec<_> { host.split('.').map(ToOwned::to_owned).collect() })
            .collect()
    })
}

/// Serialize:
/// [["example", "com"], ["foo", "net"]]
/// into:
/// ["example.com", "foo.net"]
fn serialize_hosts<S>(hosts: &[Vec<String>], s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let hosts: Vec<_> = hosts
        .iter()
        .map(|split_host| split_host.join("."))
        .collect();
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

/// Create AdmSettings from a string serialized JSON format
impl TryFrom<String> for AdmFilterSettings {
    type Error = ConfigError;

    fn try_from(settings_str: String) -> Result<Self, Self::Error> {
        // don't try to serialize bucket values quite yet.
        if settings_str.starts_with("gs://") {
            return Ok(Self {
                bucket: Some(settings_str.parse::<url::Url>().map_err(|err| {
                    ConfigError::Message(format!(
                        "Invalid bucket url: {:?} {:?}",
                        settings_str, err
                    ))
                })?),
                ..Default::default()
            });
        }
        let adm_settings: HashMap<String, AdmAdvertiserFilterSettings> =
            serde_json::from_str(&settings_str).expect("Invalid ADM Settings JSON string");
        for (adv, filter_setting) in &adm_settings {
            if filter_setting
                .include_regions
                .iter()
                .any(|region| region != &region.to_uppercase())
            {
                return Err(ConfigError::Message(format!(
                    "Advertiser {:?} include_regions must be uppercase",
                    adv
                )));
            }
            if filter_setting.advertiser_urls.iter().any(|filter| {
                if let Some(ref paths) = filter.paths {
                    return paths.iter().any(|path| match path.matching {
                        PathMatching::Prefix => !path.value.ends_with('/'),
                        PathMatching::Exact => !path.value.starts_with('/'),
                    });
                }
                false
            }) {
                return Err(ConfigError::Message(format!("Advertiser {:?} advertiser_urls contain invalid prefix PathFilter (missing trailing '/')", adv)));
            }
        }
        Ok(AdmFilterSettings {
            advertisers: adm_settings,
            ..Default::default()
        })
    }
}

impl AdmFilterSettings {
    /// Try to fetch the ADM settings from a Google Storage bucket url.
    pub async fn from_settings_bucket(
        settings_bucket: &url::Url,
        connection_timeout: std::time::Duration,
        request_timeout: std::time::Duration,
    ) -> Result<AdmFilterSettings, ConfigError> {
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
        let req = reqwest::Client::builder()
            .connect_timeout(connection_timeout)
            .timeout(request_timeout)
            .build()
            .map_err(|e| ConfigError::Message(e.to_string()))?;
        let contents = cloud_storage::Object::download_with(&bucket_name, path, &req)
            .await
            .map_err(|e| ConfigError::Message(format!("Could not download settings: {:?}", e)))?;
        let mut reply =
            AdmFilterSettings::try_from(String::from_utf8(contents).map_err(|e| {
                ConfigError::Message(format!("Could not read ADM Settings: {:?}", e))
            })?)?;
        reply.bucket = Some(settings_bucket.clone());
        Ok(reply)
    }
}

/// Attempt to read the AdmSettings as either a path to a JSON file, or as a JSON string.
///
/// This allows `CONTILE_ADM_SETTINGS` to either be specified as inline JSON, or if the
/// Settings are too large to fit into an ENV string, specified in a path to where the
/// settings more comfortably fit.
impl TryFrom<&mut Settings> for AdmFilterSettings {
    type Error = ConfigError;

    fn try_from(settings: &mut Settings) -> Result<Self, Self::Error> {
        // TODO: Convert these to macros.
        if settings.adm_sub1.is_none() {
            return Err(ConfigError::Message(format!(
                "Missing argument {}",
                "adm_sub1"
            )));
        }
        if settings.adm_partner_id.is_none() {
            return Err(ConfigError::Message(format!(
                "Missing argument {}",
                "adm_partner_id"
            )));
        }
        if settings.adm_settings.is_empty() {
            return Ok(Self::default());
        }
        let mut settings_str = settings.adm_settings.clone();
        if Path::new(&settings_str).exists() {
            if let Ok(mut f) = File::open(&settings.adm_settings) {
                settings_str = String::new();
                f.read_to_string(&mut settings_str).map_err(|e| {
                    ConfigError::Message(format!(
                        "Could not read {}: {:?}",
                        settings.adm_settings, e
                    ))
                })?;
            }
        }
        AdmFilterSettings::try_from(settings_str)
    }
}

/// Construct the AdmFilter from the provided settings.
///
/// This uses a JSON construct of settings, e.g.
/// ```javascript
/// /* for the Example Co advertiser... */
/// {"Example": {
///     /* The allowed hosts for URLs */
///     "advertiser_urls": [{"host": "www.example.org"}, {"host": "example.org"}],
///     /* Valid tile positions for this advertiser (empty for "all") */
///     "positions": 1,
///     /* Valid target countries for this advertiser
///        TODO: could support country + subdivision, e.g. "USOK" */
///     "include_regions": ["US", "MX"],
///     /* Allowed hosts for impression URLs.
///        Empty means to use the impression URLs in "DEFAULT" */
///     "impression_hosts: [],
///     },
///     ...,
///  "DEFAULT": {
///    /* The default impression URL host to check for. */
///    "impression_hosts": ["example.net"]
///     }
/// }
/// ```
///
impl From<&mut Settings> for HandlerResult<AdmFilter> {
    fn from(settings: &mut Settings) -> Self {
        let mut filter_map: HashMap<String, AdmAdvertiserFilterSettings> = HashMap::new();
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
        let mut all_include_regions = HashSet::new();
        let source = settings.adm_settings.clone();
        let connect_timeout = settings.connect_timeout;
        let request_timeout = settings.request_timeout;
        let source_url = match source.parse::<url::Url>() {
            Ok(v) => Some(v),
            Err(e) => {
                warn!(
                    "Source may be path or unparsable URL: {:?} {:?}",
                    &source, e
                );
                None
            }
        };
        for (adv, setting) in AdmFilterSettings::try_from(settings)
            .map_err(|e| HandlerError::internal(&e.to_string()))?
            .advertisers
        {
            trace!("Processing records for {:?}", &adv);
            // DEFAULT included but sans special processing -- close enough
            for country in &setting.include_regions {
                all_include_regions.insert(country.clone());
            }
            // map the settings to the URL we're going to be checking
            filter_map.insert(adv.to_lowercase(), setting);
        }
        let ignore_list: HashSet<String> = serde_json::from_str(&ignore_list).map_err(|e| {
            HandlerError::internal(&format!("Invalid ADM Ignore list specification: {:?}", e))
        })?;
        let legacy_list: HashSet<String> = serde_json::from_str(&legacy_list).map_err(|e| {
            HandlerError::internal(&format!("Invalid ADM Legacy list specification: {:?}", e))
        })?;
        Ok(AdmFilter {
            filter_set: filter_map,
            ignore_list,
            all_include_regions,
            legacy_list,
            last_updated: source.starts_with("gs://").then(chrono::Utc::now),
            source,
            source_url,
            refresh_rate: std::time::Duration::from_secs(refresh_rate),
            connect_timeout: std::time::Duration::from_secs(connect_timeout),
            request_timeout: std::time::Duration::from_secs(request_timeout),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use serde_json::json;

    use super::*;
    use crate::web::test::adm_settings;

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
    pub fn all_include_regions() {
        let mut settings = Settings::with_env_and_config_file(&None, true).unwrap();
        let mut adm_settings = adm_settings();
        adm_settings
            .advertisers
            .get_mut("Dunder Mifflin")
            .expect("No Dunder Mifflin tile")
            .include_regions = vec!["MX".to_owned()];
        settings.adm_settings = json!(adm_settings.advertisers).to_string();
        let filter = HandlerResult::<AdmFilter>::from(&mut settings).unwrap();
        assert!(
            filter.all_include_regions
                == vec!["US", "MX"]
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect()
        );
    }

    #[test]
    pub fn test_invalid_path_filters() {
        let mut settings = Settings::default();
        let adm_settings = r#"{"test-adv": {
            "advertiser_urls": [
                {
                    "host": "foo.com",
                    "paths": [
                        {
                            "value": "/bar",
                            "matching": "prefix"
                        }
                    ]
                }
            ]
        }}"#;
        settings.adm_settings = adm_settings.to_owned();
        assert!(AdmFilterSettings::try_from(&mut settings).is_err());
    }
}
