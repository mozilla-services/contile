//! Fetch and store a given remote image into Google Storage for CDN caching
use std::{env, io::Cursor, sync::Arc};

use actix_http::http::HeaderValue;
use actix_web::{http::uri, web::Bytes};
use cadence::{CountedExt, StatsdClient};
use chrono::{DateTime, Duration, Utc};
use cloud_storage::Bucket;
use dashmap::DashMap;
use image::{io::Reader as ImageReader, ImageFormat};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    error::{HandlerError, HandlerErrorKind, HandlerResult},
    settings::Settings,
    tags::Tags,
};

/// These values generally come from the Google console for Cloud Storage.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ImageMetricSettings {
    /// maximum length of the image
    max_size: u64,
    /// max image height
    max_height: u64,
    max_width: u64,
    min_height: u64,
    min_width: u64,
    symmetric: bool,
}

impl Default for ImageMetricSettings {
    fn default() -> Self {
        Self {
            max_size: 1_600_000,
            max_height: 256,
            max_width: 256,
            min_height: 96,
            min_width: 96,
            symmetric: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct StorageSettings {
    /// The GCP Cloud storage project name
    project_name: String,
    /// The Bucket name for this data
    bucket_name: String,
    /// The external CDN host
    cdn_host: String,
    /// The bucket TTL is determined by the policy set for the given bucket when it's created.
    bucket_ttl: Option<u64>,
    /// The max time to live for cached data, ~ 15 days.
    cache_ttl: u64,
    /// Max dimensions for an image
    metrics: ImageMetricSettings,
    /// Max request time (in seconds)
    request_timeout: u64,
    /// Max connection timeout (in seconds)
    connection_timeout: u64,
}

/// Instantiate from [Settings]
impl From<&Settings> for StorageSettings {
    fn from(settings: &Settings) -> Self {
        lazy_static! {
            /// https://cloud.google.com/storage/docs/naming-buckets#requirements
            /// Ignoring buckets with . in them because of domain verification requirements
            static ref VALID: Regex = Regex::new("^[0-9a-z][0-9a-z_-]{1,61}[0-9a-z]$").unwrap();
        }
        if settings.storage.is_empty() {
            return Self::default();
        }
        let storage_settings: StorageSettings =
            serde_json::from_str(&settings.storage).expect("Invalid storage settings");
        if !VALID.is_match(&storage_settings.bucket_name) {
            panic!(
                "Invalid storage settings: invalid bucket name '{}'",
                &storage_settings.bucket_name
            )
        }
        storage_settings
    }
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            project_name: "topsites-nonprod".to_owned(),
            bucket_name: "moz-topsites-stage-cdn".to_owned(),
            cdn_host: "https://cdn.stage.topsites.nonprod.cloudops.mozgcp.net/".to_owned(),
            bucket_ttl: None,
            cache_ttl: 86400 * 15,
            metrics: ImageMetricSettings::default(),
            request_timeout: 3,
            connection_timeout: 3,
        }
    }
}

/// Image storage container
#[derive(Clone)]
pub struct ImageStore {
    // No `Default` stated for `ImageStore` because we *ALWAYS* want a timeout
    // for the `reqwest::Client`
    //
    // bucket isn't really needed here, since `Object` stores and manages itself,
    // but it may prove useful in future contexts.
    //
    // bucket: Option<cloud_storage::Bucket>,
    settings: StorageSettings,
    // `Settings::tiles_ttl`
    tiles_ttl: u32,
    cadence_metrics: StatsdClient,
    req: reqwest::Client,
    /// `StoredImage`s already fetched/uploaded
    stored_images: Arc<DashMap<uri::Uri, StoredImage>>,
}

/// Stored image information, suitable for determining the URL to present to the CDN
#[derive(Clone, Debug)]
pub struct StoredImage {
    pub url: uri::Uri,
    pub image_metrics: ImageMetrics,
    expiry: DateTime<Utc>,
}

impl StoredImage {
    /// Whether this image should be refetched and checked against the Cloud
    /// Storage Bucket
    fn expired(&self) -> bool {
        self.expiry <= Utc::now()
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Default, Serialize, PartialEq)]
pub struct ImageMetrics {
    pub width: u32,
    pub height: u32,
    pub size: usize,
}

/// Store a given image into Google Storage
impl ImageStore {
    /// Connect and optionally create a new Google Storage bucket based off [Settings]
    pub async fn create(
        settings: &Settings,
        cadence_metrics: &StatsdClient,
        client: &reqwest::Client,
    ) -> HandlerResult<Option<Self>> {
        let sset = StorageSettings::from(settings);
        Self::check_bucket(&sset, settings.tiles_ttl, cadence_metrics, client).await
    }

