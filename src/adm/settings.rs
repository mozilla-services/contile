use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    fs::File,
    path::Path,
};

use serde::{ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer};

use super::AdmFilter;
use crate::{
    error::{HandlerError, HandlerResult},
    settings::Settings,
};

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
    /// Optional set of valid countries for the tile (e.g ["US", "GB"])
    /// TODO: could support country + subdivision, e.g. "USOK"
    #[serde(default)]
    pub(crate) include_regions: Vec<String>,
    pub(crate) ignore_advertisers: Option<Vec<String>>,
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
impl From<&mut Settings> for AdmSettings {
    fn from(settings: &mut Settings) -> Self {
        // TODO: Convert these to macros.
        if settings.adm_sub1.is_none() {
            if settings.sub1.is_some() {
                settings.adm_sub1 = settings.sub1.clone();
                eprintln!(
                    "{:?} is obsolete and will be removed. Please use {:?}",
                    "sub1", "adm_sub1"
                );
            } else {
                panic!("Missing argument {}", "adm_sub1");
            }
        }
        if settings.adm_partner_id.is_none() {
            if settings.partner_id.is_some() {
                settings.adm_partner_id = settings.partner_id.clone();
                eprintln!(
                    "{:?} is obsolete and will be removed. Please use {:?}",
                    "partner_id", "adm_partner_id"
                );
            } else {
                panic!("Missing argument {}", "$new");
            }
        }
        if settings.adm_settings.is_empty() {
            return Self::default();
        }
        if Path::new(&settings.adm_settings).exists() {
            if let Ok(f) = File::open(&settings.adm_settings) {
                return serde_json::from_reader(f).expect("Invalid ADM Settings file");
            }
        }
        dbg!(&settings);
        let adm_settings: AdmSettings =
            serde_json::from_str(&settings.adm_settings).expect("Invalid ADM Settings JSON string");
        for (adv, filter_setting) in &adm_settings {
            if filter_setting
                .include_regions
                .iter()
                .any(|region| region != &region.to_uppercase())
            {
                panic!("Advertiser {:?} include_regions must be uppercase", adv);
            }
        }
        adm_settings
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
        for (adv, setting) in AdmSettings::from(settings) {
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
    pub fn test_obsolete_settings() {
        let mut settings = Settings::default();
        let sub1 = "12345".to_owned();
        let partner_id = "falafal".to_owned();

        // New overrides old
        settings.adm_sub1 = Some(sub1.clone());
        settings.adm_partner_id = Some(partner_id.clone());
        settings.sub1 = Some("000000".to_owned());
        settings.partner_id = Some("banana".to_owned());
        AdmSettings::from(&mut settings);
        assert!(settings.adm_sub1 == Some(sub1.clone()));
        assert!(settings.adm_partner_id == Some(partner_id.clone()));

        // Old defaults accepted as unspecified new
        env::set_var("CONTILE_SUB1", &sub1);
        env::set_var("CONTILE_PARTNER_ID", &partner_id);
        let mut settings = Settings::with_env_and_config_file(&None, true).unwrap();
        AdmSettings::from(&mut settings);
        assert!(settings.adm_sub1 == Some(sub1));
        assert!(settings.adm_partner_id == Some(partner_id));
    }

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
            .get_mut("Dunder Mifflin")
            .expect("No Dunder Mifflin tile")
            .include_regions = vec!["MX".to_owned()];
        settings.adm_settings = json!(adm_settings).to_string();
        let filter = HandlerResult::<AdmFilter>::from(&mut settings).unwrap();
        assert!(
            filter.all_include_regions
                == vec!["US", "MX"]
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect()
        );
    }
}
