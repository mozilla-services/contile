//! Fetch and store a given remote image into Google Storage for CDN caching
use std::{env, io::Cursor, time::Duration};

use actix_http::http::HeaderValue;
use actix_web::http::uri;
use actix_web::web::Bytes;
use cloud_storage::{Bucket, Object};
use image::{io::Reader as ImageReader, ImageFormat};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{HandlerError, HandlerErrorKind, HandlerResult};
use crate::settings::Settings;
use crate::tags::Tags;

/// These values generally come from the Google console for Cloud Storage.
#[derive(Clone, Debug, Deserialize, Serialize)]
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
            max_size: 100_000,
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
    bucket_ttl: u64,
    /// The max time to live for cached data, ~ 15 days.
    cache_ttl: u64,
    /// Max dimensions for an image
    metrics: ImageMetricSettings,
    /// Max request time (in seconds)
    request_timeout: u64,
    /// Whether to attempt to create the cloud storage bucket
    create_bucket: bool,
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
            bucket_ttl: 86400 * 15,
            cache_ttl: 86400 * 15,
            metrics: ImageMetricSettings::default(),
            request_timeout: 3,
            create_bucket: false,
        }
    }
}

/// Image storage container
#[derive(Clone)]
pub struct StoreImage {
    // bucket isn't really needed here, since `Object` stores and manages itself,
    // but it may prove useful in future contexts.
    //
    // bucket: Option<cloud_storage::Bucket>,
    settings: StorageSettings,
    req: reqwest::Client,
}

impl Default for StoreImage {
    fn default() -> Self {
        Self {
            settings: StorageSettings::default(),
            req: reqwest::Client::new(),
        }
    }
}

/// Stored image information, suitable for determining the URL to present to the CDN
#[derive(Debug)]
pub struct StoredImage {
    pub url: uri::Uri,
    pub object: Object,
    pub image_metrics: ImageMetrics,
}

#[derive(Clone, Debug, Deserialize, Default, Serialize, PartialEq)]
pub struct ImageMetrics {
    pub width: u32,
    pub height: u32,
    pub size: usize,
}

/// Store a given image into Google Storage
impl StoreImage {
    /// Connect and optionally create a new Google Storage bucket based off [Settings]
    pub async fn create(
        settings: &Settings,
        client: &reqwest::Client,
    ) -> HandlerResult<Option<Self>> {
        let sset = StorageSettings::from(settings);
        Self::check_bucket(&sset, client).await
    }

    pub async fn check_bucket(
        settings: &StorageSettings,
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

        // verify that the bucket can be read
        let _content = Bucket::read(&settings.bucket_name)
            .await
            .map_err(|e| HandlerError::internal(&format!("Could not read bucket {:?}", e)))?;

        trace!("Bucket OK");

        Ok(Some(Self {
            // bucket: Some(bucket),
            settings: settings.clone(),
            req: client.clone(),
        }))
    }

    pub fn meta(&self, image: &Bytes, fmt: ImageFormat) -> HandlerResult<ImageMetrics> {
        let mut reader = ImageReader::new(Cursor::new(image));
        reader.set_format(fmt);
        let img = reader
            .decode()
            .map_err(|_| HandlerErrorKind::BadImage("Image unreadable"))?;
        let rgb_img = img.to_rgb16();
        Ok(ImageMetrics {
            height: rgb_img.height(),
            width: rgb_img.width(),
            size: rgb_img.len(),
        })
    }

    /// Store an image fetched from the passed `uri` into Google Cloud Storage
    ///
    /// This will absolutely fetch and store the img into the bucket.
    /// We don't do any form of check to see if it matches what we got before.
    /// If you have "Storage Legacy Bucket Writer" previous content is overwritten.
    /// (e.g. set the path to be the SHA1 of the bytes or whatever.)

