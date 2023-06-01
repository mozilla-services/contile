use actix_web::rt;
use base64::Engine;
use cadence::{CountedExt, StatsdClient};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{fs::read_to_string, path::Path, sync::Arc, time::Duration};
use tokio::sync::RwLock;

use crate::{
    error::{HandlerError, HandlerErrorKind, HandlerResult},
    settings::Settings,
    web::middleware::sentry as l_sentry,
};

#[derive(Debug)]
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
        // Try with the remote source first.
        if settings.sov_source.starts_with("gs://") {
            let Ok(source_url) = settings.sov_source.parse::<url::Url>() else {
                return Err(
                    HandlerErrorKind::Internal(
                        format!("Unable to parse SOV URL '{}'", &settings.sov_source)
                    ).into()
                );
            };

            return Ok(SOVManager {
                source_url: Some(source_url),
                refresh_rate: Duration::from_secs(settings.sov_refresh_rate_secs),
                encoded_sov: None,
                last_response: None,
            });
        }

        // Then check if it's a local settings file or an inline settings string.
        let sov = if Path::new(&settings.sov_source).exists() {
            let Ok(sov) =
                read_to_string(&settings.sov_source)
                    .map_err(|_| Err::<String, &str>("Unable to read SOV settings file"))
                    .and_then(|content| {
                        serde_json::from_str::<SOVResponse>(&content)
                            .map_err(|_| Err("Unable to load SOV settings from JSON"))
                    }) else {
                        return Err(
                            HandlerErrorKind::Internal(
                                format!("Unable to parse SOV settings from file '{}'", settings.sov_source)
                            ).into()
                        );
                    };
            sov
        } else {
            // Presume it's an inline SOV settings string.
            let Ok(sov) = serde_json::from_str::<SOVResponse>(&settings.sov_source) else {
                return Err(
                    HandlerErrorKind::Internal(
                        format!("Could not parse SOV settings inline: {:?}", &settings.sov_source)
                    ).into()
                );
            };
            sov
        };

        Ok(SOVManager {
            source_url: None,
            refresh_rate: Duration::from_secs(settings.sov_refresh_rate_secs),
            encoded_sov: Some(
                // Unwrapping is safe here since the parsing above ensures it's a valid `SOVResponse`.
                base64::engine::general_purpose::STANDARD_NO_PAD
                    .encode(serde_json::to_string(&sov).unwrap()),
            ),
            last_response: Some(LastResponse {
                response: sov,
                updated: Utc::now(),
            }),
        })
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::*;
    use crate::web::test::get_test_settings;

    const MOCK_SOV: &str = "eyJuYW1lIjoiU09WLTIwMjMwNTE4MjE1MzE2IiwiYWxsb2NhdGl\
                            vbnMiOlt7InBvc2l0aW9uIjoxLCJhbGxvY2F0aW9uIjpbeyJwYX\
                            J0bmVyIjoiYW1wIiwicGVyY2VudGFnZSI6MTAwfV19LHsicG9za\
                            XRpb24iOjIsImFsbG9jYXRpb24iOlt7InBhcnRuZXIiOiJhbXAi\
                            LCJwZXJjZW50YWdlIjo4OH0seyJwYXJ0bmVyIjoibW96LXNhbGV\
                            zIiwicGVyY2VudGFnZSI6MTJ9XX1dfQ";


    #[test]
    #[should_panic(expected = "Unable to parse SOV URL 'gs://bad^^url'")]
    fn test_bad_gcs_url() {
        let mut settings = Settings {
            sov_source: "gs://bad^^url".to_owned(),

            ..get_test_settings()
        };
        let _bad_url = HandlerResult::<SOVManager>::from(&mut settings).unwrap();
    }
    
    #[test]
    #[should_panic(
        expected = "Unable to parse SOV settings from file './test-engineering/contract-tests/volumes/contile/adm_settings.json'"
    )]
    fn test_bad_path() {
        let mut settings = Settings {
            sov_source: "./test-engineering/contract-tests/volumes/contile/adm_settings.json"
                .to_owned(),

            ..get_test_settings()
        };
        HandlerResult::<SOVManager>::from(&mut settings).unwrap();
    }

    #[test]
    fn test_valid_path() {
        let mut settings = Settings {
            sov_source: "./test-engineering/contract-tests/volumes/contile/sov_settings.json"
                .to_owned(),
            ..get_test_settings()
        };

        let sov_manager = HandlerResult::<SOVManager>::from(&mut settings);
        assert_eq!(sov_manager.unwrap().encoded_sov.as_deref(), Some(MOCK_SOV));
    }

    #[test]
    #[should_panic(expected = "Could not parse SOV settings inline: \\\"{}\\\"")]
    fn test_bad_json_setting_string() {
        let mut settings = Settings {
            sov_source: "{}".to_owned(),

            ..get_test_settings()
        };
        HandlerResult::<SOVManager>::from(&mut settings).unwrap();
    }

    #[test]
    fn test_valid_json_string() {
        let mut settings = Settings {
            sov_source: json!({
                "name": "SOV-20230518215316",
                "allocations": [
                    {
                        "position": 1,
                        "allocation": [
                            {
                                "partner": "amp",
                                "percentage": 100
                            }
                        ]
                    },
                    {
                        "position": 2,
                        "allocation": [
                            {
                                "partner": "amp",
                                "percentage": 88
                            },
                            {
                                "partner": "moz-sales",
                                "percentage": 12
                            }
                        ]
                    }
                ]
            })
            .to_string(),
            ..get_test_settings()
        };

        let sov_manager = HandlerResult::<SOVManager>::from(&mut settings);
        assert_eq!(sov_manager.unwrap().encoded_sov.as_deref(), Some(MOCK_SOV));
    }
}
