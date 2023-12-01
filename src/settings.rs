//! Application settings objects and initialization

use std::{path::PathBuf, time::Duration};

use actix_web::{dev::ServiceRequest, web::Data, HttpRequest};
use config::{Config, ConfigError, Environment, File};
use rand::{thread_rng, Rng};
use serde::Deserialize;

use crate::server::{img_storage::StorageSettings, ServerState};

static PREFIX: &str = "contile";

static DEFAULT_PORT: u16 = 8000;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TestModes {
    TestTimeout,
    TestFakeResponse,
    NoTest,
}

impl std::fmt::Display for TestModes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::TestTimeout => "Test Timeout",
                Self::TestFakeResponse => "Test Fake Response",
                Self::NoTest => "No Test",
            }
        )
    }
}

/// Configuration settings and options
///
/// Each of these can be specified as an environment variable by
/// making them upper case and prefixing `CONTILE_`. For example,
/// to specify the `port`, use the environment variable
/// `CONTILE_PORT=8000`
#[allow(rustdoc::private_intra_doc_links)]
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Enable verbose debugging output (default: false)
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
    /// Expire tiles after this many seconds (15 * 60s)
    pub tiles_ttl: u32,
    /// Fallback expiry for tiles after this many seconds (3 * 60 * 60s)
    pub tiles_fallback_ttl: u32,
    /// path to MaxMind location database
    pub maxminddb_loc: Option<PathBuf>,
    /// A JSON formatted string of [StorageSettings] related to
    /// the Google Cloud Storage
    pub storage: String,
    /// Run in "integration test mode"
    pub test_mode: TestModes,
    /// path to the test files
    pub test_file_path: String,
    /// Location test header override
    pub location_test_header: Option<String>,
    /// Fallback country (if no country is able to be determined for an
    /// IP). adM's API requires a minimum of country-code and form-factor
    /// parameters to return tiles
    pub fallback_country: String,
    /// URL to the official documentation
    pub documentation_url: String,
    /// Operational trace header
    pub trace_header: Option<String>,
    /// a JSON list of location DMAs to never return (population less than 15K)
    pub exclude_dma: Option<String>,
    /// Timeout (in seconds) for only the connect phase of all outbound HTTP requests
    pub connect_timeout: u64,
    /// default total request timeout (in seconds)
    pub request_timeout: u64,
    /// Whether Tiles responses may include a `Cache-Control` header
    pub cache_control_header: bool,

    // TODO: break these out into a PartnerSettings?
    /// Adm partner ID (default: "demofeed")
    pub adm_partner_id: Option<String>,
    /// Adm sub1 value (default: "123456789")
    pub adm_sub1: Option<String>,
    /// adm Endpoint URL
    pub adm_endpoint_url: String,
    /// max number of tiles returned to non-mobile clients (default: 3)
    pub adm_max_tiles: u8,
    /// Mobile versions of the above
    pub adm_mobile_partner_id: Option<String>,
    pub adm_mobile_sub1: Option<String>,
    pub adm_mobile_endpoint_url: Option<String>,
    /// max number of tiles returned to mobile clients (default: 2)
    pub adm_mobile_max_tiles: Option<u8>,
    /// number of tiles to query from ADM (default: 10)
    pub adm_query_tile_count: u8,
    /// Timeout requests to the ADM server after this many seconds (default: 5)
    pub adm_timeout: u64,
    /// ADM tile settings (either as JSON, a path to a JSON file, or a Google Storage url)
    /// This consists of an advertiser name, and the associated filter settings
    /// (e.g. ```{"Example":{"advertizer_hosts":["example.com"."example.org"]}})```)
    /// Unspecfied [crate::adm::AdmAdvertiserFilterSettings] will use Default values specified
    /// in `Default` (or the application default if not specified)
    pub adm_settings: String,
    /// Number of seconds to wait between polling ADM settings updates
    pub adm_refresh_rate_secs: u64,
    /// Check ADM settings on new tile requests.
    pub adm_live_update: bool,
    /// A JSON list of advertisers to ignore, specified by the Advertiser name.
    pub adm_ignore_advertisers: Option<String>,
    /// a JSON list of advertisers to allow for versions of firefox less than 91.
    pub adm_has_legacy_image: Option<String>,
    /// a JSON structure of the default ADM settings
    pub adm_defaults: Option<String>,
    /// Number of seconds to watch between polling SOV settings updates
    pub sov_refresh_rate_secs: u64,
    /// SOV settings (either as JSON, a path to a JSON file, or a Google Storage url)
    /// This consist of a name, and allocations settings
    /// (e.g ```{"name":"SOV","allocations":[{"position":1,"allocation":{"partner":"adm", "percentage": 100}]}]}]}```)
    pub sov_source: String,
    /// Percentage of overall time for fetch "jitter" (applied to `tiles_ttl` and tiles_fallback_ttl`)
    pub jitter: u8,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            // General settings
            debug: false,
            port: DEFAULT_PORT,
            host: "localhost".to_owned(),
            human_logs: false,
            statsd_label: PREFIX.to_owned(),
            statsd_host: None,
            statsd_port: 8125,
            actix_keep_alive: None,
            /// 15 minutes
            tiles_ttl: 15 * 60,
            /// 3 hours
            tiles_fallback_ttl: 3 * 60 * 60,
            maxminddb_loc: None,
            storage: "".to_owned(),
            test_mode: TestModes::NoTest,
            test_file_path: "./tools/test/test_data/".to_owned(),
            location_test_header: None,
            fallback_country: "US".to_owned(),
            documentation_url: "https://developer.mozilla.org/".to_owned(),
            trace_header: Some("X-Cloud-Trace-Context".to_owned()),
            // exclude for: Glendive, MT(798); Alpena, MI(583); North Platte, NE (740)
            exclude_dma: Some("[798, 583, 740]".to_owned()),
            connect_timeout: 2,
            request_timeout: 5,
            cache_control_header: true,
            // ADM specific settings
            adm_endpoint_url: "".to_owned(),
            adm_partner_id: None,
            adm_sub1: None,
            adm_max_tiles: 3,
            adm_mobile_endpoint_url: None,
            adm_mobile_partner_id: None,
            adm_mobile_sub1: None,
            adm_mobile_max_tiles: Some(2),
            adm_query_tile_count: 10,
            adm_timeout: 5,
            adm_settings: "".to_owned(),
            adm_refresh_rate_secs: 300,
            adm_live_update: false,
            adm_ignore_advertisers: None,
            adm_has_legacy_image: Some(
                r#"["adidas","amazon","ebay","etsy","geico","nike","samsung","wix"]"#.to_owned(),
            ),
            adm_defaults: None,
            sov_refresh_rate_secs: 300,
            sov_source: "".to_owned(),
            // +/- 10% of time for jitter.
            jitter: 10,
        }
    }
}

