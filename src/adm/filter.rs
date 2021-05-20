use std::{collections::HashMap, fmt::Debug};

use url::Url;

use super::{AdmAdvertiserFilterSettings, AdmTile, DEFAULT};
use crate::{
    error::{HandlerError, HandlerErrorKind, HandlerResult},
    tags::Tags,
    web::middleware::sentry as l_sentry,
};

#[allow(rustdoc::private_intra_doc_links)]
/// Filter criteria for ADM Tiles
///
/// Each "filter"  is a set of [crate::adm::AdmAdvertiserFilterSettings] that are
/// specific to a given Advertiser name (the names are matched against
/// the tiles fetch request)
/// In addition there is a special `DEFAULT` value which is a filter
/// that will be applied to all advertisers that do not supply their
/// own values.
#[derive(Default, Clone, Debug)]
pub struct AdmFilter {
    pub filter_set: HashMap<String, AdmAdvertiserFilterSettings>,
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

/// Filter a given tile data set provided by ADM and validate the various elements
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
