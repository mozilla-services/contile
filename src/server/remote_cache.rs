use redis::Commands;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, json};

use crate::error::{HandlerError, HandlerResult};
use crate::settings::Settings;

#[derive(Clone, Debug)]
pub struct RemoteImageCache {
    client: redis::Client,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum CacheState {
    Pending,
    Available,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CacheValue {
    pub state: CacheState,
    pub data: Option<String>,
}

impl RemoteImageCache {
    pub fn new(settings: &Settings) -> HandlerResult<Self> {
        let client = redis::Client::open(settings.redis_server.clone())
            .map_err(|e| HandlerError::internal(&e.to_string()))?;
        Ok(Self { client })
    }

    pub fn put(self, key: &str, value: CacheValue) -> HandlerResult<()> {
        let mut conn = self
            .client
            .get_connection()
            .map_err(|e| HandlerError::internal(&e.to_string()))?;
        conn.set(key, json!(value).to_string())
            .map_err(|e| HandlerError::internal(&e.to_string()))?;
        Ok(())
    }

    pub fn get(self, key: &str) -> HandlerResult<Option<CacheValue>> {
        let mut conn = self
            .client
            .get_connection()
            .map_err(|e| HandlerError::internal(&e.to_string()))?;
        let result: String = match conn.get(key) {
            Ok(v) => v,
            Err(e) => {
                dbg!(e);
                "".to_owned()
            }
        };
        if result.is_empty() {
            return Ok(None);
        }
        Ok(Some(from_str::<CacheValue>(&result).map_err(|e| {
            HandlerError::internal(&format!(
                "Could not deserialize shared cache entry: {} {:?}",
                key, e
            ))
        })?))
    }

    pub fn del(self, key: &str) -> HandlerResult<()> {
        let mut conn = self
            .client
            .get_connection()
            .map_err(|e| HandlerError::internal(&e.to_string()))?;
        conn.del(key)
            .map_err(|e| HandlerError::internal(&e.to_string()))
    }
}
