use sqlx::postgres::{PgPool, PgPoolOptions};
use thiserror::Error;

mod read;
mod store;
mod write;

pub use read::{ReadStore, SatkerScope, UserListScope};
pub use store::PgAuthStore;
pub use write::{
    AnimOrder, NewAnimation, NewScenario, ScenarioUpdate, WriteError, WriteStore, go_json_string,
};

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

    pub fn with_max_connections(mut self, max_connections: u32) -> Self {
        self.max_connections = max_connections.max(1);
        self
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

pub async fn connect(config: &StorageConfig) -> Result<PgPool, StorageError> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.database_url)
        .await?;
    Ok(pool)
}

/// Runs the read-only copy of the legacy migrations for a fresh database.
///
/// The baseline fixture database already contains the legacy schema and has no
/// `_sqlx_migrations` table. Rewriting that schema in place would violate the
/// Phase 4 compatibility rule, so migrations are skipped when `users` exists
/// without SQLx history. New deployments get the full schema.
pub async fn run_migrations(pool: &PgPool) -> Result<(), StorageError> {
    let legacy_schema: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.users')::text")
            .fetch_one(pool)
            .await?;
    let migration_history: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations')::text")
            .fetch_one(pool)
            .await?;
    let core_applied: bool = if migration_history.is_some() {
        sqlx::query_scalar(
            "SELECT EXISTS(
                 SELECT 1 FROM _sqlx_migrations WHERE version = 1 AND success
             )",
        )
        .fetch_one(pool)
        .await?
    } else {
        false
    };
    if legacy_schema.is_some() && !core_applied {
        tracing::warn!(
            "existing legacy schema detected without applied SQLx migrations; skipping migrations"
        );
        return Ok(());
    }
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

pub async fn ping(pool: &PgPool) -> Result<(), StorageError> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}
