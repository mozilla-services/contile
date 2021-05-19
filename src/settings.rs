//! Application settings objects and initialization

use std::collections::HashMap;

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

use crate::web::adm::AdmSettings;
use crate::server::img_storage::StorageSettings;

static PREFIX: &str = "contile";

static DEFAULT_PORT: u16 = 8000;

// issue11: AdM doesn't return any tiles for any of the example non-finalized
// fake IPs, so replaced "US": "174.245.240.112" with "130.245.32.23" (from
// their provided stage URL params) that does result in tiles
static DEFAULT_ADM_COUNTRY_IP_MAP: &str = r#"
{
    "US": "130.245.32.23",
    "UK": "86.164.248.137",
    "DE": "87.182.235.159",
    "FR": "31.39.255.255",
    "IT": "5.62.79.255",
    "JP": "27.98.191.255"
}
"#;

// TODO: Call this `EnvSettings` that serializes into
// real `Settings`?
//
/// Configuration settings and options
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Enable verbos debugging output (default: false)
    pub debug: bool,
    /// Service port (default: 8000)
    pub port: u16,
    /// Service hostname (default: 127.0.0.1)
    pub host: String,
    /// Enable "human readable" logging messages (default: false)
    pub human_logs: bool,
    /// Metric default label (default: "contile")
    pub statsd_label: String,
    /// Metric reporting host address (default: None)
    pub statsd_host: Option<String>,
    /// Metric reporting host port
    pub statsd_port: u16,
    /// Enable actix "keep alive" period in seconds (default: None)
    pub actix_keep_alive: Option<u64>,
    /// adm Endpoint URL
    pub adm_endpoint_url: String,
    /// adm country to default IP map (Hash in JSON format)
    pub adm_country_ip_map: String,
    /// max tiles to accept from ADM (default: 2)
    pub adm_max_tiles: u8,
    /// number of tiles to query from ADM (default: 10)
    pub adm_query_tile_count: u8,
    /// Expire tiles after this many seconds (15 * 60s)
    pub tiles_ttl: u32,
    /// ADM tile settings (either as JSON or a path to a JSON file)
    /// This consists of an advertiser name, and the associated filter settings
    /// (e.g. ```{"Example":{"advertizer_hosts":["example.com"."example.org"]}})```)
    /// Unspecfied [AdmAdvertiserFilterSetttings] will use Default values specified
    /// in `Default` (or the application default if not specified)
    pub adm_settings: String,
    /// path to MaxMind location database
    pub maxminddb_loc: Option<String>,
    /// [StorageSettings] related to the google cloud storage
    pub storage: String,
    /// Adm partner ID (default: "demofeed")
    pub partner_id: String,
    /// Adm sub1 value (default: "123456789")
    pub sub1: String,
    /// Run in "integration test mode"
    pub test_mode: bool,
    /// path to the test files
    pub test_file_path: String,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            debug: false,
            port: DEFAULT_PORT,
            host: "localhost".to_owned(),
            human_logs: false,
            statsd_label: PREFIX.to_owned(),
            statsd_host: None,
            statsd_port: 8125,
            actix_keep_alive: None,
            adm_endpoint_url: "".to_owned(),
            adm_country_ip_map: DEFAULT_ADM_COUNTRY_IP_MAP.to_owned(),
            adm_max_tiles: 2,
            adm_query_tile_count: 10,
            tiles_ttl: 15 * 60,
            adm_settings: "".to_owned(),
            maxminddb_loc: None,
            storage: "".to_owned(),
            partner_id: "demofeed".to_owned(),
            sub1: "123456789".to_owned(),
            test_mode: false,
            test_file_path: "./tools/test/test_data/".to_owned(),
        }
    }
}

impl Settings {
    /// Load the settings from the config file if supplied, then the environment.
    pub fn with_env_and_config_file(filename: &Option<String>, debug:Option<bool>) -> Result<Self, ConfigError> {
        let mut s = Config::default();

        // Merge the config file if supplied
        if let Some(config_filename) = filename {
            s.merge(File::with_name(config_filename))?;
        }

        // Merge the environment overrides
        // While the prefix is currently case insensitive, it's traditional that
        // environment vars be UPPERCASE, this ensures that will continue should
        // Environment ever change their policy about case insensitivity.
        // This will accept environment variables specified as
        // `SYNC_FOO__BAR_VALUE="gorp"` as `foo.bar_value = "gorp"`
        s.merge(Environment::with_prefix(&PREFIX.to_uppercase()).separator("__"))?;

        Ok(match s.try_into::<Self>() {
            Ok(mut s) => {
                if debug.unwrap_or(false) || s.test_mode {
                    dbg!("!! Running in test mode!");
                    s.adm_endpoint_url = "http://localhost:8675/".to_owned();
                    s.debug = true;
                }
                if s.adm_endpoint_url.is_empty() {
                    return Err(ConfigError::Message("Missing adm_endpoint_url".to_owned()));
                }
                // preflight check the storage
                StorageSettings::from(&s);
                AdmSettings::from(&s);
                // Adjust the max values if required.
                s
            }
            Err(e) => match e {
                // Configuration errors are not very sysop friendly, Try to make them
                // a bit more 3AM useful.
                ConfigError::Message(v) => {
                    println!("Bad configuration: {:?}", &v);
                    println!("Please set in config file or use environment variable.");
                    println!(
                        "For example to set `database_url` use env var `{}_DATABASE_URL`\n",
                        PREFIX.to_uppercase()
                    );
                    error!("Configuration error: Value undefined {:?}", &v);
                    return Err(ConfigError::NotFound(v));
                }
                _ => {
                    error!("Configuration error: Other: {:?}", &e);
                    return Err(e);
                }
            },
        })
    }

    /// A simple banner for display of certain settings at startup
    pub fn banner(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    /// convert the `adm_country_ip_map` setting from a string to a hashmap
    pub(crate) fn build_adm_country_ip_map(&self) -> HashMap<String, String> {
        let mut map: HashMap<String, String> =
            serde_json::from_str(&self.adm_country_ip_map).expect("Invalid ADM_COUNTRY_IP_MAP");
        map = map
            .into_iter()
            .map(|(key, val)| (key.to_uppercase(), val))
            .collect();
        if !map.contains_key("US") {
            panic!("Invalid ADM_COUNTRY_IP_MAP");
        }
        map
    }
}

#[cfg(test)]
pub fn test_settings() -> Settings {
    Settings {
        debug: true,
        ..Settings::with_env_and_config_file(&None, Some(true))
            .expect("Could not get Settings in get_test_settings")
    }
}
