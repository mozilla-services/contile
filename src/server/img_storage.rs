use actix_http::http::HeaderValue;
use actix_web::http::uri;
use cloud_storage::{
    bucket::{Binding, IamPolicy, IamRole, StandardIamRole},
    Bucket, Object,
};
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

#[derive(Default)]
pub struct StoreImage {
    // bucket isn't really needed here, since `Object` stores and manages itself,
    // but it may prove useful in future contexts.
    //
    // bucket: Option<cloud_storage::Bucket>,
    settings: StorageSettings,
}

#[derive(Debug)]
pub struct StoreResult {
    pub url: uri::Uri,
    pub hash: String,
    pub object: Object,
}

// TODO: Reduce all the `Internal` errors to more specific storage based ones

impl StoreImage {
    pub async fn create(settings: &Settings) -> HandlerResult<Self> {
        let sset = settings.storage.clone();
        // TOOD: Validate bucket name?
        // https://cloud.google.com/storage/docs/naming-buckets
        dbg!("Try creating bucket...");
        let bucket = match Bucket::create(&cloud_storage::NewBucket {
            name: sset.bucket_name.clone(),
            ..Default::default()
        })
        .await
        {
            Ok(v) => v,
            Err(cloud_storage::Error::Google(ger)) => {
                if ger.errors_has_reason(&cloud_storage::Reason::Conflict) {
                    dbg!("Already exists", &sset.bucket_name);
                    // try fetching the existing bucket.
                    match Bucket::read(&sset.bucket_name).await {
                        Ok(_v) => {
                            return Ok(Self {
                                // bucket: Some(v),
                                settings: sset,
                            })
                        }
                        Err(e) => {
                            return Err(HandlerErrorKind::Internal(format!(
                                "Could not read bucket {:?}",
                                e
                            ))
                            .into())
                        }
                    }
                } else {
                    return Err(HandlerErrorKind::Internal(format!(
                        "Bucket create error {:?}",
                        ger
                    ))
                    .into());
                }
            }
            Err(e) => {
                return Err(
                    HandlerErrorKind::Internal(format!("Bucket create error: {:?}", e)).into(),
                )
            }
        };
        // Set the permissions for the newly created bucket.
        dbg!("Trying to grant viewing to all");
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
                    dbg!("Can't set permission...");
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
        dbg!("Bucket OK");

        Ok(Self {
            // bucket: Some(bucket),
            settings: sset,
        })
    }

    fn gen_path(uri: &uri::Uri) -> String {
        format!("{}{}", uri.host().expect("No host!?"), uri.path())
    }

    fn gen_public_url(&self, image_path: &str) -> String {
        format!(
            "{endpoint}/{project_name}/{bucket_name}/{image_path}",
            endpoint = self.settings.endpoint,
            project_name = self.settings.project_name,
            bucket_name = self.settings.bucket_name,
            image_path = image_path,
        )
    }

    pub async fn store(&self, uri: &uri::Uri) -> HandlerResult<StoreResult> {
        // This will absolutely fetch and store the img into the bucket.
        // We don't do any form of check to see if it matches what we got before.
        // If you have "Storage Legacy Bucket Writer" previous content is overwritten.
        // (e.g. set the path to be the SHA1 of the bytes or whatever.)
        dbg!("fetching...", &uri);
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

    pub async fn fetch(&self, uri: &uri::Uri) -> HandlerResult<Option<StoreResult>> {
        let image_path = Self::gen_path(&uri);
        match Object::read(&self.settings.bucket_name, &image_path).await {
            Ok(v) => Ok(Some(StoreResult {
                hash: v.etag.clone(),
                url: self.gen_public_url(&image_path).parse()?,
                object: v,
            })),
            Err(cloud_storage::Error::Google(ger)) => {
                if ger.errors_has_reason(&cloud_storage::Reason::Forbidden) {
                    dbg!("Can't set permission...");
                    Ok(None)
                } else {
                    Err(
                        HandlerErrorKind::Internal(format!("Could not add read policy {:?}", ger))
                            .into(),
                    )
                }
            }
            Err(e) => {
                Err(HandlerErrorKind::Internal(format!("Error retrieving object {:?}", e)).into())
            }
        }
    }
}