    pub async fn check_bucket(
        settings: &StorageSettings,
        tiles_ttl: u32,
        cadence_metrics: &StatsdClient,
        client: &reqwest::Client,
    ) -> HandlerResult<Option<Self>> {
        if env::var("SERVICE_ACCOUNT").is_err()
            && env::var("GOOGLE_APPLICATION_CREDENTIALS").is_err()
        {
            trace!("No auth credentials set. Not storing...");
            return Ok(None);
        }

        // https://cloud.google.com/storage/docs/naming-buckets
        // don't try to open an empty bucket
        let empty = ["", "none"];
        if empty.contains(&settings.bucket_name.to_lowercase().as_str())
            || empty.contains(&settings.project_name.to_lowercase().as_str())
        {
            trace!("No bucket set. Not storing...");
            return Ok(None);
        }

        // The image storage bucket should be created ahead of time.
        // Bucket creation permissions would require "Storage Object Creator" account
        // permissions, which can be tricky to set up.
        //
        // Once created, the bucket should be set with "AllViewers" with
        // "allUsers" set to `ObjectViewer` to expose the contents of the bucket
        // to public view.
        //

        let _content = Bucket::read_with(&settings.bucket_name, client)
            .await
            .map_err(|e| HandlerError::internal(&format!("Could not read bucket {:?}", e)))?;

        trace!("Bucket OK");

        Ok(Some(Self {
            // bucket: Some(bucket),
            settings: settings.clone(),
            tiles_ttl,
            cadence_metrics: cadence_metrics.clone(),
            req: client.clone(),
            stored_images: Default::default(),
        }))
    }

    pub fn meta(
        &self,
        uri: &uri::Uri,
        image: &Bytes,
        fmt: ImageFormat,
    ) -> HandlerResult<ImageMetrics> {
        let mut reader = ImageReader::new(Cursor::new(image));
        reader.set_format(fmt);
        let img = reader.decode().map_err(|e| {
            let mut tags = Tags::default();
            tags.add_extra("error", &e.to_string());
            tags.add_extra("url", &uri.to_string());
            tags.add_extra("format", fmt.extensions_str().first().unwrap_or(&"Unknown"));
            let mut err: HandlerError = HandlerErrorKind::BadImage("Image unreadable").into();
            err.tags = tags;
            err
        })?;
        let rgb_img = img.to_rgb16();
        Ok(ImageMetrics {
            height: rgb_img.height(),
            width: rgb_img.width(),
            size: rgb_img.len(),
        })
    }

    /// Store an image fetched from the passed `uri` into Google Cloud Storage
    ///
    /// This will fetch and store the image into the bucket if necessary (fetch
    /// results are cached for a short time).
    pub async fn store(&self, uri: &uri::Uri) -> HandlerResult<StoredImage> {
        if let Some(stored_image) = self.stored_images.get(uri) {
            if !stored_image.expired() {
                return Ok(stored_image.clone());
            }
        }
        let (image, content_type) = self.fetch(uri).await?;
        let metrics = self.validate(uri, &image, &content_type).await?;
        let stored_image = self.upload(image, &content_type, metrics).await?;
        self.stored_images
            .insert(uri.to_owned(), stored_image.clone());
        Ok(stored_image)
    }

    /// Generate a unique hash based on the content of the image
    pub fn as_hash(&self, source: &Bytes) -> String {
        base64::encode_config(blake3::hash(source).as_bytes(), base64::URL_SAFE_NO_PAD)
    }

