use crate::read::wita;
use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use std::str::FromStr;
use twg_contracts::Timestamp;
use twg_contracts::assets::{
    Asset, AssetCategory, AssetFile, AssetGroup, AssetModelKind, AssetMoveMode, AssetSource,
    AssetSourceSlot, AssetVariant, AssetVariantKind,
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
    pub move_mode: AssetMoveMode,
    pub group_id: Uuid,
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
            "SELECT a.id, a.name, a.category, a.model_kind, a.move_mode, a.version, a.created_at,
                    a.updated_at, a.group_id, g.name AS group_name
             FROM assets a
             LEFT JOIN asset_groups g ON g.id = a.group_id
             WHERE ($1::text IS NULL OR a.category = $1)
             ORDER BY a.created_at ASC",
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
            "SELECT a.id, a.name, a.category, a.model_kind, a.move_mode, a.version, a.created_at,
                    a.updated_at, a.group_id, g.name AS group_name
             FROM assets a
             LEFT JOIN asset_groups g ON g.id = a.group_id
             WHERE a.id = $1",
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
            "INSERT INTO assets (id, name, category, model_kind, move_mode, group_id)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(asset.id)
        .bind(&asset.name)
        .bind(asset.category.to_string())
        .bind(asset.model_kind.map(|kind| kind.to_string()))
        .bind(asset.move_mode.to_string())
        .bind(asset.group_id)
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
    ///
    /// `bump_version` should only be set when something the offline cache has to
    /// re-download changed (the category decides which files exist). A pure
    /// rename keeps the version so desktop clients do not fetch the same bytes
    /// again.
    pub async fn update(
        &self,
        id: Uuid,
        name: Option<&str>,
        category: Option<AssetCategory>,
        group_id: Option<Uuid>,
        move_mode: Option<AssetMoveMode>,
        bump_version: bool,
    ) -> Result<Option<Asset>, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE assets
             SET name = COALESCE($2, name),
                 category = COALESCE($3, category),
                 group_id = COALESCE($4, group_id),
                 move_mode = COALESCE($5, move_mode),
                 version = CASE WHEN $6 THEN version + 1 ELSE version END,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind(id)
        .bind(name)
        .bind(category.map(|value| value.to_string()))
        .bind(group_id)
        .bind(move_mode.map(|value| value.to_string()))
        .bind(bump_version)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.get(id).await
    }

    pub async fn list_groups(&self) -> Result<Vec<AssetGroup>, sqlx::Error> {
        let rows = sqlx::query_as::<_, AssetGroupRow>(
            "SELECT id, name, created_at, updated_at
             FROM asset_groups
             ORDER BY name ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(AssetGroupRow::into_contract).collect())
    }

    pub async fn group_exists(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM asset_groups WHERE id = $1)")
            .bind(id)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn create_group(&self, name: &str) -> Result<AssetGroup, sqlx::Error> {
        let row = sqlx::query_as::<_, AssetGroupRow>(
            "INSERT INTO asset_groups (name) VALUES ($1)
             RETURNING id, name, created_at, updated_at",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into_contract())
    }

    pub async fn update_group(
        &self,
        id: Uuid,
        name: &str,
    ) -> Result<Option<AssetGroup>, sqlx::Error> {
        let row = sqlx::query_as::<_, AssetGroupRow>(
            "UPDATE asset_groups SET name = $2, updated_at = NOW()
             WHERE id = $1
             RETURNING id, name, created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(AssetGroupRow::into_contract))
    }

    pub async fn delete_group(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM asset_groups WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Replaces the stored file of one source slot.
    ///
    /// The variants derived from the old file are dropped (their bytes no longer
    /// match the source) and the asset version is bumped so clients download the
    /// new upload.
    pub async fn replace_source(
        &self,
        asset_id: Uuid,
        source: NewAssetSource,
    ) -> Result<Option<Asset>, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE asset_sources
             SET filename = $3, mime = $4, size_bytes = $5, sha256 = $6,
                 has_draco = $7, lod = $8
             WHERE asset_id = $1 AND slot = $2",
        )
        .bind(asset_id)
        .bind(source.slot.to_string())
        .bind(&source.filename)
        .bind(&source.mime)
        .bind(source.size_bytes)
        .bind(&source.sha256)
        .bind(source.has_draco)
        .bind(source.lod.map(|level| level as i16))
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            transaction.rollback().await?;
            return Ok(None);
        }
        sqlx::query("DELETE FROM asset_variants WHERE asset_id = $1 AND slot = $2")
            .bind(asset_id)
            .bind(source.slot.to_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE assets SET version = version + 1, updated_at = NOW() WHERE id = $1")
            .bind(asset_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.get(asset_id).await
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

    /// Updates size/sha256 for one stored file and bumps the asset version when
    /// something actually changed. Used by the asset-store repair pass after
    /// normalizing embedded GLB data.
    pub async fn sync_file_metadata(
        &self,
        asset_id: Uuid,
        slot: AssetSourceSlot,
        filename: &str,
        size_bytes: i64,
        sha256: &str,
    ) -> Result<bool, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let source = sqlx::query(
            "UPDATE asset_sources
             SET size_bytes = $4, sha256 = $5
             WHERE asset_id = $1 AND slot = $2 AND filename = $3
               AND (size_bytes IS DISTINCT FROM $4 OR sha256 IS DISTINCT FROM $5)",
        )
        .bind(asset_id)
        .bind(slot.to_string())
        .bind(filename)
        .bind(size_bytes)
        .bind(sha256)
        .execute(&mut *transaction)
        .await?;
        let variant = sqlx::query(
            "UPDATE asset_variants
             SET size_bytes = $4, sha256 = $5
             WHERE asset_id = $1 AND slot = $2 AND filename = $3
               AND (size_bytes IS DISTINCT FROM $4 OR sha256 IS DISTINCT FROM $5)",
        )
        .bind(asset_id)
        .bind(slot.to_string())
        .bind(filename)
        .bind(size_bytes)
        .bind(sha256)
        .execute(&mut *transaction)
        .await?;
        let changed = source.rows_affected() + variant.rows_affected() > 0;
        if changed {
            sqlx::query(
                "UPDATE assets
                 SET version = version + 1, updated_at = NOW()
                 WHERE id = $1",
            )
            .bind(asset_id)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(changed)
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
struct AssetGroupRow {
    id: Uuid,
    name: Option<String>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl AssetGroupRow {
    fn into_contract(self) -> AssetGroup {
        AssetGroup {
            id: self.id,
            name: self.name.unwrap_or_default(),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct AssetRow {
    id: Uuid,
    name: Option<String>,
    category: Option<String>,
    model_kind: Option<String>,
    move_mode: Option<String>,
    version: Option<i64>,
    group_id: Option<Uuid>,
    group_name: Option<String>,
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
        let category = self
            .category
            .as_deref()
            .and_then(|value| AssetCategory::from_str(value).ok())
            .unwrap_or(AssetCategory::Model);
        let model_kind = self
            .model_kind
            .as_deref()
            .and_then(|value| AssetModelKind::from_str(value).ok());
        let stored_move_mode = self
            .move_mode
            .as_deref()
            .and_then(|value| AssetMoveMode::from_str(value).ok())
            .unwrap_or_default();
        // Animated models always walk to their target, and images never move;
        // normalise here so every client sees the effective mode.
        let move_mode = if !category.is_model() {
            AssetMoveMode::None
        } else if model_kind == Some(AssetModelKind::Animated) {
            AssetMoveMode::Free
        } else {
            stored_move_mode
        };
        Asset {
            id: self.id,
            name: self.name.unwrap_or_default(),
            category,
            model_kind,
            move_mode,
            group_id: self.group_id,
            group_name: self.group_name,
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
