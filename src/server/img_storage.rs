use std::sync::Arc;

use actix_web::{http::uri, web::Buf};
use googleapis_raw::storage::v1::{storage::{self, InsertObjectRequest_oneof_data, InsertObjectRequest_oneof_first_message}, storage_grpc, storage_resources};
use grpcio::{ChannelBuilder, ChannelCredentials, EnvBuilder};
use reqwest;
use sha2::{Sha256, Digest};
use hex;

use crate::error::{HandlerResult, HandlerErrorKind};
struct StoreImage {
    client: storage_grpc::StorageClient,
    bucket: storage_resources::Bucket,
    origin: uri::Uri,
    hash: String,
}

impl StoreImage{

    async fn fetch(&self, uri: uri::Uri) -> HandlerResult<Self> {
        let endpoint = "storage.googleapis.com:443"; // <- Verify
        let mut hasher = Sha256::new();
        hasher.update(uri.path().as_bytes());
        let hash = hex::encode(hasher.finalize().as_slice());

        // I have zero faith that any of the following works.

        let bucket_name = "Contile_Test_Bucket";

        // create a new channel
        let env = Arc::new(EnvBuilder::new().build());
        let creds = ChannelCredentials::google_default_credentials().map_err(|e| HandlerErrorKind::Internal(format!("Credential error {:?}", e)))?;
        let chan = ChannelBuilder::new(env.clone())
            .secure_connect(endpoint, creds);
        let client = storage_grpc::StorageClient::new(chan);

        // TODO: Fetch the image content.
        let req = reqwest::get(&uri.to_string()).await.map_err(|e| HandlerErrorKind::Internal(format!("Image fetch error: {:?}", e)))?.bytes().await.map_err(|e| HandlerErrorKind::Internal(format!("Image body error: {:?}", e)))?;
        // TODO: Store the image content into the bucket.
        let mut obj_req = storage::InsertObjectRequest::new();
        let mut bucket = storage_resources::Bucket::new();
        bucket.set_name(bucket_name.to_owned());
        let mut object = storage_resources::Object::default();
        let mut data = storage_resources::ChecksummedData::new();
        data.content = req.to_bytes().bytes().to_vec();
        obj_req.set_checksummed_data(data);
        // TODO: set object metadata
        let bucket = obj_req.insert_object().map_err(|e| HandlerErrorKind::Internal(format!("Bucket error: {:?}", e)))?;
        // TODO: Get the path to the image

        Ok(StoreImage {
            client,
            bucket: storage_resources::Bucket::default(),
            origin: uri,
            hash
        })
    }
}