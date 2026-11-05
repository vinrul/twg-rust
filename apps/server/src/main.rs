use std::env;

use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info")),
        )
        .init();

    let address = env::var("HTTP_SERVER_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8010".to_owned());
    let listener = TcpListener::bind(&address).await?;

    tracing::info!(%address, "starting twg server");
    axum::serve(listener, twg_server::router())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
