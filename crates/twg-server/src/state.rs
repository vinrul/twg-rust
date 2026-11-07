use std::path::PathBuf;
use std::sync::Arc;

use sqlx::postgres::PgPool;
use twg_auth::AuthService;
use twg_storage::{AssetStore, ReadStore, WriteStore};

use crate::assets::{AssetFiles, ConvertConfig};
use crate::config::{RateLimitConfig, ServerConfig};
use crate::rate_limit::RateLimiter;
use crate::realtime::Realtime;
use crate::uploads::{LocalUploadStorage, UploadStorage};

#[derive(Clone)]
pub struct AppState {
    pub auth: Arc<AuthService>,
    pub pool: Option<PgPool>,
    pub read_store: Option<Arc<ReadStore>>,
    pub write_store: Option<Arc<WriteStore>>,
    pub asset_store: Option<Arc<AssetStore>>,
    pub realtime: Arc<Realtime>,
    pub rate_limiter: Arc<RateLimiter>,
    pub uploads_dir: PathBuf,
    pub uploads: Arc<dyn UploadStorage>,
    pub assets_dir: PathBuf,
    pub asset_files: Arc<AssetFiles>,
    pub convert: Arc<ConvertConfig>,
    pub cors_domains: Vec<String>,
    pub max_body_bytes: usize,
}

impl AppState {
    pub fn new(config: &ServerConfig, auth: AuthService, pool: Option<PgPool>) -> Self {
        let read_store = pool
            .as_ref()
            .map(|pool| Arc::new(ReadStore::new(pool.clone())));
        let write_store = pool
            .as_ref()
            .map(|pool| Arc::new(WriteStore::new(pool.clone())));
        let asset_store = pool
            .as_ref()
            .map(|pool| Arc::new(AssetStore::new(pool.clone())));
        Self {
            auth: Arc::new(auth),
            pool,
            read_store,
            write_store,
            asset_store,
            realtime: Arc::new(Realtime::new(
                config.ws_ticket_ttl,
                config.ws_history_limit,
                config.centrifugo.clone(),
            )),
            rate_limiter: Arc::new(RateLimiter::new(&config.rate_limit)),
            uploads_dir: config.uploads_dir.clone(),
            uploads: Arc::new(LocalUploadStorage::new(config.uploads_dir.clone())),
            assets_dir: config.assets_dir.clone(),
            asset_files: Arc::new(AssetFiles::new(config.assets_dir.clone())),
            convert: Arc::new(config.convert.clone()),
            cors_domains: config.cors_domains.clone(),
            max_body_bytes: config.max_body_bytes,
        }
    }

    pub fn for_test(auth: AuthService, rate_limit: RateLimitConfig) -> Self {
        let assets_dir = std::env::temp_dir().join("twg-test-assets");
        Self {
            auth: Arc::new(auth),
            pool: None,
            read_store: None,
            write_store: None,
            asset_store: None,
            realtime: Arc::new(Realtime::new(std::time::Duration::from_secs(30), 64, None)),
            rate_limiter: Arc::new(RateLimiter::new(&rate_limit)),
            uploads_dir: PathBuf::from("uploads"),
            uploads: Arc::new(LocalUploadStorage::new(
                std::env::temp_dir().join("twg-test-uploads"),
            )),
            assets_dir: assets_dir.clone(),
            asset_files: Arc::new(AssetFiles::new(assets_dir)),
            convert: Arc::new(ConvertConfig::default()),
            cors_domains: vec!["http://localhost:5173".to_owned()],
            max_body_bytes: 1024 * 1024,
        }
    }
}
