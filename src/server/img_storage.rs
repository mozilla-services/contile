use std::sync::Arc;

use actix_web::http::uri;
use googleapis_raw::storage::v1::{storage_resources, storage_grpc};
use grpcio::{ChannelBuilder, ChannelCredentials, EnvBuilder};
use reqwest;
use sha2::{Sha256, Digest};
use hex;

use crate::error::HandlerResult;
struct StoreImage {
    client: storage_grpc::StorageClient,
    bucket: storage_resources::Bucket,
    origin: uri::Uri,
    hash: String,
}

impl StoreImage{
    fn fetch(&self, uri: uri::Uri) -> HandlerResult<Self> {
        let endpoint = "storage.googleapis.com:443"; // <- Verify
        let mut hasher = Sha256::new();
        hasher.update(uri.path().as_bytes());
        let hash = hex::encode(hasher.finalize().as_slice());

        // create a new channel
        let env = Arc::new(EnvBuilder::new().build());
        let creds = ChannelCredentials::google_default_credentials()?;
        let chan = ChannelBuilder::new(env.clone())
            .secure_connect(endpoint, creds);
        let client = storage_grpc::StorageClient::new(chan);

        // TODO: Fetch the image content.
        // TODO: Store the image content into the bucket.
        // TODO: Get the path to the image

        Ok(StoreImage {
            client,
            bucket: storage_resources::Bucket::default(),
            origin: uri,
            hash
        })
    }
}