use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use twg_auth::{AuthService, AuthStore};
use twg_server::config::ServerConfig;
use twg_server::state::AppState;
use twg_storage::{PgAuthStore, StorageConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info")),
        )
        .init();

    let config = ServerConfig::from_env()?;
    tracing::info!(
        address = %config.address,
        cors = ?config.cors_domains,
        max_body_bytes = config.max_body_bytes,
        "loaded server configuration"
    );

    // Older uploads may contain embedded images whose glTF `mimeType` does not
    // match the actual bytes (for example PNG data labelled image/jpeg). Bevy's
    // glTF loader rejects those, so this maintenance command repairs the store
    // and synchronizes the database metadata. Run it once after upgrading, or
    // whenever assets come from an older server.
    let repair_only = std::env::args().any(|argument| argument == "--repair-assets");

    let storage = StorageConfig::new(config.database_url.clone());
    let pool = twg_storage::connect(&storage).await?;
    twg_storage::run_migrations(&pool).await?;

    if repair_only {
        let asset_store = twg_storage::AssetStore::new(pool);
        let report =
            twg_server::assets::repair_asset_store_with_metadata(&config.assets_dir, &asset_store)
                .await;
        tracing::info!(
            scanned = report.scanned,
            repaired = report.repaired,
            metadata_updated = report.metadata_updated,
            failed = report.failed,
            "asset store repair complete"
        );
        return Ok(());
    }

    let store: Arc<dyn AuthStore> = Arc::new(PgAuthStore::new(pool.clone()));
    let auth = AuthService::new(store, config.auth.clone());
    let state = AppState::new(&config, auth, Some(pool));
    let app = twg_server::router(state);

    let listener = TcpListener::bind(&config.address).await?;
    tracing::info!(address = %config.address, "starting twg server");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "failed to listen for ctrl-c");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::warn!(%error, "failed to listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