    /// Fetch the bytes for an image based on a URI
    pub(crate) async fn fetch(&self, uri: &uri::Uri) -> HandlerResult<(Bytes, String)> {
        trace!("fetching... {:?}", &uri);
        self.cadence_metrics.incr("image.fetch").ok();

        let res = self
            .req
            .get(&uri.to_string())
            .timeout(std::time::Duration::from_secs(
                self.settings.request_timeout,
            ))
            .send()
            .await?
            .error_for_status()?;
        trace!(
            "image type: {:?}, size: {:?}",
            res.headers().get("content-type"),
            res.content_length()
        );

        let mut content_type: &str = "image/jpg";
        let default_type = HeaderValue::from_str(content_type).unwrap();
        let headers = res.headers().clone();
        content_type = headers
            .get("content-type")
            .unwrap_or(&default_type)
            .to_str()
            .unwrap_or(content_type);

        trace!("Reading...");
        Ok((res.bytes().await?, content_type.to_owned()))
    }

    /// Check if a given image byte set is "valid" according to our settings.
    pub(crate) async fn validate(
        &self,
        uri: &uri::Uri,
        image: &Bytes,
        content_type: &str,
    ) -> HandlerResult<ImageMetrics> {
        // `image` can't currently handle svg
        let image_metrics = if "image/svg" == content_type.to_lowercase().as_str() {
            // svg images are vector based, so we can set the size to whatever we want.
            ImageMetrics {
                width: 128,
                height: 128,
                size: image.len(),
            }
        } else {
            // Otherwise we get the images metrics.
            let fmt = match content_type.to_lowercase().as_str() {
                "image/jpg" | "image/jpeg" => ImageFormat::Jpeg,
                "image/png" => ImageFormat::Png,
                _ => {
                    let mut tags = Tags::default();
                    tags.add_extra("url", &uri.to_string());
                    tags.add_extra("format", content_type);
                    let mut err: HandlerError =
                        HandlerErrorKind::BadImage("Invalid image format").into();
                    err.tags = tags;
                    return Err(err);
                }
            };
            self.meta(uri, image, fmt)?
        };
        if self.settings.metrics.symmetric && image_metrics.width != image_metrics.height {
            let mut tags = Tags::default();
            tags.add_extra("metrics", &format!("{:?}", image_metrics));
            tags.add_extra("url", &uri.to_string());
            let mut err: HandlerError = HandlerErrorKind::BadImage("Non symmetric image").into();
            err.tags = tags;
            return Err(err);
        }
        // Check image meta sizes
        if !(self.settings.metrics.min_width..=self.settings.metrics.max_width)
            .contains(&(image_metrics.width as u64))
            || !(self.settings.metrics.min_height..=self.settings.metrics.max_height)
                .contains(&(image_metrics.height as u64))
            || (image_metrics.size as u64) > self.settings.metrics.max_size
        {
            let mut tags = Tags::default();
            tags.add_extra("metrics", &format!("{:?}", image_metrics));
            tags.add_extra("url", &uri.to_string());
            let mut err: HandlerError = HandlerErrorKind::BadImage("Invalid image size").into();
            err.tags = tags;
            return Err(err);
        }
        Ok(image_metrics)
    }