    pub async fn store(&self, uri: &uri::Uri) -> HandlerResult<StoredImage> {
        let (image, content_type) = self.fetch(uri).await?;
        let metrics = self.validate(uri, &image, &content_type).await?;
        self.upload(image, &content_type, metrics).await
    }

    /// Generate a unique hash based on the content of the image
    pub fn as_hash(&self, source: &Bytes) -> String {
        base64::encode_config(blake3::hash(source).as_bytes(), base64::URL_SAFE_NO_PAD)
    }

    /// Fetch the bytes for an image based on a URI
    pub(crate) async fn fetch(&self, uri: &uri::Uri) -> HandlerResult<(Bytes, String)> {
        trace!("fetching... {:?}", &uri);
        let res = self
            .req
            .get(&uri.to_string())
            .timeout(Duration::from_secs(self.settings.request_timeout))
            .send()
            .await
            .map_err(|e| HandlerErrorKind::Internal(format!("Image fetch error: {:?}", e)))?
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
        Ok((
            res.bytes()
                .await
                .map_err(|e| HandlerErrorKind::Internal(format!("Image body error: {:?}", e)))?,
            content_type.to_owned(),
        ))
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
                    let mut err: HandlerError =
                        HandlerErrorKind::BadImage("Invalid image format").into();
                    err.tags = tags;
                    return Err(err);
                }
            };
            self.meta(image, fmt)?
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
        if let Ok(exists) =
            cloud_storage::Object::read(&self.settings.bucket_name, &image_path).await
        {
            trace!("Found existing image in bucket: {:?}", &exists.media_link);
            return Ok(StoredImage {
                url: format!("{}/{}", &self.settings.cdn_host, &image_path).parse()?,
                object: exists,
                image_metrics,
            });
        }

        // store new data to the googles
        match cloud_storage::Object::create(
            &self.settings.bucket_name,
            image.to_vec(),
            &image_path,
            content_type,
        )
        .await
        {
            Ok(mut object) => {
                object.content_disposition = Some("inline".to_owned());
                object.cache_control = Some(format!("public, max-age={}", self.settings.cache_ttl));
                object.update().await.map_err(|_| {
                    error!("Could not set disposition for {:?}", object.self_link);
                    HandlerErrorKind::BadImage("Could not set content disposition")
                })?;
                let url = format!("{}/{}", &self.settings.cdn_host, &image_path);
                trace!("Stored to {:?}: {:?}", &object.self_link, &url);
                Ok(StoredImage {
                    url: url.parse()?,
                    object,
                    image_metrics,
                })
            }
            Err(cloud_storage::Error::Google(ger)) => {
                Err(HandlerErrorKind::Internal(format!("Could not create object {:?}", ger)).into())
            }
            Err(e) => {
                // If the IamPolicy does not have "Storage Legacy Bucket Writer", you get 403
                Err(HandlerErrorKind::Internal(format!("Error creating object {:?}", e)).into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::settings::test_settings;
    use actix_http::http::Uri;
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
            bucket_ttl: 86400 * 15,
            cache_ttl: 86400 * (15 + 1),
            cdn_host: cdn,
            ..Default::default()
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
        let client = reqwest::Client::new();
        let bucket = StoreImage::check_bucket(&test_settings, &client)
            .await
            .unwrap()
            .unwrap();
        let target = src_img.parse::<Uri>().unwrap();
        bucket.store(&target).await.expect("Store failed");
        Ok(())
    }

    #[tokio::test]
    async fn test_image_validate() -> Result<(), ()> {
        set_env();
        let test_valid_image = test_image_buffer(96, 96);
        let test_uri: Uri = "https://example.com/test.jpg".parse().unwrap();
        let test_settings = test_storage_settings();
        let bucket = StoreImage {
            settings: test_settings,
            req: reqwest::Client::new(),
        };

        let result = bucket
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
        let bucket = StoreImage {
            settings: test_storage_settings(),
            req: reqwest::Client::new(),
        };

        assert!(bucket
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
}
