//! Fetch and store a given remote image into Google Storage for CDN caching
use std::io::Cursor;

use actix_http::http::HeaderValue;
use actix_web::http::uri;
use actix_web::web::Bytes;
use chrono;
use cloud_storage::{
    bucket::{Binding, IamPolicy, IamRole, RetentionPolicy, StandardIamRole},
    Bucket, Object,
};
use image::io::Reader as ImageReader;
use serde::{Deserialize, Serialize};

use crate::error::{HandlerError, HandlerErrorKind, HandlerResult};
use crate::settings::Settings;

/// These values generally come from the Google console for Cloud Storage.
#[derive(Clone, Debug, Deserialize)]
pub struct StorageSettings {
    /// The GCP Cloud storage project name
    project_name: String,
    /// The Bucket name for this data
    bucket_name: String,
    /// The external CDN host
    #[serde(default = "default_cdn")]
    cdn_host: String,
    /// The bucket TTL is determined by the policy set for the given bucket when it's created.
    #[serde(default = "default_ttl")]
    bucket_ttl: u64,
    /// The max time to live for cached data, ~ 15 days.
    #[serde(default = "default_ttl")]
    cache_ttl: u64,
}

fn default_ttl() -> u64 {
    86400 * 15
}

fn default_cdn() -> String {
    "https://cdn.services.mozilla.org/".to_owned()
}

/// Instantiate from [Settings]
impl From<&Settings> for StorageSettings {
    fn from(settings: &Settings) -> Self {
        if settings.storage.is_empty() {
            return Self::default();
        }
        serde_json::from_str(&settings.storage).expect("Invalid storage settings")
    }
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            //*
            project_name: "".to_owned(),
            bucket_name: "".to_owned(),
            cdn_host: "".to_owned(),
            bucket_ttl: 86400 * 15,
            cache_ttl: 86400 * (15 + 1),
            // */
            /*
            project_name: "secondary-project".to_owned(),
            bucket_name: "moz-contile-test-jr".to_owned(),
            bucket_ttl: 86400 * 15,
            cache_ttl: 86400 * (15 + 1),
            // */
        }
    }
}

/// Image storage container
#[derive(Default, Clone)]
pub struct StoreImage {
    // bucket isn't really needed here, since `Object` stores and manages itself,
    // but it may prove useful in future contexts.
    //
    // bucket: Option<cloud_storage::Bucket>,
    settings: StorageSettings,
}

/// Stored image information, suitable for determining the URL to present to the CDN
#[derive(Debug)]
pub struct StoreResult {
    pub url: uri::Uri,
    pub hash: String,
    pub object: Object,
    pub meta: ImageMeta,
    #[cfg(test)]
    pub exists: bool,
}

#[derive(Clone, Debug, Deserialize, Default, Serialize)]
pub struct ImageMeta {
    pub width: u32,
    pub height: u32,
}

/// Store a given image into Google Storage
// TODO: Reduce all the `Internal` errors to more specific storage based ones
impl StoreImage {
    /// Connect and optionally create a new Google Storage bucket based off [Settings]
    pub async fn create(settings: &Settings) -> HandlerResult<Option<Self>> {
        let sset = StorageSettings::from(settings);

        Self::build_bucket(&sset).await
    }

    pub async fn build_bucket(settings: &StorageSettings) -> HandlerResult<Option<Self>> {
        // TODO: Validate bucket name?
        // https://cloud.google.com/storage/docs/naming-buckets
        // don't try to open an empty bucket
        let empty = ["", "none"];
        if empty.contains(&settings.bucket_name.to_lowercase().as_str())
            || empty.contains(&settings.project_name.to_lowercase().as_str())
        {
            trace!("No bucket set. Not storing...");
            return Ok(None);
        }
        // It's better if the bucket already exists.
        // Creating the bucket requires "Storage Object Creator" account permissions,
        // which can be a bit tricky to configure correctly.
        trace!("Try creating bucket...");
        let bucket = match Bucket::create(&cloud_storage::NewBucket {
            name: settings.bucket_name.clone(),
            ..Default::default()
        })
        .await
        {
            Ok(mut v) => {
                // Set the newly created buckets retention policy
                v.retention_policy = Some(RetentionPolicy {
                    retention_period: settings.bucket_ttl,
                    effective_time: chrono::Utc::now(),
                    is_locked: None,
                });
                v.update()
                    .await
                    .map_err(|e| HandlerError::internal(&e.to_string()))?;
                v
            }
            Err(cloud_storage::Error::Google(ger)) => {
                if ger.errors_has_reason(&cloud_storage::Reason::Conflict) {
                    trace!("Already exists {:?}", &settings.bucket_name);
                    // try fetching the existing bucket.
                    let _content = Bucket::read(&settings.bucket_name).await.map_err(|e| {
                        HandlerError::internal(&format!("Could not read bucket {:?}", e))
                    })?;
                    return Ok(Some(Self {
                        // bucket: Some(_content),
                        settings: settings.clone(),
                    }));
                } else {
                    return Err(HandlerError::internal(&format!(
                        "Bucket create error {:?}",
                        ger
                    )));
                }
            }
            Err(e) => {
                return Err(
                    HandlerErrorKind::Internal(format!("Bucket create error: {:?}", e)).into(),
                )
            }
        };
        trace!("Trying to grant viewing to all");
        // Set the permissions for the newly created bucket.
        // grant allUsers view access
        let all_binding = Binding {
            role: IamRole::Standard(StandardIamRole::ObjectViewer),
            members: vec!["allUsers".to_owned()],
            condition: None,
        };
        let policy = IamPolicy {
            bindings: vec![all_binding],
            ..Default::default()
        };
        match bucket.set_iam_policy(&policy).await {
            Ok(_) => {}
            Err(cloud_storage::Error::Google(ger)) => {
                if ger.errors_has_reason(&cloud_storage::Reason::Forbidden) {
                    trace!("Can't set permission...");
                } else {
                    return Err(HandlerErrorKind::Internal(format!(
                        "Could not add read policy {:?}",
                        ger
                    ))
                    .into());
                }
            }
            Err(e) => {
                return Err(HandlerErrorKind::Internal(format!(
                    "Could not add read policy {:?}",
                    e
                ))
                .into())
            }
        };
        // Yay! Bucket created.
        trace!("Bucket OK");

        Ok(Some(Self {
            // bucket: Some(bucket),
            settings: settings.clone(),
        }))
    }

