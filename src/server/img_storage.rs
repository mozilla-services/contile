//! Fetch and store a given remote image into Google Storage for CDN caching
use actix_http::http::HeaderValue;
use actix_web::http::uri;
use cloud_storage::{
    bucket::{Binding, IamPolicy, IamRole, StandardIamRole},
    Bucket, Object,
};
use serde::Deserialize;

use crate::error::{HandlerError, HandlerErrorKind, HandlerResult};
use crate::settings::Settings;

/// These values generally come from the Google console for Cloud Storage.
#[derive(Clone, Debug, Deserialize)]
pub struct StorageSettings {
    project_name: String,
    bucket_name: String,
    endpoint: String,
}

/// Instantiate from [Settings]
impl From<&Settings> for StorageSettings {
    fn from(settings: &Settings) -> Self {
        if settings.storage.is_empty() {
            return Self::default();
        }
        serde_json::from_str(&settings.storage).expect("Invalud storage settings")
    }
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            project_name: "secondary-project".to_owned(),
            bucket_name: "moz-contile-test-jr".to_owned(),
            endpoint: "https://storage.googleapis.com".to_owned(),
        }
    }
}

/// Image storage container
#[derive(Default)]
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
}

/// Store a given image into Google Storage
// TODO: Reduce all the `Internal` errors to more specific storage based ones
impl StoreImage {
    /// Connect and optionally create a new Google Storage bucket based off [Settings]
    pub async fn create(settings: &Settings) -> HandlerResult<Self> {
        let sset = StorageSettings::from(settings);
        // TODO: Validate bucket name?
        // https://cloud.google.com/storage/docs/naming-buckets
        trace!("Try creating bucket...");
        let bucket = match Bucket::create(&cloud_storage::NewBucket {
            name: sset.bucket_name.clone(),
            ..Default::default()
        })
        .await
        {
            Ok(v) => v,
            Err(cloud_storage::Error::Google(ger)) => {
                if ger.errors_has_reason(&cloud_storage::Reason::Conflict) {
                    trace!("Already exists {:?}", &sset.bucket_name);
                    // try fetching the existing bucket.
                    let _content = Bucket::read(&sset.bucket_name).await.map_err(|e| {
                        HandlerError::internal(&format!("Could not read bucket {:?}", e))
                    })?;
                    return Ok(Self {
                        // bucket: Some(_content),
                        settings: sset,
                    });
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
        // Set the permissions for the newly created bucket.
        trace!("Trying to grant viewing to all");
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
        trace!("Bucket OK");

        Ok(Self {
            // bucket: Some(bucket),
            settings: sset,
        })
    }

    /// Generate an image path for data storage into Google Storage
    fn gen_path(uri: &uri::Uri) -> String {
        format!("{}{}", uri.host().expect("No host!?"), uri.path())
    }

    /// Generate the public URI for the stored image.
    fn gen_public_url(&self, image_path: &str) -> String {
        format!(
            "{endpoint}/{project_name}/{bucket_name}/{image_path}",
            endpoint = self.settings.endpoint,
            project_name = self.settings.project_name,
            bucket_name = self.settings.bucket_name,
            image_path = image_path,
        )
    }

    /// Store an image fetched from the passed `uri` into Google Cloud Storage
    ///
    /// This will absolutely fetch and store the img into the bucket.
    /// We don't do any form of check to see if it matches what we got before.
    /// If you have "Storage Legacy Bucket Writer" previous content is overwritten.
    /// (e.g. set the path to be the SHA1 of the bytes or whatever.)

    pub async fn store(&self, uri: &uri::Uri) -> HandlerResult<StoreResult> {
        trace!("fetching... {:?}", &uri);
        /*
        // Should we preserve the name of the image?
        let mut hasher = Sha256::new();
        hasher.update(uri.path().as_bytes());
        let hash = hex::encode(hasher.finalize().as_slice());
        */

        let res = reqwest::get(&uri.to_string())
            .await
            .map_err(|e| HandlerErrorKind::Internal(format!("Image fetch error: {:?}", e)))?;
        // TODO: Verify that we have an image (content type matches, size within limits, etc.)
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

        let image_path = Self::gen_path(&uri);

        let public_url = self.gen_public_url(&image_path);

        // store data to the googles
        match cloud_storage::Object::create(
            &self.settings.bucket_name,
            image.to_vec(),
            &image_path,
            content_type,
        )
        .await
        {
            Ok(v) => Ok(StoreResult {
                hash: v.etag.clone(),
                url: public_url.parse()?,
                object: v,
            }),
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
