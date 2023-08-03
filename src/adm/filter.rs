use std::{
    borrow::Cow, collections::HashSet, fmt::Debug, iter::FromIterator, sync::Arc, time::Duration,
};

use actix_web::{http::Uri, rt};
use actix_web_location::Location;
use cadence::{CountedExt, StatsdClient};
use google_cloud_storage::http::objects::{download::Range, get::GetObjectRequest};
use lazy_static::lazy_static;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use url::Url;

use super::{
    settings::AdmAdvertiserSettings,
    tiles::{AdmTile, Tile},
};
use crate::{
    adm::settings::{AdmDefaults, AdvertiserUrlFilter, PathFilter, PathMatching},
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
    static ref DEFAULT_PATH_FILTER: Vec<PathFilter> = vec![PathFilter::default()];
}

#[allow(rustdoc::private_intra_doc_links)]
/// Filter criteria for ADM Tiles
///
/// Each "filter"  is a set of [crate::adm::AdmAdvertiserFilterSettings] that are
/// specific to a given Advertiser name (the names are matched against
/// the tiles fetch request)

#[derive(Default, Clone, Debug)]
pub struct AdmFilter {
    /// Filter settings by Advertiser name
    pub advertiser_filters: AdmAdvertiserSettings,
    /// Ignored (not included but also not reported to Sentry) Advertiser names
    pub ignore_list: HashSet<String>,
    /// Temporary list of advertisers with legacy images built into firefox
    /// for pre 91 tile support.
    pub legacy_list: HashSet<String>,
    pub all_include_regions: HashSet<String>,
    pub source: Option<String>,
    pub source_url: Option<url::Url>,
    pub last_updated: Option<OffsetDateTime>,
    pub refresh_rate: Duration,
    pub defaults: AdmDefaults,
    pub excluded_countries_200: bool,
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

/// Background updater.
///
pub fn spawn_updater(
    is_cloud: bool,
    refresh_rate: Duration,
    filter: &Arc<RwLock<AdmFilter>>,
    storage_client: Arc<google_cloud_storage::client::Client>,
    metrics: Arc<StatsdClient>,
) -> HandlerResult<()> {
    {
        if !(is_cloud) {
            return Ok(());
        }
    }
    let mfilter = Arc::clone(filter);
    rt::spawn(async move {
        loop {
            updater(&mfilter, &storage_client, &metrics).await;
            rt::time::sleep(refresh_rate).await;
        }
    });
    Ok(())
}

/// Update `AdmFilter` from the Cloud Storage settings if they've been updated
async fn updater(
    filter: &Arc<RwLock<AdmFilter>>,
    storage_client: &google_cloud_storage::client::Client,
    metrics: &Arc<StatsdClient>,
) {
    // Do the check before matching so that the read lock can be released right away.
    let result = filter.read().await.fetch_new_settings(storage_client).await;
    match result {
        Ok(Some((new_settings, last_updated))) => {
            filter.write().await.update(new_settings, last_updated);
            trace!("AdmFilter updated from cloud storage");
            metrics.incr("filter.adm.update.ok").ok();
        }
        Ok(None) => {
            metrics.incr("filter.adm.update.check.skip").ok();
        }
        Err(e) => {
            trace!("AdmFilter update failed: {:?}", e);
            metrics.incr("filter.adm.update.check.error").ok();
            l_sentry::report(&e, &e.tags);
        }
    }
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
    fn report(&self, error: &HandlerError, tags: &mut Tags) {
        // trace!(&error, &tags);
        // TODO: if not error.is_reportable, just add to metrics.
        let mut merged_tags = error.tags.clone();
        merged_tags.extend(tags.clone());
        l_sentry::report(error, &merged_tags);
    }

    /// Check if the bucket has been modified since the last time we updated,
    /// returning new `AdmAdvertiserSettings` if so.
    pub async fn fetch_new_settings(
        &self,
        storage_client: &google_cloud_storage::client::Client,
    ) -> HandlerResult<Option<(AdmAdvertiserSettings, OffsetDateTime)>> {
        // don't update non-bucket versions (for now)
        if !self.is_cloud() {
            return Ok(None);
        }
        if let Some(bucket) = &self.source_url {
            let host = bucket
                .host()
                .ok_or_else(|| {
                    HandlerError::internal(&format!("Missing bucket Host {:?}", self.source))
                })?
                .to_string();
            let path = bucket.path().trim_start_matches('/');
            let request = GetObjectRequest {
                bucket: host,
                object: path.into(),
                ..Default::default()
            };
            let obj = storage_client.get_object(&request).await?;
            let Some(obj_updated) = obj.updated else {
                Err(HandlerErrorKind::General(format!("ADM Settings missing last updated timestamp")))?
            };
            if let Some(last_updated) = self.last_updated {
                // if the remote object is not newer than the local object, do nothing
                if obj_updated <= last_updated {
                    return Ok(None);
                }
            };

            let bytes = storage_client
                .download_object(&request, &Range::default())
                .await?;
            let contents = String::from_utf8(bytes).map_err(|e| {
                HandlerErrorKind::General(format!("Could not read ADM Settings: {:?}", e))
            })?;
            let new_settings = serde_json::from_str(&contents).map_err(|e| {
                HandlerErrorKind::General(format!("Could not read ADM Settings: {:?}", e))
            })?;
            return Ok(Some((new_settings, obj_updated)));
        }
        Ok(None)
    }

    /// Clear and update the ADM filter data from new `AdmAdvertiserSettings`
    pub fn update(
        &mut self,
        settings: AdmAdvertiserSettings,
        last_updated: OffsetDateTime,
    ) {
        self.all_include_regions.clear();
        self.advertiser_filters.adm_advertisers.clear();
        for (adv, setting) in settings.adm_advertisers {
            for country in setting.keys() {
                self.all_include_regions.insert(country.clone());
            }
            self.advertiser_filters
                .adm_advertisers
                .insert(adv.to_lowercase(), setting);
        }
        self.last_updated = Some(last_updated);
    }

    /// Check the advertiser URL
    fn check_advertiser(
        &self,
        filters: &Vec<AdvertiserUrlFilter>,
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

        for filter in filters {
            if host == filter.host {
                let paths = filter.paths.as_ref().unwrap_or(&DEFAULT_PATH_FILTER);
                for rule in paths {
                    match rule.matching {
                        // Note that the original path is used for exact matching
                        PathMatching::Exact if rule.value == parsed.path() => return Ok(()),
                        PathMatching::Prefix if path.starts_with(&rule.value) => return Ok(()),

                        _ => continue,
                    }
                }
            }
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
        defaults: &AdmDefaults,
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

        if !check_url(parsed, "Click", &defaults.click_hosts)? {
            trace!("bad url: url={:?}", url);
            tags.add_tag("type", species);
            tags.add_extra("tile", &tile.name);
            tags.add_extra("url", url);

            tags.add_extra("reason", "bad host");
            return Err(HandlerErrorKind::InvalidHost(species, host).into());
        }

        for key in &*REQ_CLICK_PARAMS {
            if !query_keys.contains(*key) {
                trace!("missing param: key={:?} url={:?}", &key, url);
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
                trace!("invalid param key={:?} url={:?}", &key, url);
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
        defaults: &AdmDefaults,
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
            trace!("missing param key=id url={:?}", url);
            tags.add_tag("type", species);
            tags.add_extra("tile", &tile.name);
            tags.add_extra("url", url);
            tags.add_extra("reason", "invalid query param");
            tags.add_extra("param", "id");
            let host = get_host(&parsed, species)?;
            return Err(HandlerErrorKind::InvalidHost(species, host).into());
        }
        check_url(parsed, species, &defaults.impression_hosts)?;
        Ok(())
    }

    /// Check the image URL to see if it's valid.
    ///
    /// This extends `filter_and_process`
    fn check_image_hosts(
        &self,
        defaults: &AdmDefaults,
        tile: &mut AdmTile,
        tags: &mut Tags,
    ) -> HandlerResult<()> {
        // if no hosts are defined, then accept all (this allows
        // for backward compatibility)
        if defaults.image_hosts.is_empty() {
            return Ok(());
        }
        let url = &tile.image_url;
        let species = "Image";
        let parsed = parse_url(url, species, &tile.name, tags)?;
        check_url(parsed, species, &defaults.image_hosts)?;
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
    ) -> HandlerResult<Option<Tile>> {
        // Use strict matching for now, eventually, we may want to use backwards expanding domain
        // searches, (.e.g "xyz.example.com" would match "example.com")
        match self
            .advertiser_filters
            .adm_advertisers
            .get(&tile.name.to_lowercase())
        {
            Some(filter) => {
                // Apply any additional tile filtering here.
                if filter.get(&location.country()).is_none() {
                    trace!(
                        "Rejecting tile: {:?} region {:?} not included",
                        &tile.name,
                        location.country()
                    );
                    metrics.incr_with_tags("filter.adm.err.invalid_location", Some(tags));
                    return Ok(None);
                }
                // match to the version that we switched over from built in image management
                // to CDN image fetch.

                if device_info.legacy_only()
                    && !self.legacy_list.contains(&tile.name.to_lowercase())
                {
                    trace!("Rejecting tile: Not a legacy advertiser {:?}", &tile.name);
                    metrics.incr_with_tags("filter.adm.err.non_legacy", Some(tags));
                    return Ok(None);
                }

                let adv_filter = filter.get(&location.country()).unwrap();
                if let Err(e) = self.check_advertiser(adv_filter, &mut tile, tags) {
                    trace!("Rejecting tile: bad adv");
                    metrics.incr_with_tags("filter.adm.err.invalid_advertiser", Some(tags));
                    self.report(&e, tags);
                    return Ok(None);
                }
                if let Err(e) = self.check_click(&self.defaults, &mut tile, tags) {
                    trace!("Rejecting tile: bad click");
                    metrics.incr_with_tags("filter.adm.err.invalid_click", Some(tags));
                    self.report(&e, tags);
                    return Ok(None);
                }
                if let Err(e) = self.check_impression(&self.defaults, &mut tile, tags) {
                    trace!("Rejecting tile: bad imp");
                    metrics.incr_with_tags("filter.adm.err.invalid_impression", Some(tags));
                    self.report(&e, tags);
                    return Ok(None);
                }
                if let Err(e) = self.check_image_hosts(&self.defaults, &mut tile, tags) {
                    trace!("Rejecting tile: bad image");
                    metrics.incr_with_tags("filter.adm.err.invalid_image_host", Some(tags));
                    self.report(&e, tags);
                    return Ok(None);
                }
                if let Err(e) = tile.image_url.parse::<Uri>() {
                    trace!("Rejecting tile: bad image: {:?}", e);
                    metrics.incr_with_tags("filter.adm.err.invalid_image", Some(tags));
                    self.report(
                        &HandlerErrorKind::InvalidHost("Image", tile.image_url).into(),
                        tags,
                    );
                    return Ok(None);
                }
                trace!("allowing tile {:?}", &tile.name);
                Ok(Some(Tile::from_adm_tile(tile)))
            }
            None => {
                if !self.ignore_list.contains(&tile.name.to_lowercase()) {
                    metrics.incr_with_tags("filter.adm.err.unexpected_advertiser", Some(tags));
                    self.report(
                        &HandlerErrorKind::UnexpectedAdvertiser(tile.name).into(),
                        tags,
                    );
                }
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{check_url, AdmFilter};
    use crate::adm::settings::AdmAdvertiserSettings;
    use crate::adm::{settings::AdvertiserUrlFilter, tiles::AdmTile};
    use crate::adm::{spawn_updater, AdmDefaults};
    use crate::tags::Tags;
    use crate::web::test::{find_metrics, MockTokenSourceProvider};
    use actix_web::rt;
    use cadence::{SpyMetricSink, StatsdClient};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::RwLock;
    use url::Url;

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
        let s = r#"{"adm_advertisers":{

            "Acme": {
                "US": [
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
                ]
            }
        }
    }"#;
        let advertiser_filters: AdmAdvertiserSettings = serde_json::from_str(s).unwrap();
        let filter = AdmFilter {
            advertiser_filters: advertiser_filters.clone(),
            defaults: AdmDefaults {
                click_hosts: [crate::adm::settings::break_hosts("example.com".to_owned())].to_vec(),
                image_hosts: [crate::adm::settings::break_hosts(
                    "cdn.example.org".to_owned(),
                )]
                .to_vec(),
                impression_hosts: [crate::adm::settings::break_hosts("example.net".to_owned())]
                    .to_vec(),
                ..Default::default()
            },
            ..Default::default()
        };
        let settings = advertiser_filters
            .adm_advertisers
            .get("acme")
            .unwrap()
            .get("US")
            .unwrap();
        let mut tags = Tags::default();

        let mut tile = AdmTile {
            id: 0,
            name: "test".to_owned(),
            advertiser_url: "https://acme.biz/ca/foobar".to_owned(),
            click_url: "https://example.com/foo".to_owned(),
            image_url: "https://example.org/i/cat.jpg".to_owned(),
            impression_url: "https://example.net".to_owned(),
            position: None,
        };

        // Good, contains the right lede and path
        assert!(filter
            .check_advertiser(settings, &mut tile, &mut tags,)
            .is_ok());

        // Good, missing lede
        tile.advertiser_url = "https://acme.biz/ca/".to_owned();
        assert!(filter
            .check_advertiser(settings, &mut tile, &mut tags)
            .is_ok());
        // Good, missing last slash
        tile.advertiser_url = "https://acme.biz/ca".to_owned();
        assert!(filter
            .check_advertiser(settings, &mut tile, &mut tags)
            .is_ok());

        // Bad, path isn't correct.
        tile.advertiser_url = "https://acme.biz/calzone".to_owned();
        assert!(filter
            .check_advertiser(settings, &mut tile, &mut tags)
            .is_err());
        //Bad, wrong path
        tile.advertiser_url = "https://acme.biz/fr/".to_owned();
        assert!(filter
            .check_advertiser(settings, &mut tile, &mut tags)
            .is_err());

        //Good, extra element in host
        tile.advertiser_url = "https://black_friday.acme.biz/ca/".to_owned();
        assert!(filter
            .check_advertiser(settings, &mut tile, &mut tags)
            .is_ok());

        //Good, extra matching
        tile.advertiser_url = "https://acme.biz/usa".to_owned();
        assert!(filter
            .check_advertiser(settings, &mut tile, &mut tags)
            .is_ok());

        // Bad, path doesn't match exactly
        tile.advertiser_url = "https://acme.biz/usa/".to_owned();
        assert!(filter
            .check_advertiser(settings, &mut tile, &mut tags)
            .is_err());

        // "Traditional host. "
        let s = r#"[
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
            ]"#;
        let settings: Vec<AdvertiserUrlFilter> = serde_json::from_str(s).unwrap();
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

        // replicate settings breaking hosts into component bits.
        let host_bits: Vec<String> = "example.org"
            .to_owned()
            .split('.')
            .map(String::from)
            .collect();

        let defaults = AdmDefaults {
            image_hosts: vec![host_bits],
            ..Default::default()
        };
        tile.image_url = "https://example.biz".to_owned();
        assert!(filter
            .check_image_hosts(&defaults, &mut tile, &mut tags)
            .is_err());
        tile.image_url = "https://example.org".to_owned();
        assert!(filter
            .check_image_hosts(&defaults, &mut tile, &mut tags)
            .is_ok());
        // check that sub-hosts are not rejected.
        tile.image_url = "https://cdn.example.org".to_owned();
        assert!(filter
            .check_image_hosts(&defaults, &mut tile, &mut tags)
            .is_ok());
    }
    #[actix_web::test]
    async fn check_advertiser_metrics() {
        let s = r#"{"adm_advertisers":{
            "Acme": {
                "US": [
                {
                    "host": "acme.biz",
                    "paths": [
                        { "value": "/ca/", "matching": "prefix" }
                    ]
                }
              ]
            }
        }
    }"#;
        let advertiser_filters: AdmAdvertiserSettings = serde_json::from_str(s).unwrap();
        let filter = AdmFilter {
            advertiser_filters: advertiser_filters.clone(),
            defaults: AdmDefaults {
                ..Default::default()
            },
            source_url: Some(Url::parse("https://example.net").unwrap()),
            ..Default::default()
        };
        let refresh_rate = Duration::from_secs(9999999999);
        let adm_filter = Arc::new(RwLock::new(filter));

        let (rx, sink) = SpyMetricSink::new();

        spawn_updater(
            true,
            refresh_rate,
            &adm_filter,
            Arc::new(google_cloud_storage::client::Client::new(
                google_cloud_storage::client::ClientConfig {
                    token_source_provider: Box::new(MockTokenSourceProvider),
                    ..Default::default()
                },
            )),
            Arc::new(StatsdClient::builder("contile", sink).build()),
        )
        .unwrap();
        rt::time::sleep(Duration::from_secs(1)).await;

        let prefixes = &["contile.filter.adm.update.check.skip"];
        let metrics = find_metrics(&rx, prefixes);
        assert_eq!(metrics.len(), 1);
    }
}
