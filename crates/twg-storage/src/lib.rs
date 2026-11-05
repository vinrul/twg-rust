use sqlx::postgres::{PgPool, PgPoolOptions};

pub const DEFAULT_MAX_CONNECTIONS: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    pub database_url: String,
    pub max_connections: u32,
}

impl StorageConfig {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            max_connections: DEFAULT_MAX_CONNECTIONS,
        }
    }
}

pub async fn connect(config: &StorageConfig) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.database_url)
        .await
}
