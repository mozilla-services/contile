use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::Debug,
    iter::FromIterator,
    sync::{Arc, RwLock},
};

use actix_http::http::Uri;
use actix_web_location::Location;
use lazy_static::lazy_static;
use url::Url;

use super::{
    tiles::{AdmTile, Tile},
    AdmAdvertiserFilterSettings, AdmSettings, DEFAULT,
};
use crate::{
    adm::settings::PathMatching,
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
    /// All countries set for inclusion in at least one of the advertiser regions
    /// [crate::adm::AdmAdvertiserFilterSettings]
    pub all_include_regions: HashSet<String>,
    /// Temporary list of advertisers with legacy images built into firefox
    /// for pre 91 tile support.
    pub legacy_list: HashSet<String>,
    pub source: String,
    pub source_url: Option<url::Url>,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
    pub refresh_rate: std::time::Duration,
}

/// Parse &str into a `Url`
fn parse_url(
    url: &str,
    species: &'static str,
    tile_name: &str,
    tags: &mut Tags,
) -> HandlerResult<Url> {
    Url::parse(url).map_err(|e| {
        tags.add_tag("type", species);
        tags.add_extra("tile", tile_name);
        tags.add_extra("url", url);
        tags.add_extra("parse_error", &e.to_string());
        HandlerErrorKind::InvalidHost(species, "Url::parse failed".to_owned()).into()
    })
}

/// Extract the host from Url
fn get_host(url: &Url, species: &'static str) -> HandlerResult<String> {
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
    let host = get_host(&url, species)?;
    let domains: Vec<_> = host.split('.').collect();
    for allowed in filter {
        let begin = domains.len() - allowed.len().min(domains.len());
        if &domains[begin..] == allowed {
            return Ok(true);
        }
    }
    Err(HandlerErrorKind::UnexpectedHost(species, host).into())
}

pub fn spawn_updater(filter: &Arc<RwLock<AdmFilter>>) {
    if !filter.read().unwrap().is_cloud() {
        return;
    }
    let mfilter = filter.clone();
    actix_rt::spawn(async move {
        let tags = crate::tags::Tags::default();
        loop {
            let mut filter = mfilter.write().unwrap();
            match filter.requires_update().await {
                Ok(true) => filter.update().await.unwrap_or_else(|e| {
                    filter.report(&e, &tags);
                }),
                Ok(false) => {}
                Err(e) => {
                    filter.report(&e, &tags);
                }
            }
            actix_rt::time::delay_for(filter.refresh_rate).await;
        }
    });
}

/// Filter a given tile data set provided by ADM and validate the various elements
impl AdmFilter {
    /// convenience function to determine if settings are cloud ready.
    pub fn is_cloud(&self) -> bool {
        if let Some(source) = &self.source_url {
            return source.scheme() == "gs";
        }
        false
    }

    /// Report the error directly to sentry
    fn report(&self, error: &HandlerError, tags: &Tags) {
        // trace!(&error, &tags);
        // TODO: if not error.is_reportable, just add to metrics.
        let mut merged_tags = error.tags.clone();
        merged_tags.extend(tags.clone());
        l_sentry::report(&merged_tags, sentry::event_from_error(error));
    }

    /// check to see if the bucket has been modified since the last time we updated.
    pub async fn requires_update(&self) -> HandlerResult<bool> {
        // don't update non-bucket versions (for now)
        if !self.is_cloud() {
            return Ok(false);
        }
        if let Some(bucket) = &self.source_url {
            let host = bucket
                .host()
                .ok_or_else(|| {
                    HandlerError::internal(&format!("Missing bucket Host {:?}", self.source))
                })?
                .to_string();
            let obj =
                cloud_storage::Object::read(&host, bucket.path().trim_start_matches('/')).await?;
            if let Some(updated) = self.last_updated {
                // if the bucket is older than when we last checked, do nothing.
                return Ok(updated <= obj.updated);
            };
            return Ok(true);
        }
        Ok(false)
    }

