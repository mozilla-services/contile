use actix_http::http::HeaderValue;
use actix_web::http::uri;
use cloud_storage::Object;
use serde::Deserialize;

use crate::error::{HandlerErrorKind, HandlerResult};
use crate::settings::Settings;

#[derive(Clone, Debug, Deserialize)]
pub struct StorageSettings {
    project_name: String,
    bucket_name: String,
    endpoint: String,
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
pub struct StoreImage {
    // bucket isn't really needed here, since `Object` stores and manages itself.
    // bucket: cloud_storage::Bucket,
    settings: StorageSettings,
}

#[derive(Debug)]
pub struct FetchResult {
    url: uri::Uri,
    hash: String,
    object: Object,
}

impl StoreImage {
    pub async fn new(settings: &Settings) -> HandlerResult<Self> {
        for (key, value) in std::env::vars() {
            dbg!(key, value);
        }
        // TOOD: Validate bucket name?
        // https://cloud.google.com/storage/docs/naming-buckets
        // TODO: Check and create a bucket? Probaly not needed.
        /*
        dbg!("building bucket {:?}", &sset.bucket_name);
        // create errors out if bucket already exists.
        let bucket = match Bucket::create(&cloud_storage::NewBucket {
            name: sset.bucket_name.clone(),
            ..Default::default()
        })
        .await
        {
            Ok(v) => v,
            Err(e) => {
                // most likely the bucket already exists. Need to trap for that?
                // cloud_storage::GoogleError.is_reason(cloud_storage::Reason::Conflict)
                return Err(
                    HandlerErrorKind::Internal(format!("Bucket Creation error: {:?}", e)).into(),
                )
            }
        };
        // TODO: Grant "Storage Object Creator" to whoever can write data to this bucket
        //      (must match `client_id` in google credentials JSON file.)
        // TODO: Grant "Storage Object Viewer" to "allUsers" so it's visible for cacher.
        dbg!("Bucket OK");
        */
        Ok(Self {
            // bucket,
            settings: settings.storage.clone(),
        })
    }

    pub async fn fetch(&self, uri: uri::Uri) -> HandlerResult<FetchResult> {
        // This will absolutely fetch and store the img into the bucket.
        // We don't do any form of check to see if it matches what we got before.
        // (e.g. set the path to be the SHA1 of the bytes or whatever.)
        dbg!("fetching...", &uri);
        /*
        // Should we preserve the name of the image?
        let mut hasher = Sha256::new();
        hasher.update(uri.path().as_bytes());
        let hash = hex::encode(hasher.finalize().as_slice());
        */

        // TODO: Fetch the image content.
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

        let now = chrono::Utc::now();
        let image_path = format!(
            "{}/{:?}{}",
            uri.host().expect("No host!?"),
            now.timestamp(),
            uri.path()
        );

        let public_url = format!(
            "{endpoint}/{project_name}/{bucket_name}/{image_path}",
            endpoint = self.settings.endpoint,
            project_name = self.settings.project_name,
            bucket_name = self.settings.bucket_name,
            image_path = image_path,
        );

        // store data to the googles
        let object = cloud_storage::Object::create(
            &self.settings.bucket_name,
            image.to_vec(),
            &image_path,
            content_type,
        )
        .await
        .map_err(|e| HandlerErrorKind::Internal(format!("Error storing object: {:?}", e)))?;

        Ok(FetchResult {
            hash: object.etag.clone(),
            url: public_url.parse()?,
            object,
        })
    }
}
