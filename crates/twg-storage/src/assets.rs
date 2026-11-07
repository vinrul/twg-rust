use chrono::{DateTime, FixedOffset, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use std::str::FromStr;
use twg_contracts::Timestamp;
use twg_contracts::assets::{
    Asset, AssetCategory, AssetFile, AssetModelKind, AssetSource, AssetSourceSlot, AssetVariant,
    AssetVariantKind,
};
use uuid::Uuid;

/// Uploaded source file metadata recorded for a new asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAssetSource {
    pub slot: AssetSourceSlot,
    pub filename: String,
    pub mime: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub has_draco: bool,
    pub lod: Option<u16>,
}

/// File metadata recorded for a variant row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAssetVariant {
    pub kind: AssetVariantKind,
    pub filename: String,
    pub mime: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub reused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAsset {
    pub id: Uuid,
    pub name: String,
    pub category: AssetCategory,
    pub model_kind: Option<AssetModelKind>,
    pub sources: Vec<NewAssetSource>,
}

/// Persistence for the asset library.
#[derive(Clone)]
pub struct AssetStore {
    pool: PgPool,
}

impl AssetStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        category: Option<AssetCategory>,
        search: &str,
    ) -> Result<Vec<Asset>, sqlx::Error> {
        let rows = sqlx::query_as::<_, AssetRow>(
            "SELECT id, name, category, model_kind, version, created_at, updated_at
             FROM assets
             WHERE ($1::text IS NULL OR category = $1)
             ORDER BY created_at ASC",
        )
        .bind(category.map(|value| value.to_string()))
        .fetch_all(&self.pool)
        .await?;
        let needle = search.trim().to_lowercase();
        let rows = rows
            .into_iter()
            .filter(|row| needle.is_empty() || row.matches(&needle))
            .collect::<Vec<_>>();
        self.attach_sources(rows).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Asset>, sqlx::Error> {
        let row = sqlx::query_as::<_, AssetRow>(
            "SELECT id, name, category, model_kind, version, created_at, updated_at
             FROM assets
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(self.attach_sources(vec![row]).await?.pop())
    }

    pub async fn create(&self, asset: NewAsset) -> Result<Asset, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO assets (id, name, category, model_kind)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(asset.id)
        .bind(&asset.name)
        .bind(asset.category.to_string())
        .bind(asset.model_kind.map(|kind| kind.to_string()))
        .execute(&mut *transaction)
        .await?;
        for source in &asset.sources {
            sqlx::query(
                "INSERT INTO asset_sources (
                     asset_id, slot, filename, mime, size_bytes, sha256, has_draco, lod
                 )
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(asset.id)
            .bind(source.slot.to_string())
            .bind(&source.filename)
            .bind(&source.mime)
            .bind(source.size_bytes)
            .bind(&source.sha256)
            .bind(source.has_draco)
            .bind(source.lod.map(|level| level as i16))
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        self.get(asset.id).await?.ok_or(sqlx::Error::RowNotFound)
    }

    /// Renames or re-categorises an asset. Returns `None` when the id is unknown.
    pub async fn update(
        &self,
        id: Uuid,
        name: Option<&str>,
        category: Option<AssetCategory>,
    ) -> Result<Option<Asset>, sqlx::Error> {
        let row = sqlx::query_as::<_, AssetRow>(
            "UPDATE assets
             SET name = COALESCE($2, name),
                 category = COALESCE($3, category),
                 version = version + 1,
                 updated_at = NOW()
             WHERE id = $1
             RETURNING id, name, category, model_kind, version, created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(category.map(|value| value.to_string()))
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(self.attach_sources(vec![row]).await?.pop())
    }

    pub async fn delete(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM assets WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Inserts or replaces a variant for one source slot and bumps the asset
    /// version so clients re-download.
    pub async fn upsert_variant(
        &self,
        asset_id: Uuid,
        slot: AssetSourceSlot,
        variant: NewAssetVariant,
    ) -> Result<Option<Asset>, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO asset_variants (
                 asset_id, slot, kind, filename, mime, size_bytes, sha256, width, height, reused
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (asset_id, slot, kind) DO UPDATE SET
                 filename = EXCLUDED.filename,
                 mime = EXCLUDED.mime,
                 size_bytes = EXCLUDED.size_bytes,
                 sha256 = EXCLUDED.sha256,
                 width = EXCLUDED.width,
                 height = EXCLUDED.height,
                 reused = EXCLUDED.reused,
                 created_at = NOW()",
        )
        .bind(asset_id)
        .bind(slot.to_string())
        .bind(variant.kind.to_string())
        .bind(&variant.filename)
        .bind(&variant.mime)
        .bind(variant.size_bytes)
        .bind(&variant.sha256)
        .bind(variant.width.map(|value| value as i32))
        .bind(variant.height.map(|value| value as i32))
        .bind(variant.reused)
        .execute(&mut *transaction)
        .await?;
        let updated = sqlx::query(
            "UPDATE assets
             SET version = version + 1, updated_at = NOW()
             WHERE id = $1",
        )
        .bind(asset_id)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        self.get(asset_id).await
    }

    async fn attach_sources(&self, rows: Vec<AssetRow>) -> Result<Vec<Asset>, sqlx::Error> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
        let sources = sqlx::query_as::<_, SourceRow>(
            "SELECT asset_id, slot, filename, mime, size_bytes, sha256, has_draco, lod
             FROM asset_sources
             WHERE asset_id = ANY($1)
             ORDER BY CASE slot
                 WHEN 'static' THEN 0
                 WHEN 'idle' THEN 1
                 ELSE 2
             END",
        )
        .bind(ids.clone())
        .fetch_all(&self.pool)
        .await?;
        let variants = sqlx::query_as::<_, VariantRow>(
            "SELECT asset_id, slot, kind, filename, mime, size_bytes, sha256,
                    width, height, reused
             FROM asset_variants
             WHERE asset_id = ANY($1)
             ORDER BY created_at ASC",
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let asset_id = row.id;
                let sources = sources
                    .iter()
                    .filter(|source| source.asset_id == asset_id)
                    .map(|source| {
                        let slot = source.slot();
                        let variants = variants
                            .iter()
                            .filter(|variant| {
                                variant.asset_id == asset_id && variant.slot_name() == slot
                            })
                            .map(|variant| variant.to_contract(asset_id, slot))
                            .collect();
                        source.to_contract(asset_id, variants)
                    })
                    .collect();
                row.into_contract(sources)
            })
            .collect())
    }
}

