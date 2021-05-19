use std::{collections::HashMap, fmt::Debug};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    error::{HandlerError, HandlerErrorKind, HandlerResult},
    server::{location::LocationResult, ServerState},
    settings::Settings,
    tags::Tags,
    web::{middleware::sentry as l_sentry, FormFactor, OsFamily},
};

pub(crate) const DEFAULT: &str = "DEFAULT";

#[derive(Debug, Deserialize, Serialize)]
pub struct AdmTileResponse {
    pub tiles: Vec<AdmTile>,
}

/// Filter criteria for adm Tiles
/// Each "filter"  is a set of [AdmAdvertiserFilterSettings] that are
/// specific to a given Advertiser name (the names are matched against
/// the tiles fetch request)
/// In addition there is a special [DEFAULT] value which is a filter
/// that will be applied to all advertisers that do not supply their
/// own values.
#[derive(Default, Clone, Debug)]
pub struct AdmFilter {
    pub filter_set: HashMap<String, AdmAdvertiserFilterSettings>,
}

/// The AdmAdvertiserFilterSettings contain the settings for the various
/// ADM provided partners. These are specified as a JSON formatted hash
/// that contains the components. A special "DEFAULT" setting provides
/// information that may be used as a DEFAULT, or commonly appearing set
/// of data.
/// See `impl From<Settings>` for details of the structure.
#[derive(Clone, Debug, Deserialize, Default, Serialize)]
pub struct AdmAdvertiserFilterSettings {
    /// Set of valid hosts for the `advertiser_url`
    pub(crate) advertiser_hosts: Vec<String>,
    /// Set of valid hosts for the `impression_url`
    pub(crate) impression_hosts: Vec<String>,
    /// Set of valid hosts for the `click_url`
    pub(crate) click_hosts: Vec<String>,
    /// valid position for the tile
    pub(crate) position: Option<u8>,
    /// Set of valid regions for the tile (e.g ["en", "en-US/TX"])
    pub(crate) include_regions: Vec<String>,
}

pub(crate) type AdmSettings = HashMap<String, AdmAdvertiserFilterSettings>;

impl From<&Settings> for AdmSettings {
    fn from(settings: &Settings) -> Self {
        if settings.adm_settings.is_empty() {
            return Self::default();
        }
        serde_json::from_str(&settings.adm_settings).expect("Invalid ADM Settings")
    }
}

/// Check that a given URL is valid according to it's corresponding filter
fn check_url(url: Url, species: &'static str, filter: &[String]) -> HandlerResult<bool> {
    let host = match url.host() {
        Some(v) => v.to_string(),
        None => {
            return Err(HandlerErrorKind::MissingHost(species, url.to_string()).into());
        }
    };
    if !filter.contains(&host) {
        return Err(HandlerErrorKind::UnexpectedHost(species, host).into());
    }
    Ok(true)
}

impl AdmFilter {
    /// Report the error directly to sentry
    fn report(&self, error: &HandlerError, tags: &Tags) {
        // dbg!(&error, &tags);
        // TODO: if not error.is_reportable, just add to metrics.
        l_sentry::report(tags, sentry::event_from_error(error));
    }

    /// Check the advertiser URL
    fn check_advertiser(
        &self,
        filter: &AdmAdvertiserFilterSettings,
        tile: &mut AdmTile,
        tags: &mut Tags,
    ) -> HandlerResult<()> {
        let url = &tile.advertiser_url;
        let species = "Advertiser";
        tags.add_tag("type", species);
        tags.add_extra("url", &url);
        let parsed: Url = match url.parse() {
            Ok(v) => v,
            Err(e) => {
                tags.add_extra("parse_error", &e.to_string());
                return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
            }
        };
        check_url(parsed, species, &filter.advertiser_hosts)?;
        Ok(())
    }

    /// Check the click URL
    ///
    /// Internally, this will use the hard-coded `req_keys` and `opt_keys` to specify
    /// the required and optional query parameter keys that can appear in the click_url
    fn check_click(
        &self,
        filter: &AdmAdvertiserFilterSettings,
        tile: &mut AdmTile,
        tags: &mut Tags,
    ) -> HandlerResult<()> {
        let url = &tile.click_url;
        let species = "Click";
        tags.add_tag("type", species);
        tags.add_extra("url", &url);

        // Check the required fields are present for the `click_url`
        // pg 15 of 5.7.21 spec. (sort for efficiency)
        // The list of sorted required query param keys for click_urls
        let req_keys = vec!["aespFlag", "ci", "ctag", "key", "version"];
        // the list of optionally appearing query param keys
        let opt_keys = vec!["click-status"];

        let mut all_keys = req_keys.clone();
        all_keys.extend(opt_keys.clone());

        let parsed: Url = match url.parse() {
            Ok(v) => v,
            Err(e) => {
                tags.add_extra("parse_error", &e.to_string());
                return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
            }
        };
        let mut query_keys = parsed
            .query_pairs()
            .map(|p| p.0.to_string())
            .collect::<Vec<String>>();
        query_keys.sort();

        // run the gauntlet of checks.
        if !check_url(parsed, "Click", &filter.click_hosts)? {
            dbg!("bad url", url.to_string());
            tags.add_extra("reason", "bad host");
            return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
        }
        for key in req_keys {
            if !query_keys.contains(&key.to_owned()) {
                dbg!("missing param", key, url.to_string());
                tags.add_extra("reason", "missing required query param");
                return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
            }
        }
        for key in query_keys {
            if !all_keys.contains(&key.as_str()) {
                dbg!("invalid param", key, url.to_string());
                tags.add_extra("reason", "invalid query param");
                return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
            }
        }
        Ok(())
    }

