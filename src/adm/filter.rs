use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    iter::FromIterator,
};

use actix_http::http::Uri;
use actix_web_location::Location;
use lazy_static::lazy_static;
use url::Url;

use super::{
    tiles::{AdmTile, Tile},
    AdmAdvertiserFilterSettings, DEFAULT,
};
use crate::{
    error::{HandlerError, HandlerErrorKind, HandlerResult},
    metrics::Metrics,
    tags::Tags,
    web::middleware::sentry as l_sentry,
    web::DeviceInfo,
};

lazy_static! {
    static ref REQ_CLICK_PARAMS: Vec<&'static str> = vec!["ci", "ctag", "key", "version"];
    static ref ALL_CLICK_PARAMS: HashSet<&'static str> = {
        let opt_click_params = vec!["click-status"];
        let mut all = HashSet::from_iter(REQ_CLICK_PARAMS.clone());
        all.extend(opt_click_params);
        all
    };
}

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
    /// Filter settings by Advertiser name
    pub filter_set: HashMap<String, AdmAdvertiserFilterSettings>,
    /// Ignored (not included but also not reported to Sentry) Advertiser names
    pub ignore_list: HashSet<String>,
    /// All countries set for inclusion in at least one of the
    /// [crate::adm::AdmAdvertiserFilterSettings]
    pub all_include_regions: HashSet<String>,
    /// Temporary list of advertisers with legacy images built into firefox
    /// for pre 91 tile support.
    pub legacy_list: HashSet<String>,
}

/// Extract the host from Url
fn get_host(url: Url, species: &'static str) -> HandlerResult<String> {
    url.host()
        .map(|host| host.to_string())
        .ok_or_else(|| HandlerErrorKind::MissingHost(species, url.to_string()).into())
}

/// Check that a given URL is valid according to it's corresponding filter.
///
/// Allows a partial match: a filter setting for "example.com" (["example",
/// "com"]) allows "foo.example.com" and "quux.bar.example.com" (["quux",
/// "bar", "example", "com"])
fn check_url(url: Url, species: &'static str, filter: &[Vec<String>]) -> HandlerResult<bool> {
    let host = get_host(url, species)?;
    let domains: Vec<_> = host.split('.').collect();
    for allowed in filter {
        let begin = domains.len() - allowed.len().min(domains.len());
        if &domains[begin..] == allowed {
            return Ok(true);
        }
    }
    Err(HandlerErrorKind::UnexpectedHost(species, host).into())
}