impl Settings {
    pub fn verify_settings(&mut self) -> Result<(), ConfigError> {
        if self.adm_endpoint_url.is_empty() {
            return Err(ConfigError::Message("Missing adm_endpoint_url".to_owned()));
        }

        if self.fallback_country.len() != 2 {
            return Err(ConfigError::Message(
                "Invalid fallback_country specified. Please use a string like \"US\"".to_owned(),
            ));
        }
        self.fallback_country = self.fallback_country.to_uppercase();

        // preflight check the storage
        let _ = StorageSettings::from(&*self);
        Ok(())
    }

    /// Load the settings from the config file if supplied, then the environment.
    pub fn with_env_and_config_file(
        filename: &Option<String>,
        debug: bool,
    ) -> Result<Self, ConfigError> {
        let mut builder = Config::builder();
        // Merge the config file if supplied
        if let Some(config_filename) = filename {
            debug!("Reading settings from {}", &config_filename);
            builder = builder.add_source(File::with_name(config_filename));
        }
        // Merge the environment overrides
        let s = builder
            .add_source(Environment::with_prefix(&PREFIX.to_uppercase()))
            .build()?;

        debug!("deserializing {:?}", &s);
        Ok(match s.try_deserialize::<Self>() {
            Ok(mut s) => {
                trace!("raw Settings: {:?}", &s);
                if debug || s.test_mode != TestModes::NoTest {
                    trace!("!! Running in test mode!");
                    s.adm_endpoint_url = "http://localhost:8675/".to_owned();
                    s.debug = true;
                    s.adm_partner_id = Some("test".to_owned());
                    s.adm_sub1 = Some("test".to_owned());
                }
                // Adjust the max values if required.
                s.verify_settings()?;
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

    pub fn tiles_ttl_with_jitter(&self) -> Duration {
        Duration::from_secs(self.add_jitter(self.tiles_ttl) as u64)
    }

    pub fn tiles_fallback_ttl_with_jitter(&self) -> Duration {
        Duration::from_secs(self.add_jitter(self.tiles_fallback_ttl) as u64)
    }

    /// Calculate the ttl from the settings by taking the tiles_ttl and
    /// calculating a jitter that is no more than 50% of the total TTL. It is
    /// recommended that "jitter" be 10%.
    fn add_jitter(&self, value: u32) -> u32 {
        let mut rng = thread_rng();
        let ftl = value as f32;
        let offset = ftl * (std::cmp::min(self.jitter, 50) as f32 * 0.01);
        if offset == 0.0 {
            // Don't panic gen_range with an empty range (a tiles_ttl or jitter
            // of 0 was specified)
            return 0;
        }
        let jit = rng.gen_range(0.0 - offset..offset);
        (ftl + jit) as u32
    }
}

impl<'a> From<&'a HttpRequest> for &'a Settings {
    fn from(req: &'a HttpRequest) -> Self {
        let state = req.app_data::<Data<ServerState>>().expect("No State!");
        &state.settings
    }
}

impl<'a> From<&'a ServiceRequest> for &'a Settings {
    fn from(req: &'a ServiceRequest) -> Self {
        let state = req.app_data::<Data<ServerState>>().expect("No State!");
        &state.settings
    }
}

#[cfg(test)]
pub fn test_settings() -> Settings {
    Settings::with_env_and_config_file(&None, true)
        .expect("Could not get Settings in get_test_settings")
}

#[cfg(test)]
mod tests {
    use crate::{error::HandlerResult, settings::Settings};

    #[actix_web::test]
    async fn test_fallback_loc() -> HandlerResult<()> {
        // From a bad setting
        let mut settings = Settings {
            fallback_country: "USA".to_owned(),
            adm_endpoint_url: "http://localhost:8080".to_owned(),
            adm_sub1: Some("test".to_owned()),
            adm_partner_id: Some("test".to_owned()),
            ..Default::default()
        };
        assert!(settings.verify_settings().is_err());
        settings.fallback_country = "US,OK".to_owned();
        assert!(settings.verify_settings().is_err());
        settings.fallback_country = "US".to_owned();
        assert!(settings.verify_settings().is_ok());
        assert!(settings.fallback_country == "US");

        // // From an empty Google LB header
        // let metrics = Metrics::noop();
        // let mut test_head = RequestHead::default();
        // let hv = ", ";
        // test_head.headers_mut().append(
        //     HeaderName::from_static(GOOG_LOC_HEADER),
        //     HeaderValue::from_static(&hv),
        // );
        // let loc = LocationResult::from_header(&test_head, &settings, &metrics);
        // assert!(loc.region() == "");
        // assert!(loc.country() == "US");
        Ok(())
    }
}