    /// Try to update the ADM filter data from the remote bucket.
    pub async fn update(&mut self) -> HandlerResult<()> {
        if let Some(bucket) = &self.source_url {
            let adm_settings = AdmSettings::from_settings_bucket(bucket)
                .await
                .map_err(|e| {
                    HandlerError::internal(&format!(
                        "Invalid bucket data in {:?}: {:?}",
                        self.source, e
                    ))
                })?;
            for (adv, setting) in adm_settings.advertisers {
                if setting.delete {
                    trace!("Removing advertiser {:?}", &adv);
                    self.filter_set.remove(&adv.to_lowercase());
                };
                trace!("Processing records for {:?}", &adv);
                // DEFAULT included but sans special processing -- close enough
                for country in &setting.include_regions {
                    if !self.all_include_regions.contains(country) {
                        self.all_include_regions.insert(country.clone());
                    }
                }
                // map the settings to the URL we're going to be checking
                self.filter_set.insert(adv.to_lowercase(), setting);
            }
            self.last_updated = Some(chrono::Utc::now());
        }
        Ok(())
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
        let parsed = parse_url(url, species, &tile.name, tags)?;
        let host = get_host(&parsed, species)?;

        if parsed.scheme().to_lowercase() != "https" {
            tags.add_tag("type", species);
            tags.add_extra("tile", &tile.name);
            tags.add_extra("url", url);
            return Err(HandlerErrorKind::InvalidHost(species, host).into());
        }

        // do a quick string comparison between the supplied host and the provided filter.
        let mut path = Cow::from(parsed.path());
        if !path.ends_with('/') {
            path.to_mut().push('/');
        }

        for filter in &filter.advertiser_urls {
            if !host.eq(&filter.host) {
                continue;
            }

            if let Some(ref paths) = filter.paths {
                for rule in paths {
                    match rule.matching {
                        // Note that the orignal path is used for exact matching
                        PathMatching::Exact if rule.value == parsed.path() => return Ok(()),
                        PathMatching::Prefix if path.starts_with(&rule.value) => return Ok(()),
                        _ => continue,
                    }
                }
            } else {
                // Host matches without any path filters, matching succeeds.
                return Ok(());
            };
        }

        tags.add_tag("type", species);
        tags.add_extra("tile", &tile.name);
        tags.add_extra("url", url);
        Err(HandlerErrorKind::InvalidHost(species, host).into())
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
        let parsed = parse_url(url, species, &tile.name, tags)?;
        let host = get_host(&parsed, species)?;
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
            return Err(HandlerErrorKind::InvalidHost(species, host).into());
        }
        for key in &*REQ_CLICK_PARAMS {
            if !query_keys.contains(*key) {
                trace!("missing param: key={:?} url={:?}", &key, url.to_string());
                tags.add_tag("type", species);
                tags.add_extra("tile", &tile.name);
                tags.add_extra("url", url);

                tags.add_extra("reason", "missing required query param");
                tags.add_extra("param", key);
                return Err(HandlerErrorKind::InvalidHost(species, host).into());
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
                return Err(HandlerErrorKind::InvalidHost(species, host).into());
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
        let parsed = parse_url(url, species, &tile.name, tags)?;
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
            let host = get_host(&parsed, species)?;
            return Err(HandlerErrorKind::InvalidHost(species, host).into());
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

                let adv_filter = if filter.advertiser_urls.is_empty() {
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
    use crate::adm::tiles::AdmTile;
    use crate::adm::AdmAdvertiserFilterSettings;
    use crate::tags::Tags;

    use super::{check_url, AdmFilter};

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

    #[test]
    fn check_advertiser() {
        let s = r#"{
            "advertiser_urls": [
                {
                    "host": "acme.biz",
                    "paths": [
                        { "value": "/ca/", "matching": "prefix" }
                    ]
                },
                {
                    "host": "black_friday.acme.biz",
                    "paths": [
                        { "value": "/ca/", "matching": "prefix" }
                    ]

                },
                {
                    "host": "acme.biz",
                    "paths": [
                        { "value": "/usa", "matching": "exact" }
                    ]
                }
            ],
            "position": 0
        }"#;
        let settings: AdmAdvertiserFilterSettings = serde_json::from_str(s).unwrap();
        let filter = AdmFilter::default();
        let mut tags = Tags::default();

        let mut tile = AdmTile {
            id: 0,
            name: "test".to_owned(),
            advertiser_url: "https://acme.biz/ca/foobar".to_owned(),
            click_url: "https://example.com/foo".to_owned(),
            image_url: "".to_owned(),
            impression_url: "https://example.net".to_owned(),
            position: None,
        };

        // Good, contains the right lede and path
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags,)
            .is_ok());

        // Good, missing lede
        tile.advertiser_url = "https://acme.biz/ca/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_ok());
        // Good, missing last slash
        tile.advertiser_url = "https://acme.biz/ca".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_ok());

        // Bad, path isn't correct.
        tile.advertiser_url = "https://acme.biz/calzone".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_err());
        //Bad, wrong path
        tile.advertiser_url = "https://acme.biz/fr/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_err());

        //Good, extra element in host
        tile.advertiser_url = "https://black_friday.acme.biz/ca/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_ok());

        //Good, extra matching
        tile.advertiser_url = "https://acme.biz/usa".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_ok());

        // Bad, path doesn't match exactly
        tile.advertiser_url = "https://acme.biz/usa/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_err());

        // "Traditional host. "
        let s = r#"{
            "advertiser_urls": [
                {
                    "host": "acme.biz",
                    "paths": [
                        { "value": "/ca/", "matching": "prefix" }
                    ]
                },
                {
                    "host": "www.acme.co",
                    "paths": [
                        { "value": "/foo.bar/", "matching": "prefix" }
                    ]

                },
                {
                    "host": "acme.biz",
                    "paths": [
                        { "value": "/", "matching": "exact" }
                    ]
                }
            ],
            "position": 0
        }"#;
        let settings: AdmAdvertiserFilterSettings = serde_json::from_str(s).unwrap();
        // Good, matches hosts
        tile.advertiser_url = "https://acme.biz/ca/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_ok());
        tile.advertiser_url = "https://www.acme.co/foo.bar/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_ok());
        tile.advertiser_url = "https://acme.biz/foo.bar/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_err());

        // Bad, invalid host.
        tile.advertiser_url = "https://acme.biz.uk/ca/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_err());
        // Bad, host in path.
        tile.advertiser_url = "https://example.com/acme.biz/ca/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_err());

        //Good, dotted path
        tile.advertiser_url = "https://www.acme.co/foo.bar/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_ok());

        //Bad, invalid scheme
        tile.advertiser_url = "http://www.acme.co/foo.bar/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_err());

        //Good, matches exact host and path
        tile.advertiser_url = "https://acme.biz/".to_owned();
        assert!(filter
            .check_advertiser(&settings, &mut tile, &mut tags)
            .is_ok());
    }
}
