//! Main application server
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use actix_cors::Cors;
use actix_web::{
    dev,
    http::StatusCode,
    middleware::ErrorHandlers,
    web::{self, Data},
    App, HttpServer,
};
use cadence::StatsdClient;

use crate::{
    adm::{spawn_updater, AdmFilter},
    create_app_version,
    error::{HandlerError, HandlerResult},
    metrics::metrics_from_opts,
    server::{img_storage::ImageStore, location::location_config_from_settings},
    settings::Settings,
    sov::{spawn_updater as sov_spawn_updater, SOVManager},
    web::{dockerflow, handlers, middleware},
};

pub mod cache;
pub mod img_storage;
pub mod location;

/// Arbitrary initial cache size based on the expected mean, feel free to
/// adjust
const TILES_CACHE_INITIAL_CAPACITY: usize = 768;

/// This is the global HTTP state object that will be made available to all
/// HTTP API calls.
pub struct ServerState {
    /// Metric reporting
    pub metrics: Arc<StatsdClient>,
    pub reqwest_client: reqwest::Client,
    pub tiles_cache: cache::TilesCache,
    pub settings: Settings,
    pub partner_filter: Arc<RwLock<AdmFilter>>,
    pub sov_manager: Arc<RwLock<SOVManager>>,
    pub img_store: Option<ImageStore>,
    pub excluded_dmas: Option<Vec<u16>>,
    pub start_up: Instant,
}

impl Clone for ServerState {
    fn clone(&self) -> Self {
        Self {
            metrics: self.metrics.clone(),
            reqwest_client: self.reqwest_client.clone(),
            tiles_cache: self.tiles_cache.clone(),
            settings: self.settings.clone(),
            partner_filter: self.partner_filter.clone(),
            sov_manager: self.sov_manager.clone(),
            img_store: self.img_store.clone(),
            excluded_dmas: self.excluded_dmas.clone(),
            start_up: self.start_up,
        }
    }
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("ServerState")
            .field("metrics", &self.metrics)
            .field("adm_endpoint_url", &self.settings.adm_endpoint_url)
            .field("adm_mobile_endpoint_url", &self.settings.adm_endpoint_url)
            .field("reqwest_client", &self.reqwest_client)
            .field("tiles_cache", &self.tiles_cache)
            .finish()
    }
}

/// The main Actix server
pub struct Server;

/// Simplified Actix app builder (used by both the app and unit test)
#[macro_export]
macro_rules! build_app {
    ($state: expr, $location_config: expr) => {
        App::new()
            .app_data(Data::new($state))
            .app_data(Data::new($location_config.clone()))
            // Middleware is applied LIFO
            // These will wrap all outbound responses with matching status codes.
            .wrap(ErrorHandlers::new().handler(StatusCode::NOT_FOUND, HandlerError::render_404))
            // These are our wrappers
            .wrap(middleware::sentry::SentryWrapper::default())
            // Followed by the "official middleware" so they run first.
            // actix is getting increasingly tighter about CORS headers. Our server is
            // not a huge risk but does deliver XHR JSON content.
            // For now, let's be permissive and use NGINX (the wrapping server)
            // for finer grained specification.
            .wrap(Cors::permissive())
            // Next, the API we are implementing
            .service(web::resource("/v1/tiles").route(web::get().to(handlers::get_tiles)))
            // image cache tester...
            //.service(web::resource("/v1/test").route(web::get().to(handlers::get_image)))
            // And finally the behavior necessary to satisfy Dockerflow
            .service(web::scope("").configure(dockerflow::service))
    };
}

impl Server {
    /// initialize a new instance of the server from [Settings]
    pub async fn with_settings(mut settings: Settings) -> Result<dev::Server, HandlerError> {
        let metrics = Arc::new(metrics_from_opts(&settings)?);
        let req = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(settings.connect_timeout))
            .timeout(Duration::from_secs(settings.request_timeout))
            .user_agent(create_app_version("/"))
            .build()?;
        let storage_client = Arc::new(
            cloud_storage::Client::builder()
                .client(req.clone())
                .build()?,
        );
        let mut partner_filter = HandlerResult::<AdmFilter>::from(&mut settings)?;
        // try to update from the bucket if possible.
        if partner_filter.is_cloud() {
            let (advertiser_settings, last_updated) = partner_filter
                .fetch_new_settings(&storage_client)
                .await?
                .expect("Expected AdmAdvertiserSettings for is_cloud AdmFilter");
            partner_filter.update(advertiser_settings, last_updated);
        }
        let refresh_rate = partner_filter.refresh_rate;
        let is_cloud = partner_filter.is_cloud();
        let filter = Arc::new(RwLock::new(partner_filter));
        spawn_updater(
            is_cloud,
            refresh_rate,
            &filter,
            Arc::clone(&storage_client),
            Arc::clone(&metrics),
        )?;
        let mut sov_manager = HandlerResult::<SOVManager>::from(&mut settings)?;
        if let Some(last_response) = sov_manager.fetch(&storage_client).await? {
            sov_manager.update(last_response);
        }

        let sov = Arc::new(RwLock::new(sov_manager));
        sov_spawn_updater(
            refresh_rate,
            &sov,
            Arc::clone(&storage_client),
            Arc::clone(&metrics),
        )?;
        let tiles_cache = cache::TilesCache::new(TILES_CACHE_INITIAL_CAPACITY);
        let img_store = ImageStore::create(&settings, Arc::clone(&metrics), &req).await?;
        let excluded_dmas = if let Some(exclude_dmas) = &settings.exclude_dma {
            serde_json::from_str(exclude_dmas).map_err(|e| {
                HandlerError::internal(&format!("Invalid exclude_dma field: {:?}", e))
            })?
        } else {
            None
        };
        let state = ServerState {
            metrics: Arc::clone(&metrics),
            reqwest_client: req,
            tiles_cache: tiles_cache.clone(),
            settings: settings.clone(),
            partner_filter: filter,
            sov_manager: sov,
            img_store,
            excluded_dmas,
            start_up: Instant::now(),
        };
        let location_config = location_config_from_settings(&settings, Arc::clone(&metrics));

        tiles_cache.spawn_periodic_reporter(Duration::from_secs(60), Arc::clone(&metrics));

        let mut server = HttpServer::new(move || build_app!(state.clone(), location_config));
        if let Some(keep_alive) = settings.actix_keep_alive {
            server = server.keep_alive(Duration::from_secs(keep_alive));
        }
        let server = server
            .bind((settings.host, settings.port))
            .expect("Could not get Server in Server::with_settings")
            .run();
        Ok(server)
    }
}
