use std::{collections::HashMap, fmt::Debug, fs::File, path::Path};

use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    pub(crate) impression_hosts: Vec<String>,
    /// Optional set of valid hosts for the `click_url`
    #[serde(default)]
    pub(crate) click_hosts: Vec<String>,
    /// Optional valid position for the tile
    pub(crate) position: Option<u8>,
    /// Optional set of valid regions for the tile (e.g ["en", "en-US/TX"])
    #[serde(default)]
    pub(crate) include_regions: Vec<String>,
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
            dbg!("Processing records for {:?}", &adv);
            // map the settings to the URL we're going to be checking
            filter_map.insert(adv.to_lowercase(), setting);
        }
        Ok(AdmFilter {
            filter_set: filter_map,
        })
    }
}