/// Filter a given tile data set provided by ADM and validate the various elements
impl AdmFilter {
    /// Report the error directly to sentry
    fn report(&self, error: &HandlerError, tags: &Tags) {
        // trace!(&error, &tags);
        // TODO: if not error.is_reportable, just add to metrics.
        let mut merged_tags = error.tags.clone();
        merged_tags.extend(tags.clone());
        l_sentry::report(&merged_tags, sentry::event_from_error(error));
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
        let parsed: Url = match url.parse() {
            Ok(v) => v,
            Err(e) => {
                tags.add_tag("type", species);
                tags.add_extra("tile", &tile.name);
                tags.add_extra("url", url);
                tags.add_extra("parse_error", &e.to_string());
                return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
            }
        };

        let host = get_host(parsed, species)?;
        if !filter.advertiser_hosts.contains(&host) {
            return Err(HandlerErrorKind::UnexpectedHost(species, host).into());
        }
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
        // Check the required fields are present for the `click_url` pg 15 of
        // 5.7.21 spec
        let parsed: Url = match url.parse() {
            Ok(v) => v,
            Err(e) => {
                tags.add_tag("type", species);
                tags.add_extra("tile", &tile.name);
                tags.add_extra("url", url);

                tags.add_extra("parse_error", &e.to_string());
                return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
            }
        };
        let query_keys = parsed
            .query_pairs()
            .map(|p| p.0.to_string())
            .collect::<HashSet<String>>();

        // run the gauntlet of checks.
        if !check_url(parsed, "Click", &filter.click_hosts)? {
            trace!("bad url: url={:?}", url.to_string());
            tags.add_tag("type", species);
            tags.add_extra("tile", &tile.name);
            tags.add_extra("url", url);

            tags.add_extra("reason", "bad host");
            return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
        }
        for key in &*REQ_CLICK_PARAMS {
            if !query_keys.contains(*key) {
                trace!("missing param: key={:?} url={:?}", &key, url.to_string());
                tags.add_tag("type", species);
                tags.add_extra("tile", &tile.name);
                tags.add_extra("url", url);

                tags.add_extra("reason", "missing required query param");
                tags.add_extra("param", key);
                return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
            }
        }
        for key in query_keys {
            if !ALL_CLICK_PARAMS.contains(key.as_str()) {
                trace!("invalid param key={:?} url={:?}", &key, url.to_string());
                tags.add_tag("type", species);
                tags.add_extra("tile", &tile.name);
                tags.add_extra("url", url);

                tags.add_extra("reason", "invalid query param");
                tags.add_extra("param", &key);
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
        let parsed: Url = match url.parse() {
            Ok(v) => v,
            Err(e) => {
                tags.add_tag("type", species);
                tags.add_extra("tile", &tile.name);
                tags.add_extra("url", url);
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
            trace!("missing param key=id url={:?}", url.to_string());
            tags.add_tag("type", species);
            tags.add_extra("tile", &tile.name);
            tags.add_extra("url", url);
            tags.add_extra("reason", "invalid query param");
            tags.add_extra("param", "id");
            return Err(HandlerErrorKind::InvalidHost(species, url.to_string()).into());
        }
        check_url(parsed, species, &filter.impression_hosts)?;
        Ok(())
    }

    /// Filter and process tiles from ADM:
    ///
    /// - Returns None for tiles that shouldn't be shown to the client
    /// - Modifies tiles for output to the client (adding additional fields, etc.)
    pub fn filter_and_process(
        &self,
        mut tile: AdmTile,
        location: &Location,
        device_info: &DeviceInfo,
        tags: &mut Tags,
        metrics: &Metrics,
    ) -> Option<Tile> {
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
                let include_regions = if filter.include_regions.is_empty() {
                    default
                } else {
                    filter
                };
                if !include_regions
                    .include_regions
                    .contains(&location.country())
                {
                    trace!(
                        "Rejecting tile: region {:?} not included",
                        location.country()
                    );
                    metrics.incr_with_tags("filter.adm.err.invalid_location", Some(tags));
                    return None;
                }
                // match to the version that we switched over from built in image management
                // to CDN image fetch. Note: iOS does not use the standard firefox version number

                if device_info.legacy_only()
                    && !self.legacy_list.contains(&tile.name.to_lowercase())
                {
                    trace!("Rejecting tile: Not a legacy advertiser {:?}", &tile.name);
                    metrics.incr_with_tags("filter.adm.err.non_legacy", Some(tags));
                    return None;
                }

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
                    trace!("Rejecting tile: bad adv");
                    metrics.incr_with_tags("filter.adm.err.invalid_advertiser", Some(tags));
                    self.report(&e, tags);
                    return None;
                }
                if let Err(e) = self.check_click(click_filter, &mut tile, tags) {
                    trace!("Rejecting tile: bad click");
                    metrics.incr_with_tags("filter.adm.err.invalid_click", Some(tags));
                    self.report(&e, tags);
                    return None;
                }
                if let Err(e) = self.check_impression(impression_filter, &mut tile, tags) {
                    trace!("Rejecting tile: bad imp");
                    metrics.incr_with_tags("filter.adm.err.invalid_impression", Some(tags));
                    self.report(&e, tags);
                    return None;
                }

                if let Err(e) = tile.image_url.parse::<Uri>() {
                    trace!("Rejecting tile: bad img: {:?}", e);
                    metrics.incr_with_tags("filter.adm.err.invalid_image", Some(tags));
                    self.report(
                        &HandlerErrorKind::InvalidHost("Image", tile.image_url).into(),
                        tags,
                    );
                    return None;
                }

                // Use the default.position (Option<u8>) if the filter.position (Option<u8>) isn't
                // defined. In either case `None` is a valid return, but we should favor `filter` over
                // `default`.
                Some(Tile::from_adm_tile(
                    tile,
                    filter.position.or(default.position),
                ))
            }
            None => {
                if !self.ignore_list.contains(&tile.name.to_lowercase()) {
                    metrics.incr_with_tags("filter.adm.err.unexpected_advertiser", Some(tags));
                    self.report(
                        &HandlerErrorKind::UnexpectedAdvertiser(tile.name).into(),
                        tags,
                    );
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::check_url;

    #[test]
    fn check_url_matches() {
        let species = "Click";
        assert!(check_url(
            "https://example.com".parse().unwrap(),
            species,
            &[vec!["example".to_owned(), "com".to_owned()]]
        )
        .unwrap());

        assert!(check_url(
            "https://foo.bridge.example.com/?quux=baz".parse().unwrap(),
            species,
            &[vec!["example".to_owned(), "com".to_owned()]]
        )
        .unwrap());
    }

    #[test]
    fn check_url_failed() {
        let species = "Click";
        assert!(check_url(
            "https://foo.com".parse().unwrap(),
            species,
            &[vec!["example".to_owned(), "com".to_owned()]]
        )
        .is_err());

        assert!(check_url(
            "https://foo.com".parse().unwrap(),
            species,
            &[vec![
                "bar".to_owned(),
                "example".to_owned(),
                "com".to_owned()
            ]]
        )
        .is_err());

        assert!(check_url(
            "https://badexample.com".parse().unwrap(),
            species,
            &[vec!["example".to_owned(), "com".to_owned()]]
        )
        .is_err());
    }

    #[test]
    fn check_mx_domains() {
        // Ensure that complex domains are validated correctly.
        assert!(check_url(
            "https://foo.co.mx".parse().unwrap(),
            "Click",
            &[
                vec!["bar".to_owned(), "co".to_owned(), "mx".to_owned()],
                vec!["bar".to_owned(), "com".to_owned()],
                vec!["foo".to_owned(), "co".to_owned(), "uk".to_owned()],
            ]
        )
        .is_err());
    }
}
