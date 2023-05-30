use actix_web::rt;
use base64::Engine;
use cadence::{CountedExt, StatsdClient};
use chrono::Utc;
use config::ConfigError;
use serde::{Deserialize, Serialize};
use std::{fs::read_to_string, path::Path, sync::Arc, time::Duration};
use tokio::sync::RwLock;

use crate::{
    error::{HandlerError, HandlerErrorKind, HandlerResult},
    settings::Settings,
    web::middleware::sentry as l_sentry,
};

pub struct SOVManager {
    pub refresh_rate: Duration,
    pub last_response: Option<LastResponse>,
    pub source_url: Option<url::Url>,
    pub encoded_sov: Option<String>,
}

impl SOVManager {
    pub async fn fetch(
        &self,
        storage_client: &cloud_storage::Client,
    ) -> HandlerResult<Option<LastResponse>> {
        if let Some(bucket) = &self.source_url {
            let host = bucket
                .host()
                .ok_or_else(|| {
                    HandlerError::internal(&format!("Missing bucket Host {:?}", self.source_url))
                })?
                .to_string();
            let path = bucket.path().trim_start_matches('/');
            let obj = storage_client.object().read(&host, path).await?;
            if let Some(LastResponse { updated, .. }) = self.last_response {
                // if the remote object is not newer than the local object, do nothing
                if obj.updated <= updated {
                    return Ok(None);
                }
            };

            let bytes = storage_client.object().download(&host, path).await?;
            let contents = String::from_utf8(bytes).map_err(|e| {
                HandlerErrorKind::General(format!("Could not read SOV Settings: {:?}", e))
            })?;
            let new_response = serde_json::from_str(&contents).map_err(|e| {
                HandlerErrorKind::General(format!("Could not read SOV Settings: {:?}", e))
            })?;
            return Ok(Some(LastResponse {
                updated: obj.updated,
                response: new_response,
            }));
        }
        Ok(None)
    }

    pub fn update(&mut self, last_response: LastResponse) {
        let json_string = serde_json::to_string(&last_response.response).unwrap();
        self.encoded_sov =
            Some(base64::engine::general_purpose::STANDARD_NO_PAD.encode(json_string.as_bytes()));
        self.last_response = Some(last_response);
    }
}

/// Background updater.
///
pub fn spawn_updater(
    refresh_rate: Duration,
    manager: &Arc<RwLock<SOVManager>>,
    storage_client: Arc<cloud_storage::Client>,
    metrics: Arc<StatsdClient>,
) -> HandlerResult<()> {
    let manager = Arc::clone(manager);
    rt::spawn(async move {
        loop {
            updater(&manager, &storage_client, &metrics).await;
            rt::time::sleep(refresh_rate).await;
        }
    });
    Ok(())
}

async fn updater(
    manager: &Arc<RwLock<SOVManager>>,
    storage_client: &cloud_storage::Client,
    metrics: &Arc<StatsdClient>,
) {
    // Do the check before matching so that the read lock can be released right away.
    let result = manager.read().await.fetch(storage_client).await;
    match result {
        Ok(Some(last_response)) => {
            manager.write().await.update(last_response);
            trace!("SOV updated from cloud storage");
            metrics.incr("sov.update.ok").ok();
        }
        Ok(None) => {
            metrics.incr("sov.update.check.skip").ok();
        }
        Err(e) => {
            trace!("SOV update failed: {:?}", e);
            metrics.incr("sov.update.check.error").ok();
            l_sentry::report(&e, &e.tags);
        }
    }
}

#[derive(Debug)]
pub struct LastResponse {
    pub updated: chrono::DateTime<chrono::Utc>,
    pub response: SOVResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SOVResponse {
    pub name: String,
    pub allocations: Vec<PositionAllocation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionAllocation {
    pub position: i64,
    pub allocation: Vec<Allocation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Allocation {
    pub partner: String,
    pub percentage: i64,
}

impl From<&mut Settings> for HandlerResult<SOVManager> {
    fn from(settings: &mut Settings) -> Self {
        let (source_url, last_response) = if settings.sov_source.starts_with("gs://") {
            (
                Some(
                    settings
                        .sov_source
                        .parse::<url::Url>()
                        .map_err(|e| {
                            ConfigError::Message(format!(
                                "Unable to parse SOV URL '{}': {:?}",
                                &settings.sov_source, e
                            ))
                        })
                        .unwrap(),
                ),
                None,
            )
        } else if Path::new(&settings.sov_source).exists() {
            let response = match read_to_string(&settings.sov_source) {
                Ok(contents) => serde_json::from_str(&contents)
                    .map_err(|e| {
                        ConfigError::Message(format!(
                            "Could not parse SOV settings from file '{}': {:?}",
                            settings.sov_source, e
                        ))
                    })
                    .unwrap(),
                Err(e) => panic!(
                    "Could not read file '{}' with SOV settings: {:?}",
                    &settings.sov_source, e
                ),
            };
            (
                None,
                Some(LastResponse {
                    response,
                    updated: Utc::now(),
                }),
            )
        } else if !settings.sov_source.is_empty() {
            let response = serde_json::from_str::<SOVResponse>(&settings.sov_source)
                .map_err(|e| ConfigError::Message(format!("Could not parse SOV settings: {:?}", e)))
                .unwrap();
            (
                None,
                Some(LastResponse {
                    response,
                    updated: Utc::now(),
                }),
            )
        } else {
            (None, None)
        };

        if last_response.as_ref().is_some() {
            Ok(SOVManager {
                refresh_rate: Duration::from_secs(settings.sov_refresh_rate_secs),
                source_url,
                encoded_sov: Some(base64::engine::general_purpose::STANDARD_NO_PAD.encode(
                    serde_json::to_string(&last_response.as_ref().unwrap().response).unwrap(),
                )),
                last_response,
            })
        } else {
            Ok(SOVManager {
                refresh_rate: Duration::from_secs(settings.sov_refresh_rate_secs),
                source_url,
                encoded_sov: None,
                last_response,
            })
        }
    }
}