    /// Check the impression URL to see if it's valid.
    ///
    /// This extends `filter_and_process`
    fn check_impression(
        &self,
        filter: &AdmAdvertiserFilterSettings,
        tile: &mut AdmTile,
        tags: &mut Tags,
    ) -> HandlerResult<()> {
        let url = &tile.impression_url;
        let species = "Impression";
        tags.add_tag("type", species);
        tags.add_extra("url", &url);
        let parsed: Url = match url.parse() {
            Ok(v) => v,
            Err(e) => {
                tags.add_extra("parse_error", &e.to_string());
                return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
            }
        };
        let mut query_keys = parsed
            .query_pairs()
            .map(|p| p.0.to_string())
            .collect::<Vec<String>>();
        query_keys.sort();
        if query_keys != vec!["id"] {
            dbg!("missing param", "id", url.to_string());
            tags.add_extra("reason", "invalid query param");
            return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
        }
        check_url(parsed, species, &filter.impression_hosts)?;
        Ok(())
    }

    /// Filter and process tiles from ADM:
    ///
    /// - Returns None for tiles that shouldn't be shown to the client
    /// - Modifies tiles for output to the client (adding additional fields, etc.)
    pub fn filter_and_process(&self, mut tile: AdmTile, tags: &mut Tags) -> Option<AdmTile> {
        // Use strict matching for now, eventually, we may want to use backwards expanding domain
        // searches, (.e.g "xyz.example.com" would match "example.com")
        match self.filter_set.get(&tile.name.to_lowercase()) {
            Some(filter) => {
                // Apply any additional tile filtering here.
                let none = AdmAdvertiserFilterSettings::default();
                let default = self
                    .filter_set
                    .get(&DEFAULT.to_lowercase())
                    .unwrap_or(&none);
                // if the filter doesn't have anything defined, try using what's in the default.
                // Sadly, `vec.or()` doesn't exist, so do this a bit "long hand"
                let adv_filter = if filter.advertiser_hosts.is_empty() {
                    default
                } else {
                    filter
                };
                let impression_filter = if filter.impression_hosts.is_empty() {
                    default
                } else {
                    filter
                };
                let click_filter = if filter.click_hosts.is_empty() {
                    default
                } else {
                    filter
                };
                if let Err(e) = self.check_advertiser(adv_filter, &mut tile, tags) {
                    self.report(&e, tags);
                    return None;
                }
                if let Err(e) = self.check_click(click_filter, &mut tile, tags) {
                    self.report(&e, tags);
                    return None;
                }
                if let Err(e) = self.check_impression(impression_filter, &mut tile, tags) {
                    self.report(&e, tags);
                    return None;
                }
                // Use the default.position (Option<u8>) if the filter.position (Option<u8>) isn't
                // defined. In either case `None` is a valid return, but we should favor `filter` over
                // `default`.
                tile.position = filter.position.or(default.position);
                Some(tile)
            }
            None => {
                self.report(
                    &HandlerErrorKind::UnexpectedAdvertiser(tile.name).into(),
                    tags,
                );
                None
            }
        }
    }
}

/// Construct the AdmFilter from the provided settings.
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AdmTile {
    pub id: u64,
    pub name: String,
    pub advertiser_url: String,
    pub click_url: String,
    pub image_url: String,
    pub impression_url: String,
    pub position: Option<u8>,
}

#[allow(clippy::too_many_arguments)]
pub async fn get_tiles(
    reqwest_client: &reqwest::Client,
    adm_endpoint_url: &str,
    location: &LocationResult,
    stripped_ua: &str,
    os_family: OsFamily,
    form_factor: FormFactor,
    state: &ServerState,
    tags: &mut Tags,
) -> Result<AdmTileResponse, HandlerError> {
    // XXX: Assumes adm_endpoint_url includes
    // ?partner=<mozilla_partner_name>&sub1=<mozilla_tag_id> (probably should
    // validate this on startup)
    let settings = &state.settings;
    let adm_url = Url::parse_with_params(
        adm_endpoint_url,
        &[
            ("partner", settings.partner_id.as_str()),
            ("sub1", settings.sub1.as_str()),
            ("ip", &location.fake_ip), // TODO: remove once ADM API finalized
            ("ua", &stripped_ua),      // TODO: remove once ADM API finalized
            ("country-code", &location.country()),
            ("region-code", &location.region()),
            // ("dma-code", location.dma),
            ("form-factor", &form_factor.to_string()),
            ("os-family", &os_family.to_string()),
            ("sub2", "newtab"),
            ("v", "1.0"),
            // XXX: some value for results seems required, it defaults to 0
            // when omitted (despite AdM claiming it would default to 1)
            ("results", &settings.adm_query_tile_count.to_string()),
        ],
    )
    .map_err(|e| HandlerError::internal(&e.to_string()))?;
    let adm_url = adm_url.as_str();

    trace!("get_tiles GET {}", adm_url);
    let mut response: AdmTileResponse = reqwest_client
        .get(adm_url)
        .header(reqwest::header::USER_AGENT, stripped_ua)
        .send()
        .await
        .map_err(|e| {
            // ADM servers are down, or improperly configured
            HandlerErrorKind::BadAdmResponse(format!("ADM Server Error: {:?}", e))
        })?
        .error_for_status()?
        .json()
        .await
        .map_err(|e| {
            // ADM servers are not returning correct information
            HandlerErrorKind::BadAdmResponse(format!("ADM provided invalid response: {:?}", e))
        })?;
    response.tiles = response
        .tiles
        .into_iter()
        .filter_map(|tile| state.filter.filter_and_process(tile, tags))
        .take(settings.adm_max_tiles as usize)
        .collect();
    Ok(response)
}