#[derive(Debug, FromRow)]
struct AssetRow {
    id: Uuid,
    name: Option<String>,
    category: Option<String>,
    model_kind: Option<String>,
    version: Option<i64>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

impl AssetRow {
    fn matches(&self, needle: &str) -> bool {
        self.name
            .as_deref()
            .is_some_and(|name| name.to_lowercase().contains(needle))
    }

    fn into_contract(self, sources: Vec<AssetSource>) -> Asset {
        Asset {
            id: self.id,
            name: self.name.unwrap_or_default(),
            category: self
                .category
                .as_deref()
                .and_then(|value| AssetCategory::from_str(value).ok())
                .unwrap_or(AssetCategory::Model),
            model_kind: self
                .model_kind
                .as_deref()
                .and_then(|value| AssetModelKind::from_str(value).ok()),
            sources,
            version: self.version.unwrap_or(1),
            created_at: wita_utc(self.created_at),
            updated_at: wita_utc(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct SourceRow {
    asset_id: Uuid,
    slot: Option<String>,
    filename: Option<String>,
    mime: Option<String>,
    size_bytes: Option<i64>,
    sha256: Option<String>,
    has_draco: Option<bool>,
    lod: Option<i16>,
}

impl SourceRow {
    fn slot(&self) -> AssetSourceSlot {
        self.slot
            .as_deref()
            .and_then(|value| AssetSourceSlot::from_str(value).ok())
            .unwrap_or(AssetSourceSlot::Static)
    }

    fn to_contract(&self, asset_id: Uuid, variants: Vec<AssetVariant>) -> AssetSource {
        let slot = self.slot();
        AssetSource {
            slot,
            file: AssetFile {
                url: asset_file_url(asset_id, slot, self.filename.as_deref().unwrap_or_default()),
                filename: self.filename.clone().unwrap_or_default(),
                mime: self.mime.clone().unwrap_or_default(),
                size_bytes: self.size_bytes.unwrap_or_default(),
                sha256: self.sha256.clone().unwrap_or_default(),
            },
            has_draco: self.has_draco.unwrap_or(false),
            lod: self.lod.map(|value| value.max(0) as u16),
            variants,
        }
    }
}

#[derive(Debug, FromRow)]
struct VariantRow {
    asset_id: Uuid,
    slot: Option<String>,
    kind: Option<String>,
    filename: Option<String>,
    mime: Option<String>,
    size_bytes: Option<i64>,
    sha256: Option<String>,
    width: Option<i32>,
    height: Option<i32>,
    reused: Option<bool>,
}

impl VariantRow {
    fn slot_name(&self) -> AssetSourceSlot {
        self.slot
            .as_deref()
            .and_then(|value| AssetSourceSlot::from_str(value).ok())
            .unwrap_or(AssetSourceSlot::Static)
    }

    fn to_contract(&self, asset_id: Uuid, slot: AssetSourceSlot) -> AssetVariant {
        let filename = self.filename.clone().unwrap_or_default();
        AssetVariant {
            kind: self
                .kind
                .as_deref()
                .and_then(|value| AssetVariantKind::from_str(value).ok())
                .unwrap_or(AssetVariantKind::Draco),
            url: asset_file_url(asset_id, slot, &filename),
            filename,
            mime: self.mime.clone().unwrap_or_default(),
            size_bytes: self.size_bytes.unwrap_or_default(),
            sha256: self.sha256.clone().unwrap_or_default(),
            width: self.width.map(|value| value.max(0) as u32),
            height: self.height.map(|value| value.max(0) as u32),
            reused: self.reused.unwrap_or(false),
        }
    }
}

/// Relative URL served by the `assets` router, e.g.
/// `/assets/<id>/animated/draco.glb`.
pub fn asset_file_url(asset_id: Uuid, slot: AssetSourceSlot, filename: &str) -> String {
    format!("/assets/{asset_id}/{slot}/{filename}")
}

fn wita_utc(value: Option<DateTime<Utc>>) -> Timestamp {
    let value = value.unwrap_or_else(Utc::now);
    value.with_timezone(&FixedOffset::east_opt(8 * 60 * 60).expect("valid WITA offset"))
}