    pub fn meta(&self, image: &Bytes) -> HandlerResult<ImageMeta> {
        let img = ImageReader::new(Cursor::new(image))
            .decode()
            .map_err(|e| HandlerErrorKind::Internal(format!("Invalid image from ADM: {:?}", e)))?;
        let meta = img.to_rgb16().dimensions();
        Ok(ImageMeta {
            height: meta.1,
            width: meta.0,
        })
    }

    /// Store an image fetched from the passed `uri` into Google Cloud Storage
    ///
    /// This will absolutely fetch and store the img into the bucket.
    /// We don't do any form of check to see if it matches what we got before.
    /// If you have "Storage Legacy Bucket Writer" previous content is overwritten.
    /// (e.g. set the path to be the SHA1 of the bytes or whatever.)

    pub async fn store(&self, uri: &uri::Uri) -> HandlerResult<StoreResult> {
        trace!("fetching... {:?}", &uri);
        let res = reqwest::get(&uri.to_string())
            .await
            .map_err(|e| HandlerErrorKind::Internal(format!("Image fetch error: {:?}", e)))?;
        // TODO: Verify that we have an image (content type matches, size within limits, etc.)
        dbg!(
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
        let image = res
            .bytes()
            .await
            .map_err(|e| HandlerErrorKind::Internal(format!("Image body error: {:?}", e)))?;

        let meta = self.meta(&image)?;
        // TODO: Check image meta sizes

        // image paths tend to be "https://<host>/account/###/###/####.jpg"
        // for now, let's use the various numbers to construct the file name.
        // this will presume that new images will use a different filename, since the last ####.jpg
        // looks an awful lot like <creation_utc>.jpg
        let image_path = &uri
            .path()
            .split('/')
            .filter(|v| !(v.is_empty() || v == &"account")) // remove useless bits.
            .collect::<Vec<&str>>()
            .join("_");

        // check to see if image exists.
        if let Ok(exists) =
            cloud_storage::Object::read(&self.settings.bucket_name, &image_path).await
        {
            trace!("Found existing image in bucket: {:?}", &exists.media_link);
            return Ok(StoreResult {
                hash: exists.etag.clone(),
                url: exists.self_link.clone().parse()?,
                object: exists,
                meta,
                #[cfg(test)]
                exists: true,
            });
        }

        // store data to the googles
        match cloud_storage::Object::create(
            &self.settings.bucket_name,
            image.to_vec(),
            &image_path,
            content_type,
        )
        .await
        {
            Ok(v) => {
                trace!("Stored to {:?}", &v.self_link);
                Ok(StoreResult {
                    hash: v.etag.clone(),
                    url: format!("{:?}/{:?}", &self.settings.cdn_host, &image_path).parse()?,
                    object: v,
                    meta,
                    #[cfg(test)]
                    exists: false,
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

    #[test]
    fn test_config() {
        let test_val = r#"{"project_name": "project", "bucket_name": "bucket"}"#;

        let mut setting = test_settings();
        setting.storage = test_val.to_owned();
        let store_set: StorageSettings = (&setting).into();

        assert!(store_set.project_name == *"project");
        assert!(store_set.bucket_name == *"bucket");
        assert!(store_set.cache_ttl == default_ttl());
    }

    #[tokio::test]
    async fn test_bucket_gen() -> Result<(), ()> {
        // TODO: Add credentials and settings for this test
        if std::env::var("GOOGLE_APPLICATION_CREDENTIALS").is_err() {
            print!("Skipping test: No credentials found.");
            return Ok(());
        }
        // TODO: Set these to be valid bucket data.
        let project_name = "secondary_project";
        let bucket_name = "moz-contile-test-jr";
        // TODO: Give this an appropriate target image, at least one dir down.
        let src_img = "https://evilonastick.com/i/catfact16.png";

        let test_settings = StorageSettings {
            project_name: project_name.to_owned(),
            bucket_name: bucket_name.to_owned(),
            bucket_ttl: 86400 * 15,
            cache_ttl: 86400 * (15 + 1),
            cdn_host: "https://example.com/".to_owned(),
        };
        let bucket = StoreImage::build_bucket(&test_settings)
            .await
            .unwrap()
            .unwrap();
        let target = src_img.parse::<Uri>().unwrap();
        let result = bucket.store(&target).await;
        dbg!(&result);
        assert!(result.is_ok());
        Ok(())
    }
}
