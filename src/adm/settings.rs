use std::{collections::HashMap, fmt::Debug, fs::File, path::Path};

use serde::{ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer};

use super::AdmFilter;
use crate::{error::HandlerResult, settings::Settings};

/// The name of the "Default" node, which is used as a fall back if no data
/// is defined for a given partner.
pub(crate) const DEFAULT: &str = "DEFAULT";

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
    /// Required set of valid hosts for the `advertiser_url`
    pub(crate) advertiser_hosts: Vec<String>,
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
    /// valid position for the tile
    pub(crate) position: Option<u8>,
    /// Optional set of valid regions for the tile (e.g ["en", "en-US/TX"])
    #[serde(default)]
    pub(crate) include_regions: Vec<String>,
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

pub(crate) type AdmSettings = HashMap<String, AdmAdvertiserFilterSettings>;

/// Attempt to read the AdmSettings as either a path to a JSON file, or as a JSON string.
///
/// This allows `CONTILE_ADM_SETTINGS` to either be specified as inline JSON, or if the
/// Settings are too large to fit into an ENV string, specified in a path to where the
/// settings more comfortably fit.
impl From<&Settings> for AdmSettings {
    fn from(settings: &Settings) -> Self {
        if settings.adm_settings.is_empty() {
            return Self::default();
        }
        if Path::new(&settings.adm_settings).exists() {
            if let Ok(f) = File::open(&settings.adm_settings) {
                return serde_json::from_reader(f).expect("Invalid ADM Settings file");
            }
        }
        serde_json::from_str(&settings.adm_settings).expect("Invalid ADM Settings JSON string")
    }
}

/// Construct the AdmFilter from the provided settings.
///
/// This uses a JSON construct of settings, e.g.
/// ```javascript
/// /* for the Example Co advertiser... */
/// {"Example": {
///     /* The allowed hosts for URLs */
///     "advertiser_hosts": ["www.example.org", "example.org"],
///     /* Valid tile positions for this advertiser (empty for "all") */
///     "positions": 1,
///     /* Valid target regions for this advertiser
///        (use "en-US" for "all in english speaking United States") */
///     "include_regions": ["en-US/TX", "en-US/CA"],
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
impl From<&Settings> for HandlerResult<AdmFilter> {
    fn from(settings: &Settings) -> Self {
        let mut filter_map: HashMap<String, AdmAdvertiserFilterSettings> = HashMap::new();
        for (adv, setting) in AdmSettings::from(settings) {
            trace!("Processing records for {:?}", &adv);
            // map the settings to the URL we're going to be checking
            filter_map.insert(adv.to_lowercase(), setting);
        }
        Ok(AdmFilter {
            filter_set: filter_map,
        })
    }
}
