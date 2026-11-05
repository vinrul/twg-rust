use std::path::PathBuf;
use std::sync::Arc;

use sqlx::postgres::PgPool;
use twg_auth::AuthService;
use twg_storage::ReadStore;

use crate::config::{RateLimitConfig, ServerConfig};
use crate::rate_limit::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub auth: Arc<AuthService>,
    pub pool: Option<PgPool>,
    pub read_store: Option<Arc<ReadStore>>,
    pub rate_limiter: Arc<RateLimiter>,
    pub uploads_dir: PathBuf,
    pub cors_domains: Vec<String>,
    pub max_body_bytes: usize,
}

impl AppState {
    pub fn new(config: &ServerConfig, auth: AuthService, pool: Option<PgPool>) -> Self {
        let read_store = pool
            .as_ref()
            .map(|pool| Arc::new(ReadStore::new(pool.clone())));
        Self {
            auth: Arc::new(auth),
            pool,
            read_store,
            rate_limiter: Arc::new(RateLimiter::new(&config.rate_limit)),
            uploads_dir: config.uploads_dir.clone(),
            cors_domains: config.cors_domains.clone(),
            max_body_bytes: config.max_body_bytes,
        }
    }

    pub fn for_test(auth: AuthService, rate_limit: RateLimitConfig) -> Self {
        Self {
            auth: Arc::new(auth),
            pool: None,
            read_store: None,
            rate_limiter: Arc::new(RateLimiter::new(&rate_limit)),
            uploads_dir: PathBuf::from("uploads"),
            cors_domains: vec!["http://localhost:5173".to_owned()],
            max_body_bytes: 1024 * 1024,
        }
    }
}
