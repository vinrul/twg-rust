//! Repairs embedded image mime types in existing GLB asset files and syncs
//! their size/sha256 metadata in the asset database.
//!
//! Run from the project root (or pass the asset directory):
//!
//! ```sh
//! cargo run -p twg-server --example repair_assets -- asset-store
//! ```
//!
//! Existing client caches then see a newer asset version and re-download the
//! repaired files.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let directory = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "asset-store".to_owned());
    let database_url = std::env::var("DB_SOURCE")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .map_err(|_| "DB_SOURCE or DATABASE_URL is not set")?;
    let pool = twg_storage::connect(&twg_storage::StorageConfig::new(database_url)).await?;
    let store = twg_storage::AssetStore::new(pool);
    let report = twg_server::assets::repair_asset_store_with_metadata(
        std::path::Path::new(&directory),
        &store,
    )
    .await;
    println!(
        "scanned={} repaired={} metadata_updated={} failed={}",
        report.scanned, report.repaired, report.metadata_updated, report.failed
    );
    Ok(())
}