    /// upload an image to Google Cloud Storage
    pub(crate) async fn upload(
        &self,
        image: Bytes,
        content_type: &str,
        image_metrics: ImageMetrics,
    ) -> HandlerResult<StoredImage> {
        if self.settings.bucket_name.is_empty() {
            return Err(HandlerError::internal("No storage bucket defined"));
        }

        // image source paths tend to be
        // "https://<remote_host>/account/###/###/####.jpg"
        // They may be unreliable as a hash source, so use the image bytes.
        let image_path = format!(
            "{}.{}.{}",
            self.as_hash(&image),
            image.len(),
            match content_type {
                "image/jpg" | "image/jpeg" => "jpg",
                "image/png" => "png",
                "image/svg" => "svg",
                _ => ".oct",
            }
        );

        // check to see if image has already been stored.
        self.cadence_metrics.incr("image.object.check").ok();
        if let Ok(exists) =
            cloud_storage::Object::read_with(&self.settings.bucket_name, &image_path, &self.req)
                .await
        {
            trace!("Found existing image in bucket: {:?}", &exists.media_link);
            return Ok(self.new_image(
                format!("{}/{}", &self.settings.cdn_host, &image_path).parse()?,
                image_metrics,
                exists.time_created,
            ));
        }

        // store new data to the googles
        self.cadence_metrics.incr("image.object.create").ok();
        match cloud_storage::Object::create_with_params(
            &self.settings.bucket_name,
            image.to_vec(),
            &image_path,
            content_type,
            Some(&[("ifGenerationMatch", "0")]),
            Some(self.req.clone()),
        )
        .await
        {
            Ok(mut object) => {
                object.content_disposition = Some("inline".to_owned());
                object.cache_control = Some(format!("public, max-age={}", self.settings.cache_ttl));
                self.cadence_metrics.incr("image.object.update").ok();
                object.update().await?;
                let url = format!("{}/{}", &self.settings.cdn_host, &image_path);
                trace!("Stored to {:?}: {:?}", &object.self_link, &url);
                Ok(self.new_image(url.parse()?, image_metrics, object.time_created))
            }
            Err(e) => {
                if let cloud_storage::Error::Other(ref json) = e {
                    // NOTE: cloud_storage doesn't parse the Google response
                    // correctly so they seem to come up as the Other variant
                    let body: serde_json::Value = serde_json::from_str(json).map_err(|e| {
                        HandlerError::internal(&format!(
                            "Could not parse cloud_storage::Error::Other: ({:?}) {:?}",
                            e, json
                        ))
                    })?;
                    if body["error"]["code"].as_i64() == Some(412) {
                        // 412 Precondition Failed: the image already exists, so we
                        // can continue on
                        trace!("Store Precondition Failed (412), image already exists, continuing");
                        self.cadence_metrics
                            .incr("image.object.already_exists")
                            .ok();
                        let url = format!("{}/{}", &self.settings.cdn_host, &image_path);
                        return Ok(self.new_image(
                            url.parse()?,
                            image_metrics,
                            // approximately (close enough)
                            Utc::now(),
                        ));
                    }
                }
                Err(e.into())
            }
        }
    }

    fn new_image(
        &self,
        url: uri::Uri,
        image_metrics: ImageMetrics,
        time_created: DateTime<Utc>,
    ) -> StoredImage {
        // Images should not change (any image modification should result in a
        // new url from upstream). However, poll it every `Settings::tiles_ttl`
        // anyway, just in case
        let mut expiry = Utc::now() + Duration::seconds(self.tiles_ttl.into());
        if let Some(bucket_ttl) = self.settings.bucket_ttl {
            // Take `StorageSettings::bucket_ttl` into account in the rare case
            // it's set the image to expire earlier than now + `tiles_ttl`
            expiry = std::cmp::min(expiry, time_created + Duration::seconds(bucket_ttl as i64));
        }
        StoredImage {
            url,
            image_metrics,
            expiry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::settings::test_settings;
    use actix_http::http::Uri;
    use cadence::{NopMetricSink, SpyMetricSink};
    use rand::Rng;

    fn set_env() {
        // sometimes the IDE doesn't include the host env vars.
        // this lets you set them proactively.

        // std::env::set_var("GOOGLE_APPLICATION_CREDENTIALS","/home/jrconlin/.ssh/keys/sync-spanner-dev.json");
    }

    fn test_storage_settings() -> StorageSettings {
        let project =
            std::env::var("CONTILE_TEST_PROJECT").unwrap_or_else(|_| "topsites_nonprod".to_owned());
        let bucket =
            std::env::var("CONTILE_TEST_BUCKET").unwrap_or_else(|_| "moz_test_bucket".to_owned());
        let cdn = std::env::var("CONTILE_TEST_CDN_HOST")
            .unwrap_or_else(|_| "https://example.com".to_owned());
        StorageSettings {
            project_name: project,
            bucket_name: bucket,
            bucket_ttl: None,
            cache_ttl: 86400 * (15 + 1),
            cdn_host: cdn,
            ..Default::default()
        }
    }

    fn test_store() -> ImageStore {
        let settings = test_storage_settings();
        let timeout = std::time::Duration::from_secs(settings.request_timeout);
        ImageStore {
            settings,
            tiles_ttl: 15 * 60,
            cadence_metrics: StatsdClient::builder("", NopMetricSink).build(),
            req: reqwest::Client::builder()
                .connect_timeout(timeout)
                .build()
                .unwrap(),
            stored_images: Default::default(),
        }
    }

    fn test_image_buffer(height: u32, width: u32) -> Bytes {
        let mut rng = rand::thread_rng();
        // generate a garbage image.
        let img = image::ImageBuffer::from_fn(width, height, |_x, _y| {
            image::Rgb([
                rng.gen_range(0..255),
                rng.gen_range(0..255),
                rng.gen_range(0..255),
            ])
        });
        let buf: Vec<u8> = Vec::new();
        let mut out = std::io::Cursor::new(buf);
        let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut out);
        encoder
            .encode(&img.into_raw(), width, height, image::ColorType::Rgb8)
            .unwrap();
        Bytes::from(out.into_inner())
    }

    #[test]
    fn test_config() {
        let test_val = r#"{"project_name": "project", "bucket_name": "bucket"}"#;

        let mut setting = test_settings();
        setting.storage = test_val.to_owned();
        let store_set: StorageSettings = (&setting).into();

        assert!(store_set.project_name == *"project");
        assert!(store_set.bucket_name == *"bucket");
    }

    #[tokio::test]
    async fn test_image_proc() -> Result<(), ()> {
        if std::env::var("GOOGLE_APPLICATION_CREDENTIALS").is_err() {
            print!("Skipping test: No credentials found.");
            return Ok(());
        }
        let src_img = "https://evilonastick.com/test/128px.jpg";

        let test_settings = test_storage_settings();
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(
                test_settings.request_timeout,
            ))
            .build()
            .unwrap();
        let img_store = ImageStore::check_bucket(
            &test_settings,
            15 * 60,
            &StatsdClient::builder("", NopMetricSink).build(),
            &client,
        )
        .await
        .unwrap()
        .unwrap();
        let target = src_img.parse::<Uri>().unwrap();
        img_store.store(&target).await.expect("Store failed");
        Ok(())
    }

