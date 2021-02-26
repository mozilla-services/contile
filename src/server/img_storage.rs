use actix_web::http::uri;
use googleapis_raw::storage::v1::storage_resources;
use reqwest;
use sha2::{Sha256, Digest};
use hex;

use crate::error::HandlerResult;
struct StoreImage {
    bucket: storage_resources::Bucket,
    origin: uri::Uri,
    hash: String,
}

impl StoreImage{
    fn fetch(uri: uri::Uri) -> HandlerResult<Self> {
        let mut hasher = Sha256::new();
        hasher.update(uri.path().as_bytes());
        let hash = hex::encode(hasher.finalize().as_slice());

        // TODO: Fetch the image content.
        // TODO: Store the image content into the bucket.
        // TODO: Get the path to the image

        Ok(StoreImage {
            bucket: storage_resources::Bucket::default(),
            origin: uri,
            hash
        })
    }
}