use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::Debug,
    iter::FromIterator,
    sync::Arc,
    time::Duration,
};

use actix_web::{http::Uri, rt};
use actix_web_location::Location;
use lazy_static::lazy_static;
use tokio::sync::RwLock;
use url::Url;

use super::{
    tiles::{AdmTile, Tile},
    AdmAdvertiserFilterSettings,
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
    pub advertiser_filters: HashMap<String, AdmAdvertiserFilterSettings>,
    /// Ignored (not included but also not reported to Sentry) Advertiser names
    pub ignore_list: HashSet<String>,
    /// Temporary list of advertisers with legacy images built into firefox
    /// for pre 91 tile support.
    pub legacy_list: HashSet<String>,
    pub source: Option<String>,
    pub source_url: Option<url::Url>,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
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

// Clippy complains about the lock being held across the await for `requires_update` and `update`.
// However these are fairly atomic, and do async calls on the shared filter. Not sure how to avoid
// those.
#[allow(clippy::await_holding_lock)]
/// Background updater.
///
pub fn spawn_updater(
    is_cloud: bool,
    refresh_rate: Duration,
    filter: &Arc<RwLock<AdmFilter>>,
    storage_client: cloud_storage::Client,
) -> HandlerResult<()> {
    {
        if !(is_cloud) {
            return Ok(());
        }
    }
    let mfilter = filter.clone();
    rt::spawn(async move {
        let mut tags = crate::tags::Tags::default();
        loop {
            {
                match mfilter.read().await.requires_update(&storage_client).await {
                    Ok(true) => {
                        let mut filter = mfilter.write().await;
                        filter.update(&storage_client).await.unwrap_or_else(|e| {
                            filter.report(&e, &mut tags);
                        });
                    }
                    Ok(false) => {}
                    Err(e) => {
                        mfilter.read().await.report(&e, &mut tags);
                    }
                }
            }
            rt::time::sleep(refresh_rate).await;
        }
    });
    Ok(())
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
        l_sentry::report(sentry::event_from_error(error), &merged_tags);
    }

    /// check to see if the bucket has been modified since the last time we updated.
    pub async fn requires_update(
        &self,
        storage_client: &cloud_storage::Client,
    ) -> HandlerResult<bool> {
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
            let obj = storage_client
                .object()
                .read(&host, bucket.path().trim_start_matches('/'))
                .await?;
            if let Some(updated) = self.last_updated {
                // if the bucket is older than when we last checked, do nothing.
                return Ok(updated <= obj.updated);
            };
            return Ok(true);
        }
        Ok(false)
    }

    /// Try to update the ADM filter data from the remote bucket.
    pub async fn update(&mut self, storage_client: &cloud_storage::Client) -> HandlerResult<()> {
        if let Some(bucket) = &self.source_url {
            let advertiser_filters =
                AdmFilter::advertisers_from_settings_bucket(storage_client, bucket)
                    .await
                    .map_err(|e| {
                        HandlerError::internal(&format!(
                            "Invalid bucket data in {:?}: {:?}",
                            self.source, e
                        ))
                    })?;
            for (adv, setting) in advertiser_filters {
                if setting.delete {
                    trace!("Removing advertiser {:?}", &adv);
                    self.advertiser_filters.remove(&adv.to_lowercase());
                };
                self.advertiser_filters.insert(adv.to_lowercase(), setting);
            }
            self.last_updated = Some(chrono::Utc::now());
        }
        Ok(())
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
                let paths = &filter
                    .paths
                    .clone()
                    .unwrap_or_else(|| [PathFilter::default()].to_vec());
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
        match self.advertiser_filters.get(&tile.name.to_lowercase()) {
            Some(filter) => {
                // Apply any additional tile filtering here.
                if filter.countries.get(&location.country()).is_none() {
                    trace!(
                        "Rejecting tile: region {:?} not included",
                        location.country()
                    );
                    metrics.incr_with_tags("filter.adm.err.invalid_location", Some(tags));
                    if self.excluded_countries_200 {
                        return Err(HandlerErrorKind::NoTilesForCountry(location.country()).into());
                    }
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

                let adv_filter = filter.countries.get(&location.country()).unwrap();
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
    use crate::adm::AdmDefaults;
    use crate::adm::{settings::AdvertiserUrlFilter, tiles::AdmTile};
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
        }"#;
        let advertiser_filters = AdmFilter::advertisers_from_string(s).unwrap();
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
            .get("acme")
            .unwrap()
            .countries
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
}