    #[tokio::test]
    async fn test_image_validate() -> Result<(), ()> {
        set_env();
        let test_valid_image = test_image_buffer(96, 96);
        let test_uri: Uri = "https://example.com/test.jpg".parse().unwrap();
        let img_store = test_store();
        let result = img_store
            .validate(&test_uri, &test_valid_image, "image/jpg")
            .await
            .unwrap();

        assert!(result.height == 96);
        assert!(result.width == 96);
        Ok(())
    }

    #[tokio::test]
    async fn test_image_invalidate_offsize() -> Result<(), ()> {
        set_env();
        let test_valid_image = test_image_buffer(96, 100);
        let test_uri: Uri = "https://example.com/test.jpg".parse().unwrap();
        let img_store = test_store();
        assert!(img_store
            .validate(&test_uri, &test_valid_image, "image/jpg")
            .await
            .is_err());

        Ok(())
    }

    #[test]
    #[should_panic]
    fn test_invalid_bucket() {
        let test_val = r#"{"project_name": "project", "bucket_name": "+bucket"}"#;

        let mut setting = test_settings();
        setting.storage = test_val.to_owned();
        let _store_set: StorageSettings = (&setting).into();
    }

    #[tokio::test]
    async fn test_image_caching() -> Result<(), ()> {
        if std::env::var("GOOGLE_APPLICATION_CREDENTIALS").is_err() {
            print!("Skipping test: No credentials found.");
            return Ok(());
        }
        let src_img = "https://evilonastick.com/test/128px.jpg";

        let test_settings = test_storage_settings();
        let tiles_ttl = 2;
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(
                test_settings.request_timeout,
            ))
            .build()
            .unwrap();
        let (rx, sink) = SpyMetricSink::new();
        let img_store = ImageStore::check_bucket(
            &test_settings,
            tiles_ttl,
            &StatsdClient::builder("contile", sink).build(),
            &client,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(rx.len(), 0);

        let target = src_img.parse::<Uri>().unwrap();
        img_store.store(&target).await.expect("Store failed");
        assert_eq!(rx.len(), 2);

        img_store.store(&target).await.expect("Store failed");
        assert_eq!(rx.len(), 2);

        tokio::time::delay_for(std::time::Duration::from_secs(tiles_ttl.into())).await;
        img_store.store(&target).await.expect("Store failed");
        assert_eq!(rx.len(), 4);
        let spied_metrics: Vec<String> = rx
            .try_iter()
            .map(|x| String::from_utf8(x).unwrap())
            .collect();
        assert_eq!(
            spied_metrics,
            vec![
                "contile.image.fetch:1|c",
                "contile.image.object.check:1|c",
                "contile.image.fetch:1|c",
                "contile.image.object.check:1|c",
            ]
        );
        Ok(())
    }
}
