use sqlx::postgres::{PgPool, PgPoolOptions};
use thiserror::Error;

mod assets;
mod read;
mod store;
mod write;

pub use assets::{AssetStore, NewAsset, NewAssetSource, NewAssetVariant, asset_file_url};
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
        apply_compat_columns(pool).await?;
        return Ok(());
    }
    sqlx::migrate!("./migrations").run(pool).await?;
    apply_compat_columns(pool).await?;
    Ok(())
}

/// Additive columns that must also reach legacy databases whose SQLx
/// migrations are intentionally skipped. Each statement is idempotent.
async fn apply_compat_columns(pool: &PgPool) -> Result<(), sqlx::Error> {
    let text_markers: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.text_markers')::text")
            .fetch_one(pool)
            .await?;
    if text_markers.is_some() {
        sqlx::query(
            "ALTER TABLE text_markers
             ADD COLUMN IF NOT EXISTS altitude DOUBLE PRECISION NOT NULL DEFAULT 0",
        )
        .execute(pool)
        .await?;
    }
    let particles: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.particles')::text")
            .fetch_one(pool)
            .await?;
    if particles.is_some() {
        sqlx::query(
            "ALTER TABLE particles
             ADD COLUMN IF NOT EXISTS altitude DOUBLE PRECISION NOT NULL DEFAULT 0",
        )
        .execute(pool)
        .await?;
    }
    let skenarios: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.skenarios')::text")
            .fetch_one(pool)
            .await?;
    if skenarios.is_some() {
        sqlx::query(
            "ALTER TABLE skenarios
             ADD COLUMN IF NOT EXISTS sky_mode VARCHAR(20) DEFAULT 'black'",
        )
        .execute(pool)
        .await?;
    }
    let assets: Option<String> = sqlx::query_scalar("SELECT to_regclass('public.assets')::text")
        .fetch_one(pool)
        .await?;
    if assets.is_some() {
        let asset_groups_existed: Option<String> =
            sqlx::query_scalar("SELECT to_regclass('public.asset_groups')::text")
                .fetch_one(pool)
                .await?;
        if asset_groups_existed.is_none() {
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS asset_groups (
                     id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                     name VARCHAR(120) NOT NULL UNIQUE,
                     created_at TIMESTAMP DEFAULT NOW(),
                     updated_at TIMESTAMP DEFAULT NOW()
                 )",
            )
            .execute(pool)
            .await?;
            sqlx::query(
                "INSERT INTO asset_groups (name) VALUES
                     ('Personel'), ('Bangunan'), ('Kendaraan Bermotor'),
                     ('Senjata'), ('Perlengkapan'), ('Lainnya')
                 ON CONFLICT (name) DO NOTHING",
            )
            .execute(pool)
            .await?;
        }
        sqlx::query(
            "ALTER TABLE assets
             ADD COLUMN IF NOT EXISTS group_id UUID REFERENCES asset_groups(id) ON DELETE SET NULL",
        )
        .execute(pool)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_assets_group_id ON assets (group_id)")
            .execute(pool)
            .await?;
        sqlx::query(
            "ALTER TABLE assets
             ADD COLUMN IF NOT EXISTS move_mode TEXT NOT NULL DEFAULT 'none'",
        )
        .execute(pool)
        .await?;
        sqlx::query(
            "ALTER TABLE assets
             ADD COLUMN IF NOT EXISTS origin TEXT NOT NULL DEFAULT 'admin'",
        )
        .execute(pool)
        .await?;
        sqlx::query(
            "ALTER TABLE assets
             ADD COLUMN IF NOT EXISTS operasi_id UUID REFERENCES operasis(id) ON DELETE CASCADE",
        )
        .execute(pool)
        .await?;
        sqlx::query(
            "ALTER TABLE assets
             ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES users(id) ON DELETE SET NULL",
        )
        .execute(pool)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_assets_operasi_id ON assets (operasi_id)")
            .execute(pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_assets_origin ON assets (origin)")
            .execute(pool)
            .await?;
    }
    let markers: Option<String> = sqlx::query_scalar("SELECT to_regclass('public.markers')::text")
        .fetch_one(pool)
        .await?;
    if markers.is_some() {
        sqlx::query(
            "ALTER TABLE markers
             ADD COLUMN IF NOT EXISTS obstacle BOOLEAN NOT NULL DEFAULT TRUE",
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn ping(pool: &PgPool) -> Result<(), StorageError> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}
