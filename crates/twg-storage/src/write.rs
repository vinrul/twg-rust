use std::collections::HashMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use thiserror::Error;
use twg_contracts::alurs::Alur;
use twg_contracts::animations::Animation;
use twg_contracts::categories::Category;
use twg_contracts::map::{Building, Line, Marker, Particle, TextMarker};
use twg_contracts::operations::Operation;
use twg_contracts::satkers::Satker;
use twg_contracts::scenarios::Scenario;
use twg_contracts::snapshots::Snapshot;
use twg_contracts::users::User;
use twg_domain::TextAlign;
use uuid::Uuid;

use crate::read::{ReadStore, ScenarioContext, wita};

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("{0}")]
    NotFound(&'static str),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

fn database_code(error: &sqlx::Error) -> Option<String> {
    match error {
        sqlx::Error::Database(database) => database.code().map(|code| code.into_owned()),
        _ => None,
    }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    database_code(error).as_deref() == Some("23505")
}

fn is_foreign_key_violation(error: &sqlx::Error) -> bool {
    database_code(error).as_deref() == Some("23503")
}

fn map_write_error(error: sqlx::Error, conflict: &str) -> WriteError {
    if is_unique_violation(&error) {
        return WriteError::Conflict(conflict.to_owned());
    }
    if is_foreign_key_violation(&error) {
        return WriteError::Invalid("referenced record not found".to_owned());
    }
    WriteError::Database(error)
}

#[derive(Clone)]
pub struct WriteStore {
    pool: PgPool,
    read: ReadStore,
}

#[derive(Debug, Clone)]
pub struct NewScenario {
    pub operasi_id: Uuid,
    pub satker_id: Option<Uuid>,
    pub name: String,
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
    pub min_zoom: f64,
    pub max_zoom: f64,
    pub pitch: f64,
    pub bearing: f64,
    pub map_mode: String,
    pub base_layer: String,
    pub sky_mode: String,
    pub indoor_config: String,
}

#[derive(Debug, Clone)]
pub struct ScenarioUpdate {
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
    pub min_zoom: f64,
    pub max_zoom: f64,
    pub pitch: f64,
    pub bearing: f64,
    pub map_mode: String,
    pub base_layer: String,
    pub sky_mode: String,
    pub indoor_config: String,
}

#[derive(Debug, Clone)]
pub struct NewAnimation {
    pub operasi_id: Uuid,
    pub skenario_id: Uuid,
    pub name: String,
    pub username: String,
    pub keterangan: String,
    pub tipe: String,
    pub data: String,
    pub geom: String,
    pub speed: i64,
    pub detik: i64,
    pub replay: Option<bool>,
    pub autoplay: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
pub struct AnimOrder {
    pub id: Uuid,
    pub urut: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpawnData {
    marker_id: String,
    name: String,
    rot_x: f64,
    rot_y: f64,
    rot_z: f64,
    pos_x: f64,
    pos_y: f64,
    scale: f64,
    scale_y: f64,
    scale_z: f64,
    altitude: f64,
    unit_id: String,
    kategori: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParticleSpawnData {
    particle_id: String,
    tipe: String,
    pos_x: f64,
    pos_y: f64,
    is_spawn: i32,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShootData {
    marker_id: String,
    scale: f64,
    height: f64,
    dir_forward: f64,
    spread: f64,
    speed: f64,
    angle: f64,
    smoke_count: f64,
    tipe: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CombineItem {
    id: Uuid,
    tipe: String,
}

#[derive(Debug, Deserialize)]
struct CombinePayload {
    #[serde(default)]
    items: Vec<CombineItem>,
    #[serde(default)]
    data: Vec<CombineItem>,
    #[serde(default)]
    anims: Vec<CombineItem>,
}

impl WriteStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            read: ReadStore::new(pool.clone()),
            pool,
        }
    }

    pub fn read(&self) -> &ReadStore {
        &self.read
    }

    async fn context(&self, skenario_id: Uuid) -> Result<ScenarioContext, WriteError> {
        self.read
            .scenario_context(skenario_id)
            .await?
            .ok_or(WriteError::NotFound("skenario not found"))
    }

    // ---------------------------------------------------------------- categories

    pub async fn create_category(&self, name: &str, icon: &str) -> Result<Category, WriteError> {
        let row = sqlx::query_as::<_, CategoryRow>(
            "INSERT INTO categories (name, icon) VALUES ($1, $2)
             RETURNING id, name, icon, created_at, updated_at",
        )
        .bind(name)
        .bind(icon)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "category already exists"))?;
        Ok(row.into_contract())
    }

    pub async fn update_category(
        &self,
        id: Uuid,
        name: &str,
        icon: &str,
    ) -> Result<Category, WriteError> {
        let row = sqlx::query_as::<_, CategoryRow>(
            "UPDATE categories
             SET name = $2, icon = $3, updated_at = NOW()
             WHERE id = $1
             RETURNING id, name, icon, created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(icon)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "category already exists"))?
        .ok_or(WriteError::NotFound("category not found"))?;
        Ok(row.into_contract())
    }

    pub async fn delete_category(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM categories WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("category not found"));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- operations

    pub async fn create_operation(
        &self,
        name: &str,
        satker_id: Option<Uuid>,
    ) -> Result<Operation, WriteError> {
        let row = sqlx::query_as::<_, OperationRow>(
            "INSERT INTO operasis (name, satker_id) VALUES ($1, $2)
             RETURNING id, name, satker_id, created_at, updated_at",
        )
        .bind(name)
        .bind(satker_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "operasi already exists"))?;
        Ok(row.into_contract())
    }

    /// Updates an operation. When `satker_id` is set, the same value is cascaded
    /// to every scenario in the operation inside one transaction.
    pub async fn update_operation(
        &self,
        id: Uuid,
        name: &str,
        satker_id: Option<Uuid>,
    ) -> Result<Operation, WriteError> {
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query_as::<_, OperationRow>(
            "UPDATE operasis
             SET name = $2, satker_id = COALESCE($3, satker_id), updated_at = NOW()
             WHERE id = $1
             RETURNING id, name, satker_id, created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(satker_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| map_write_error(error, "operasi already exists"))?
        .ok_or(WriteError::NotFound("operasi not found"))?;
        if satker_id.is_some() {
            sqlx::query(
                "UPDATE skenarios SET satker_id = $2, updated_at = NOW()
                 WHERE operasi_id = $1",
            )
            .bind(id)
            .bind(satker_id)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(row.into_contract())
    }

    pub async fn delete_operation(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM operasis WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("operasi not found"));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- scenarios

    pub async fn create_scenario(&self, new: &NewScenario) -> Result<Scenario, WriteError> {
        let id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO skenarios (
                 name, operasi_id, satker_id, center_x, center_y, zoom, min_zoom, max_zoom,
                 pitch, bearing, map_mode, base_layer, sky_mode, indoor_config
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
             RETURNING id",
        )
        .bind(&new.name)
        .bind(new.operasi_id)
        .bind(new.satker_id)
        .bind(new.center_x)
        .bind(new.center_y)
        .bind(new.zoom)
        .bind(new.min_zoom)
        .bind(new.max_zoom)
        .bind(new.pitch)
        .bind(new.bearing)
        .bind(&new.map_mode)
        .bind(&new.base_layer)
        .bind(&new.sky_mode)
        .bind(&new.indoor_config)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "skenario already exists"))?;
        self.read
            .get_scenario(id)
            .await?
            .ok_or(WriteError::NotFound("skenario not found"))
    }

    pub async fn update_scenario(
        &self,
        id: Uuid,
        update: &ScenarioUpdate,
    ) -> Result<Scenario, WriteError> {
        let row = sqlx::query_as::<_, ScenarioLookupRow>(
            "UPDATE skenarios
             SET center_x = $2, center_y = $3, zoom = $4, min_zoom = $5, max_zoom = $6,
                 pitch = $7, bearing = $8, map_mode = $9, base_layer = $10,
                 sky_mode = $11, indoor_config = $12, updated_at = NOW()
             WHERE id = $1
             RETURNING id, operasi_id",
        )
        .bind(id)
        .bind(update.center_x)
        .bind(update.center_y)
        .bind(update.zoom)
        .bind(update.min_zoom)
        .bind(update.max_zoom)
        .bind(update.pitch)
        .bind(update.bearing)
        .bind(&update.map_mode)
        .bind(&update.base_layer)
        .bind(&update.sky_mode)
        .bind(&update.indoor_config)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("skenario not found"))?;
        self.read
            .get_scenario(row.id)
            .await?
            .ok_or(WriteError::NotFound("skenario not found"))
    }

    pub async fn update_scenario_name(&self, id: Uuid, name: &str) -> Result<Scenario, WriteError> {
        let result =
            sqlx::query("UPDATE skenarios SET name = $2, updated_at = NOW() WHERE id = $1")
                .bind(id)
                .bind(name)
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("skenario not found"));
        }
        self.read
            .get_scenario(id)
            .await?
            .ok_or(WriteError::NotFound("skenario not found"))
    }

    pub async fn delete_scenario(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM skenarios WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("skenario not found"));
        }
        Ok(())
    }

    /// Copies a scenario and every child entity in a single transaction.
    ///
    /// Marker, particle, and animation ids inside animation `data`/`geom` JSON
    /// are remapped to the ids of the copied rows, matching the legacy server.
    pub async fn copy_scenario(
        &self,
        name: &str,
        source_id: Uuid,
        target_operasi_id: Uuid,
    ) -> Result<(), WriteError> {
        let mut transaction = self.pool.begin().await?;

        let source = sqlx::query_as::<_, CopyScenarioRow>(
            "SELECT id, operasi_id, center_x, center_y, zoom, min_zoom, max_zoom,
                    pitch, bearing, map_mode, base_layer, sky_mode, indoor_config
             FROM skenarios WHERE id = $1",
        )
        .bind(source_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(WriteError::NotFound("skenario not found"))?;

        let new_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO skenarios (
                 name, operasi_id, center_x, center_y, zoom, min_zoom, max_zoom,
                 pitch, bearing, map_mode, base_layer, sky_mode, indoor_config
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
             RETURNING id",
        )
        .bind(name)
        .bind(target_operasi_id)
        .bind(source.center_x)
        .bind(source.center_y)
        .bind(source.zoom)
        .bind(source.min_zoom)
        .bind(source.max_zoom)
        .bind(source.pitch)
        .bind(source.bearing)
        .bind(&source.map_mode)
        .bind(&source.base_layer)
        .bind(
            source
                .sky_mode
                .clone()
                .unwrap_or_else(|| "black".to_owned()),
        )
        .bind(&source.indoor_config)
        .fetch_one(&mut *transaction)
        .await?;

        let markers = sqlx::query_as::<_, CopyMarkerRow>(
            "SELECT id, unit_id, name, jumlah, rot_x, rot_y, rot_z, pos_x, pos_y,
                    keterangan, kategori, is_spawn, shoot, scale, scale_y, scale_z, altitude,
                    obstacle
             FROM markers WHERE skenario_id = $1 ORDER BY created_at DESC",
        )
        .bind(source_id)
        .fetch_all(&mut *transaction)
        .await?;

        let mut replacements: HashMap<String, String> = HashMap::new();
        replacements.insert(source_id.to_string(), new_id.to_string());
        replacements.insert(source.operasi_id.to_string(), target_operasi_id.to_string());

        for marker in &markers {
            let marker_id = sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO markers (
                     name, operasi_id, skenario_id, unit_id, jumlah, rot_x, rot_y, rot_z,
                     pos_x, pos_y, keterangan, kategori, scale, is_spawn, shoot, scale_y,
                     scale_z, altitude, obstacle
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                           $14, $15, $16, $17, $18, $19)
                 RETURNING id",
            )
            .bind(&marker.name)
            .bind(target_operasi_id)
            .bind(new_id)
            .bind(marker.unit_id)
            .bind(marker.jumlah)
            .bind(marker.rot_x)
            .bind(marker.rot_y)
            .bind(marker.rot_z)
            .bind(marker.pos_x)
            .bind(marker.pos_y)
            .bind(&marker.keterangan)
            .bind(&marker.kategori)
            .bind(marker.scale)
            .bind(marker.is_spawn)
            .bind(&marker.shoot)
            .bind(marker.scale_y)
            .bind(marker.scale_z)
            .bind(marker.altitude)
            .bind(marker.obstacle)
            .fetch_one(&mut *transaction)
            .await?;
            replacements.insert(marker.id.to_string(), marker_id.to_string());
        }

        let buildings = sqlx::query_as::<_, CopyBuildingRow>(
            "SELECT name, keterangan, geom, color, height
             FROM buildings WHERE skenario_id = $1 ORDER BY created_at DESC",
        )
        .bind(source_id)
        .fetch_all(&mut *transaction)
        .await?;
        for building in buildings {
            sqlx::query(
                "INSERT INTO buildings (operasi_id, skenario_id, name, keterangan, geom, color, height)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(target_operasi_id)
            .bind(new_id)
            .bind(building.name)
            .bind(building.keterangan)
            .bind(building.geom)
            .bind(building.color)
            .bind(building.height)
            .execute(&mut *transaction)
            .await?;
        }

        let lines = sqlx::query_as::<_, CopyLineRow>(
            "SELECT name, color, width, alpha, geom
             FROM plot_lines WHERE skenario_id = $1 ORDER BY created_at DESC",
        )
        .bind(source_id)
        .fetch_all(&mut *transaction)
        .await?;
        for line in lines {
            sqlx::query(
                "INSERT INTO plot_lines (operasi_id, skenario_id, name, color, width, alpha, geom)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(target_operasi_id)
            .bind(new_id)
            .bind(line.name)
            .bind(line.color)
            .bind(line.width)
            .bind(line.alpha)
            .bind(line.geom)
            .execute(&mut *transaction)
            .await?;
        }

        let text_markers = sqlx::query_as::<_, CopyTextMarkerRow>(
            "SELECT text, color, font_size, align, wrap_text, pos_x, pos_y, altitude
             FROM text_markers WHERE skenario_id = $1 ORDER BY created_at ASC",
        )
        .bind(source_id)
        .fetch_all(&mut *transaction)
        .await?;
        for marker in text_markers {
            sqlx::query(
                "INSERT INTO text_markers (
                     operasi_id, skenario_id, text, color, font_size, align, wrap_text,
                     pos_x, pos_y, altitude
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(target_operasi_id)
            .bind(new_id)
            .bind(marker.text)
            .bind(marker.color)
            .bind(marker.font_size)
            .bind(marker.align)
            .bind(marker.wrap_text)
            .bind(marker.pos_x)
            .bind(marker.pos_y)
            .bind(marker.altitude)
            .execute(&mut *transaction)
            .await?;
        }

        let particles = sqlx::query_as::<_, CopyParticleRow>(
            "SELECT id, name, pos_x, pos_y, data, is_spawn, tipe, altitude
             FROM particles WHERE skenario_id = $1 ORDER BY created_at DESC",
        )
        .bind(source_id)
        .fetch_all(&mut *transaction)
        .await?;
        for particle in &particles {
            let particle_id = sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO particles (name, operasi_id, skenario_id, pos_x, pos_y, data, is_spawn, tipe, altitude)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 RETURNING id",
            )
            .bind(&particle.name)
            .bind(target_operasi_id)
            .bind(new_id)
            .bind(particle.pos_x)
            .bind(particle.pos_y)
            .bind(&particle.data)
            .bind(particle.is_spawn)
            .bind(&particle.tipe)
            .bind(particle.altitude)
            .fetch_one(&mut *transaction)
            .await?;
            replacements.insert(particle.id.to_string(), particle_id.to_string());
        }

        let anims = sqlx::query_as::<_, CopyAnimRow>(
            "SELECT id, name, username, keterangan, tipe, data, geom, speed, detik,
                    urut, replay, autoplay
             FROM anims WHERE skenario_id = $1",
        )
        .bind(source_id)
        .fetch_all(&mut *transaction)
        .await?;

        let mut copied = Vec::with_capacity(anims.len());
        for anim in &anims {
            let anim_id = sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO anims (
                     operasi_id, skenario_id, name, username, keterangan, tipe, data,
                     geom, speed, detik, replay, autoplay, urut
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                 RETURNING id",
            )
            .bind(target_operasi_id)
            .bind(new_id)
            .bind(&anim.name)
            .bind(&anim.username)
            .bind(&anim.keterangan)
            .bind(&anim.tipe)
            .bind(&anim.data)
            .bind(&anim.geom)
            .bind(anim.speed)
            .bind(anim.detik)
            .bind(anim.replay)
            .bind(anim.autoplay)
            .bind(anim.urut)
            .fetch_one(&mut *transaction)
            .await?;
            replacements.insert(anim.id.to_string(), anim_id.to_string());
            copied.push((
                anim_id,
                anim.data.clone(),
                anim.geom.clone(),
                anim.name.clone(),
                anim.username.clone(),
                anim.keterangan.clone(),
            ));
        }

        for (anim_id, data, geom, name, username, keterangan) in copied {
            let rewritten = remap_json_data_string(data.as_deref().unwrap_or(""), &replacements);
            sqlx::query(
                "UPDATE anims
                 SET name = $2, username = $3, keterangan = $4, data = $5, geom = $6,
                     updated_at = NOW()
                 WHERE id = $1",
            )
            .bind(anim_id)
            .bind(name)
            .bind(username)
            .bind(keterangan)
            .bind(rewritten)
            .bind(geom)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    // ---------------------------------------------------------------- users

    #[allow(clippy::too_many_arguments)]
    pub async fn create_user(
        &self,
        name: &str,
        username: &str,
        password_hash: &str,
        role: &str,
        profile_img: &str,
        units: &str,
        satker_id: Option<Uuid>,
    ) -> Result<User, WriteError> {
        let row = sqlx::query_as::<_, UserRow>(
            "INSERT INTO users (name, username, password, role, profile_img, units, satker_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id, name, username, role, satker_id, profile_img, units,
                       created_at, updated_at",
        )
        .bind(name)
        .bind(username)
        .bind(password_hash)
        .bind(role)
        .bind(profile_img)
        .bind(units)
        .bind(satker_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "username already exists"))?;
        Ok(row.into_contract())
    }

    pub async fn update_user(
        &self,
        id: Uuid,
        name: &str,
        units: &str,
        profile_img: &str,
        satker_id: Option<Uuid>,
    ) -> Result<User, WriteError> {
        let row = sqlx::query_as::<_, UserRow>(
            "UPDATE users
             SET name = $2, units = $3, profile_img = $4,
                 satker_id = COALESCE($5, satker_id), updated_at = NOW()
             WHERE id = $1
             RETURNING id, name, username, role, satker_id, profile_img, units,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(units)
        .bind(profile_img)
        .bind(satker_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("user not found"))?;
        Ok(row.into_contract())
    }

    pub async fn update_profile(
        &self,
        id: Uuid,
        name: &str,
        profile_img: &str,
    ) -> Result<User, WriteError> {
        let row = sqlx::query_as::<_, UserRow>(
            "UPDATE users SET name = $2, profile_img = $3, updated_at = NOW()
             WHERE id = $1
             RETURNING id, name, username, role, satker_id, profile_img, units,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(profile_img)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("user not found"))?;
        Ok(row.into_contract())
    }

    pub async fn delete_user(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("user not found"));
        }
        Ok(())
    }

    pub async fn revoke_user_sessions(&self, user_id: Uuid) -> Result<u64, WriteError> {
        let result = sqlx::query(
            "UPDATE user_sessions SET revoked_at = NOW()
             WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // ---------------------------------------------------------------- satkers

    pub async fn create_satker(
        &self,
        code: &str,
        name: &str,
        is_active: bool,
        created_by: Uuid,
    ) -> Result<Satker, WriteError> {
        let row = sqlx::query_as::<_, SatkerRow>(
            "INSERT INTO satkers (code, name, is_active, created_by)
             VALUES ($1, $2, $3, $4)
             RETURNING id, code, name, is_active, created_by, created_at, updated_at",
        )
        .bind(code)
        .bind(name)
        .bind(is_active)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "satker already exists"))?;
        Ok(row.into_contract())
    }

    pub async fn update_satker(
        &self,
        id: Uuid,
        code: &str,
        name: &str,
        is_active: bool,
    ) -> Result<Satker, WriteError> {
        let row = sqlx::query_as::<_, SatkerRow>(
            "UPDATE satkers
             SET code = $2, name = $3, is_active = $4, updated_at = NOW()
             WHERE id = $1
             RETURNING id, code, name, is_active, created_by, created_at, updated_at",
        )
        .bind(id)
        .bind(code)
        .bind(name)
        .bind(is_active)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "satker already exists"))?
        .ok_or(WriteError::NotFound("satker not found"))?;
        Ok(row.into_contract())
    }

    pub async fn delete_satker(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM satkers WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("satker not found"));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- alurs

    pub async fn create_alur(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        alur: &str,
    ) -> Result<Alur, WriteError> {
        let row = sqlx::query_as::<_, AlurRow>(
            "INSERT INTO alurs (operasi_id, skenario_id, alur) VALUES ($1, $2, $3)
             RETURNING id, alur, created_at, updated_at",
        )
        .bind(operasi_id)
        .bind(skenario_id)
        .bind(alur)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "alur already exists"))?;
        let context = self.context(skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_alur(&self, id: Uuid, alur: &str) -> Result<Alur, WriteError> {
        let row = sqlx::query_as::<_, AlurWithScenarioRow>(
            "UPDATE alurs SET alur = $2, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, alur, created_at, updated_at",
        )
        .bind(id)
        .bind(alur)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("alur not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn delete_alur(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM alurs WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("alur not found"));
        }
        Ok(())
    }
}

impl WriteStore {
    // ---------------------------------------------------------------- markers

    #[allow(clippy::too_many_arguments)]
    pub async fn create_marker(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        unit_id: Uuid,
        name: &str,
        kategori: &str,
        rot_x: f64,
        rot_y: f64,
        rot_z: f64,
        pos_x: f64,
        pos_y: f64,
        jumlah: i32,
        obstacle: bool,
        keterangan: &str,
    ) -> Result<Marker, WriteError> {
        let row = sqlx::query_as::<_, MarkerRow>(
            "INSERT INTO markers (
                 name, operasi_id, skenario_id, unit_id, jumlah, rot_x, rot_y, rot_z,
                 pos_x, pos_y, keterangan, kategori, obstacle
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
             RETURNING id, skenario_id, name, unit_id, kategori, jumlah, rot_x, rot_y,
                       rot_z, pos_x, pos_y, scale, scale_y, scale_z, altitude, is_spawn, obstacle,
                       keterangan, shoot, created_at, updated_at",
        )
        .bind(name)
        .bind(operasi_id)
        .bind(skenario_id)
        .bind(unit_id)
        .bind(jumlah)
        .bind(rot_x)
        .bind(rot_y)
        .bind(rot_z)
        .bind(pos_x)
        .bind(pos_y)
        .bind(keterangan)
        .bind(kategori)
        .bind(obstacle)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "marker already exists"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_marker_geom(
        &self,
        id: Uuid,
        rot_x: f64,
        rot_y: f64,
        rot_z: f64,
        pos_x: f64,
        pos_y: f64,
    ) -> Result<Marker, WriteError> {
        let row = sqlx::query_as::<_, MarkerRow>(
            "UPDATE markers
             SET pos_x = $2, pos_y = $3, rot_x = $4, rot_y = $5, rot_z = $6, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, unit_id, kategori, jumlah, rot_x, rot_y,
                       rot_z, pos_x, pos_y, scale, scale_y, scale_z, altitude, is_spawn, obstacle,
                       keterangan, shoot, created_at, updated_at",
        )
        .bind(id)
        .bind(pos_x)
        .bind(pos_y)
        .bind(rot_x)
        .bind(rot_y)
        .bind(rot_z)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("marker not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_marker_name(
        &self,
        id: Uuid,
        name: &str,
        jumlah: i32,
        keterangan: &str,
    ) -> Result<Marker, WriteError> {
        let row = sqlx::query_as::<_, MarkerRow>(
            "UPDATE markers
             SET name = $2, jumlah = $3, keterangan = $4, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, unit_id, kategori, jumlah, rot_x, rot_y,
                       rot_z, pos_x, pos_y, scale, scale_y, scale_z, altitude, is_spawn, obstacle,
                       keterangan, shoot, created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(jumlah)
        .bind(keterangan)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("marker not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_marker_obstacle(
        &self,
        id: Uuid,
        obstacle: bool,
    ) -> Result<Marker, WriteError> {
        let row = sqlx::query_as::<_, MarkerRow>(
            "UPDATE markers
             SET obstacle = $2, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, unit_id, kategori, jumlah, rot_x, rot_y,
                       rot_z, pos_x, pos_y, scale, scale_y, scale_z, altitude, is_spawn, obstacle,
                       keterangan, shoot, created_at, updated_at",
        )
        .bind(id)
        .bind(obstacle)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("marker not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_marker_unit(&self, id: Uuid, unit_id: Uuid) -> Result<Marker, WriteError> {
        let row = sqlx::query_as::<_, MarkerRow>(
            "UPDATE markers
             SET unit_id = $2, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, unit_id, kategori, jumlah, rot_x, rot_y,
                       rot_z, pos_x, pos_y, scale, scale_y, scale_z, altitude, is_spawn, obstacle,
                       keterangan, shoot, created_at, updated_at",
        )
        .bind(id)
        .bind(unit_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("marker not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_marker_scale(
        &self,
        id: Uuid,
        scale: f64,
        scale_y: f64,
        scale_z: f64,
    ) -> Result<Marker, WriteError> {
        let row = sqlx::query_as::<_, MarkerRow>(
            "UPDATE markers
             SET scale = $2, scale_y = $3, scale_z = $4, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, unit_id, kategori, jumlah, rot_x, rot_y,
                       rot_z, pos_x, pos_y, scale, scale_y, scale_z, altitude, is_spawn, obstacle,
                       keterangan, shoot, created_at, updated_at",
        )
        .bind(id)
        .bind(scale)
        .bind(scale_y)
        .bind(scale_z)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("marker not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_marker_altitude(
        &self,
        id: Uuid,
        altitude: f64,
    ) -> Result<Marker, WriteError> {
        let row = sqlx::query_as::<_, MarkerRow>(
            "UPDATE markers
             SET altitude = $2, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, unit_id, kategori, jumlah, rot_x, rot_y,
                       rot_z, pos_x, pos_y, scale, scale_y, scale_z, altitude, is_spawn, obstacle,
                       keterangan, shoot, created_at, updated_at",
        )
        .bind(id)
        .bind(altitude)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("marker not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_marker_rotation(
        &self,
        id: Uuid,
        rotasi: f64,
        rotasi_x: f64,
        rotasi_y: f64,
    ) -> Result<Marker, WriteError> {
        let row = sqlx::query_as::<_, MarkerRow>(
            "UPDATE markers
             SET rot_z = $2, rot_x = $3, rot_y = $4, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, unit_id, kategori, jumlah, rot_x, rot_y,
                       rot_z, pos_x, pos_y, scale, scale_y, scale_z, altitude, is_spawn, obstacle,
                       keterangan, shoot, created_at, updated_at",
        )
        .bind(id)
        .bind(rotasi)
        .bind(rotasi_x)
        .bind(rotasi_y)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("marker not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn duplicate_marker(
        &self,
        id: Uuid,
        pos_x: f64,
        pos_y: f64,
    ) -> Result<Marker, WriteError> {
        let source = sqlx::query_as::<_, MarkerCopyRow>(
            "SELECT operasi_id, skenario_id, unit_id, name, jumlah, rot_x, rot_y, rot_z,
                    keterangan, kategori, scale, is_spawn, shoot, scale_y, scale_z, altitude,
                    obstacle
             FROM markers WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("marker not found"))?;
        let row = sqlx::query_as::<_, MarkerRow>(
            "INSERT INTO markers (
                 name, operasi_id, skenario_id, unit_id, jumlah, rot_x, rot_y, rot_z,
                 pos_x, pos_y, keterangan, kategori, scale, is_spawn, shoot, scale_y,
                 scale_z, altitude, obstacle
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                       $16, $17, $18, $19)
             RETURNING id, skenario_id, name, unit_id, kategori, jumlah, rot_x, rot_y,
                       rot_z, pos_x, pos_y, scale, scale_y, scale_z, altitude, is_spawn, obstacle,
                       keterangan, shoot, created_at, updated_at",
        )
        .bind(&source.name)
        .bind(source.operasi_id)
        .bind(source.skenario_id)
        .bind(source.unit_id)
        .bind(source.jumlah)
        .bind(source.rot_x)
        .bind(source.rot_y)
        .bind(source.rot_z)
        .bind(pos_x)
        .bind(pos_y)
        .bind(&source.keterangan)
        .bind(&source.kategori)
        .bind(source.scale)
        .bind(source.is_spawn)
        .bind(&source.shoot)
        .bind(source.scale_y)
        .bind(source.scale_z)
        .bind(source.altitude)
        .bind(source.obstacle)
        .fetch_one(&self.pool)
        .await?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn delete_marker(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM markers WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("marker not found"));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- buildings

    #[allow(clippy::too_many_arguments)]
    pub async fn create_building(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        name: &str,
        color: &str,
        geom: &str,
        height: f64,
        keterangan: &str,
    ) -> Result<Building, WriteError> {
        let row = sqlx::query_as::<_, BuildingRow>(
            "INSERT INTO buildings (name, operasi_id, skenario_id, keterangan, color, geom, height)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id, skenario_id, name, keterangan, color, geom, height, created_at,
                       updated_at",
        )
        .bind(name)
        .bind(operasi_id)
        .bind(skenario_id)
        .bind(keterangan)
        .bind(color)
        .bind(geom)
        .bind(height)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "building already exists"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_building(
        &self,
        id: Uuid,
        name: &str,
        color: &str,
        height: f64,
        keterangan: &str,
    ) -> Result<Building, WriteError> {
        let row = sqlx::query_as::<_, BuildingRow>(
            "UPDATE buildings
             SET name = $2, color = $3, height = $4, keterangan = $5, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, keterangan, color, geom, height, created_at,
                       updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(color)
        .bind(height)
        .bind(keterangan)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("building not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_building_geom(&self, id: Uuid, geom: &str) -> Result<Building, WriteError> {
        let row = sqlx::query_as::<_, BuildingRow>(
            "UPDATE buildings SET geom = $2, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, keterangan, color, geom, height, created_at,
                       updated_at",
        )
        .bind(id)
        .bind(geom)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("building not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn delete_building(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM buildings WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("building not found"));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- lines

    #[allow(clippy::too_many_arguments)]
    pub async fn create_line(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        name: &str,
        color: &str,
        width: f64,
        alpha: f64,
        geom: &str,
    ) -> Result<Line, WriteError> {
        let row = sqlx::query_as::<_, LineRow>(
            "INSERT INTO plot_lines (name, operasi_id, skenario_id, color, width, alpha, geom)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id, skenario_id, name, color, width, alpha, geom, created_at, updated_at",
        )
        .bind(name)
        .bind(operasi_id)
        .bind(skenario_id)
        .bind(color)
        .bind(width)
        .bind(alpha)
        .bind(geom)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "invalid line"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_line(
        &self,
        id: Uuid,
        name: &str,
        color: &str,
        width: f64,
        alpha: f64,
    ) -> Result<Line, WriteError> {
        let row = sqlx::query_as::<_, LineRow>(
            "UPDATE plot_lines
             SET name = $2, color = $3, width = $4, alpha = $5, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, color, width, alpha, geom, created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(color)
        .bind(width)
        .bind(alpha)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "invalid line"))?
        .ok_or(WriteError::NotFound("line not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_line_geom(&self, id: Uuid, geom: &str) -> Result<Line, WriteError> {
        let row = sqlx::query_as::<_, LineRow>(
            "UPDATE plot_lines SET geom = $2, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, color, width, alpha, geom, created_at, updated_at",
        )
        .bind(id)
        .bind(geom)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("line not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn delete_line(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM plot_lines WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("line not found"));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- text markers

    #[allow(clippy::too_many_arguments)]
    pub async fn create_text_marker(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        text: &str,
        color: &str,
        font_size: f64,
        align: TextAlign,
        wrap_text: bool,
        pos_x: f64,
        pos_y: f64,
        altitude: f64,
    ) -> Result<TextMarker, WriteError> {
        let row = sqlx::query_as::<_, TextMarkerRow>(
            "INSERT INTO text_markers (
                 operasi_id, skenario_id, text, color, font_size, align, wrap_text,
                 pos_x, pos_y, altitude
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING id, skenario_id, text, color, font_size, align, wrap_text, pos_x,
                       pos_y, altitude, created_at, updated_at",
        )
        .bind(operasi_id)
        .bind(skenario_id)
        .bind(text)
        .bind(color)
        .bind(font_size)
        .bind(text_align_str(align))
        .bind(wrap_text)
        .bind(pos_x)
        .bind(pos_y)
        .bind(altitude)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "text marker already exists"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_text_marker(
        &self,
        id: Uuid,
        text: &str,
        color: &str,
        font_size: f64,
        align: TextAlign,
        wrap_text: bool,
    ) -> Result<TextMarker, WriteError> {
        let row = sqlx::query_as::<_, TextMarkerRow>(
            "UPDATE text_markers
             SET text = $2, color = $3, font_size = $4, align = $5, wrap_text = $6,
                 updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, text, color, font_size, align, wrap_text, pos_x,
                       pos_y, altitude, created_at, updated_at",
        )
        .bind(id)
        .bind(text)
        .bind(color)
        .bind(font_size)
        .bind(text_align_str(align))
        .bind(wrap_text)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("text marker not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_text_marker_geom(
        &self,
        id: Uuid,
        pos_x: f64,
        pos_y: f64,
        altitude: f64,
    ) -> Result<TextMarker, WriteError> {
        let row = sqlx::query_as::<_, TextMarkerRow>(
            "UPDATE text_markers SET pos_x = $2, pos_y = $3, altitude = $4, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, text, color, font_size, align, wrap_text, pos_x,
                       pos_y, altitude, created_at, updated_at",
        )
        .bind(id)
        .bind(pos_x)
        .bind(pos_y)
        .bind(altitude)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("text marker not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn delete_text_marker(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM text_markers WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("text marker not found"));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- particles

    #[allow(clippy::too_many_arguments)]
    pub async fn create_particle(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        name: &str,
        tipe: &str,
        data: &str,
        pos_x: f64,
        pos_y: f64,
        altitude: f64,
    ) -> Result<Particle, WriteError> {
        let row = sqlx::query_as::<_, ParticleRow>(
            "INSERT INTO particles (
                 operasi_id, skenario_id, name, tipe, data, pos_x, pos_y, altitude, is_spawn
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 0)
             RETURNING id, skenario_id, tipe, name, data, pos_x, pos_y, altitude, is_spawn,
                       created_at, updated_at",
        )
        .bind(operasi_id)
        .bind(skenario_id)
        .bind(name)
        .bind(tipe)
        .bind(data)
        .bind(pos_x)
        .bind(pos_y)
        .bind(altitude)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "particle already exists"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_particle(
        &self,
        id: Uuid,
        name: &str,
        tipe: &str,
        data: &str,
    ) -> Result<Particle, WriteError> {
        let row = sqlx::query_as::<_, ParticleRow>(
            "UPDATE particles SET name = $2, tipe = $3, data = $4, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, tipe, name, data, pos_x, pos_y, altitude, is_spawn,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(tipe)
        .bind(data)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("particle not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_particle_geom(
        &self,
        id: Uuid,
        pos_x: f64,
        pos_y: f64,
        altitude: f64,
    ) -> Result<Particle, WriteError> {
        let row = sqlx::query_as::<_, ParticleRow>(
            "UPDATE particles SET pos_x = $2, pos_y = $3, altitude = $4, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, tipe, name, data, pos_x, pos_y, altitude, is_spawn,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(pos_x)
        .bind(pos_y)
        .bind(altitude)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("particle not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn delete_particle(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM particles WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("particle not found"));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- anims

    pub async fn create_anim(&self, new: &NewAnimation) -> Result<Animation, WriteError> {
        let row = sqlx::query_as::<_, AnimRow>(
            "INSERT INTO anims (
                 operasi_id, skenario_id, name, username, keterangan, tipe, data, geom,
                 speed, detik, replay, autoplay
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
             RETURNING id, skenario_id, name, username, keterangan, tipe, data, geom,
                       speed::int8 AS speed, detik::int8 AS detik, urut, replay, autoplay,
                       created_at, updated_at",
        )
        .bind(new.operasi_id)
        .bind(new.skenario_id)
        .bind(&new.name)
        .bind(&new.username)
        .bind(&new.keterangan)
        .bind(&new.tipe)
        .bind(&new.data)
        .bind(&new.geom)
        .bind(new.speed as i32)
        .bind(new.detik as i32)
        .bind(new.replay)
        .bind(new.autoplay)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "anim already exists"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_anim_route(
        &self,
        id: Uuid,
        name: &str,
        username: &str,
        keterangan: &str,
        data: &str,
        speed: i64,
        detik: i64,
        replay: Option<bool>,
        autoplay: Option<bool>,
    ) -> Result<Animation, WriteError> {
        let row = sqlx::query_as::<_, AnimRow>(
            "UPDATE anims
             SET name = $2, username = $3, keterangan = $4, data = $5, speed = $6,
                 detik = $7, replay = $8, autoplay = $9, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, username, keterangan, tipe, data, geom,
                       speed::int8 AS speed, detik::int8 AS detik, urut, replay, autoplay,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(username)
        .bind(keterangan)
        .bind(data)
        .bind(speed as i32)
        .bind(detik as i32)
        .bind(replay)
        .bind(autoplay)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("anim not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_anim_route_geom(
        &self,
        id: Uuid,
        geom: &str,
    ) -> Result<Animation, WriteError> {
        let row = sqlx::query_as::<_, AnimRow>(
            "UPDATE anims SET geom = $2, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, username, keterangan, tipe, data, geom,
                       speed::int8 AS speed, detik::int8 AS detik, urut, replay, autoplay,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(geom)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("anim not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_anim_spawn(
        &self,
        id: Uuid,
        name: &str,
        username: &str,
        keterangan: &str,
        data: &str,
        geom: &str,
    ) -> Result<Animation, WriteError> {
        let row = sqlx::query_as::<_, AnimRow>(
            "UPDATE anims
             SET name = $2, username = $3, keterangan = $4, data = $5, geom = $6,
                 updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, username, keterangan, tipe, data, geom,
                       speed::int8 AS speed, detik::int8 AS detik, urut, replay, autoplay,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(username)
        .bind(keterangan)
        .bind(data)
        .bind(geom)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("anim not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_anim_spawn_name(
        &self,
        id: Uuid,
        name: &str,
        username: &str,
        keterangan: &str,
    ) -> Result<Animation, WriteError> {
        let row = sqlx::query_as::<_, AnimRow>(
            "UPDATE anims SET name = $2, username = $3, keterangan = $4, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, username, keterangan, tipe, data, geom,
                       speed::int8 AS speed, detik::int8 AS detik, urut, replay, autoplay,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(username)
        .bind(keterangan)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("anim not found"))?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn delete_anim(&self, id: Uuid) -> Result<(), WriteError> {
        let result = sqlx::query("DELETE FROM anims WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("anim not found"));
        }
        Ok(())
    }

    pub async fn update_anim_tipe(&self, id: Uuid, tipe: &str) -> Result<(), WriteError> {
        let result = sqlx::query("UPDATE anims SET tipe = $2, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(tipe)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WriteError::NotFound("anim not found"));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_spawn_anim(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        unit_id: Uuid,
        marker_name: &str,
        kategori: &str,
        rot_x: f64,
        rot_y: f64,
        rot_z: f64,
        pos_x: f64,
        pos_y: f64,
        scale: f64,
        altitude: f64,
        name: &str,
        username: &str,
        keterangan: &str,
    ) -> Result<Animation, WriteError> {
        let mut transaction = self.pool.begin().await?;
        let marker = sqlx::query_as::<_, SpawnMarkerRow>(
            "INSERT INTO markers (
                 name, operasi_id, skenario_id, unit_id, jumlah, rot_x, rot_y, rot_z,
                 pos_x, pos_y, keterangan, kategori, scale, is_spawn
             ) VALUES ($1, $2, $3, $4, 1, $5, $6, $7, $8, $9, '', $10, $11, 1)
             RETURNING id, skenario_id, name, unit_id, kategori, rot_x, rot_y, rot_z,
                       pos_x, pos_y, scale, scale_y, scale_z",
        )
        .bind(marker_name)
        .bind(operasi_id)
        .bind(skenario_id)
        .bind(unit_id)
        .bind(rot_x)
        .bind(rot_y)
        .bind(rot_z)
        .bind(pos_x)
        .bind(pos_y)
        .bind(kategori)
        .bind(scale)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| map_write_error(error, "marker already exists"))?;

        if altitude != 0.0 {
            sqlx::query("UPDATE markers SET altitude = $2 WHERE id = $1")
                .bind(marker.id)
                .bind(altitude)
                .execute(&mut *transaction)
                .await?;
        }

        let data = SpawnData {
            marker_id: marker.id.to_string(),
            name: marker.name.clone().unwrap_or_default(),
            rot_x: marker.rot_x.unwrap_or(0.0),
            rot_y: marker.rot_y.unwrap_or(0.0),
            rot_z: marker.rot_z.unwrap_or(0.0),
            pos_x: marker.pos_x.unwrap_or(0.0),
            pos_y: marker.pos_y.unwrap_or(0.0),
            scale: marker.scale.unwrap_or(1.0),
            scale_y: marker.scale.unwrap_or(1.0),
            scale_z: marker.scale.unwrap_or(1.0),
            altitude,
            unit_id: marker.unit_id.map(|id| id.to_string()).unwrap_or_default(),
            kategori: marker.kategori.clone().unwrap_or_default(),
        };
        let data_json = go_json_string(&vec![data])?;

        let row = insert_anim(
            &mut transaction,
            operasi_id,
            skenario_id,
            name,
            username,
            keterangan,
            "spawn",
            &data_json,
            "[]",
        )
        .await?;
        transaction.commit().await?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn create_spawn_pickup_anim(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        marker_ids: &[Uuid],
        name: &str,
        username: &str,
        keterangan: &str,
    ) -> Result<Animation, WriteError> {
        let mut transaction = self.pool.begin().await?;
        let mut unique = Vec::new();
        for marker_id in marker_ids {
            if !unique.contains(marker_id) {
                unique.push(*marker_id);
            }
        }
        if unique.is_empty() {
            return Err(WriteError::Invalid(
                "marker id must not be empty".to_owned(),
            ));
        }

        let mut datas = Vec::with_capacity(unique.len());
        for marker_id in unique {
            let marker = sqlx::query_as::<_, SpawnMarkerRow>(
                "SELECT id, skenario_id, name, unit_id, kategori, rot_x, rot_y, rot_z,
                        pos_x, pos_y, scale, scale_y, scale_z
                 FROM markers WHERE id = $1",
            )
            .bind(marker_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(WriteError::NotFound("marker not found"))?;
            let is_spawn: Option<i32> =
                sqlx::query_scalar("SELECT is_spawn FROM markers WHERE id = $1")
                    .bind(marker_id)
                    .fetch_one(&mut *transaction)
                    .await?;
            if is_spawn.unwrap_or(0) == 1 {
                return Err(WriteError::Conflict(
                    "marker already assigned to spawn".to_owned(),
                ));
            }
            sqlx::query("UPDATE markers SET is_spawn = 1, updated_at = NOW() WHERE id = $1")
                .bind(marker_id)
                .execute(&mut *transaction)
                .await?;
            datas.push(SpawnData {
                marker_id: marker.id.to_string(),
                name: marker.name.clone().unwrap_or_default(),
                rot_x: marker.rot_x.unwrap_or(0.0),
                rot_y: marker.rot_y.unwrap_or(0.0),
                rot_z: marker.rot_z.unwrap_or(0.0),
                pos_x: marker.pos_x.unwrap_or(0.0),
                pos_y: marker.pos_y.unwrap_or(0.0),
                scale: marker.scale.unwrap_or(1.0),
                scale_y: marker.scale_y.unwrap_or(0.0),
                scale_z: marker.scale_z.unwrap_or(0.0),
                altitude: 0.0,
                unit_id: marker.unit_id.map(|id| id.to_string()).unwrap_or_default(),
                kategori: marker.kategori.clone().unwrap_or_default(),
            });
        }

        let data_json = go_json_string(&datas)?;
        let row = insert_anim(
            &mut transaction,
            operasi_id,
            skenario_id,
            name,
            username,
            keterangan,
            "spawn",
            &data_json,
            "[]",
        )
        .await?;
        transaction.commit().await?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_shoot_anim(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        marker_id: Uuid,
        name: &str,
        username: &str,
        keterangan: &str,
        data: &str,
    ) -> Result<Animation, WriteError> {
        let new_shoot: ShootData = serde_json::from_str(data)
            .map_err(|_| WriteError::Invalid("invalid shoot data".to_owned()))?;
        let mut transaction = self.pool.begin().await?;
        let marker = sqlx::query_as::<_, MarkerShootRow>(
            "SELECT id, skenario_id, shoot FROM markers WHERE id = $1",
        )
        .bind(marker_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(WriteError::NotFound("marker not found"))?;

        let mut shoots = parse_shoot_list(marker.shoot.as_deref().unwrap_or(""))?;
        if shoots.iter().any(|item| item.tipe == new_shoot.tipe) {
            return Err(WriteError::Conflict(format!(
                "tipe {} shoot sudah ada",
                new_shoot.tipe
            )));
        }
        shoots.push(new_shoot);
        let shoots_json = go_json_string(&shoots)?;
        sqlx::query("UPDATE markers SET shoot = $2, updated_at = NOW() WHERE id = $1")
            .bind(marker_id)
            .bind(shoots_json)
            .execute(&mut *transaction)
            .await?;

        let row = insert_anim(
            &mut transaction,
            operasi_id,
            skenario_id,
            name,
            username,
            keterangan,
            "shoot",
            data,
            "[]",
        )
        .await?;
        transaction.commit().await?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_combine_anim(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        name: &str,
        username: &str,
        keterangan: &str,
        data: &str,
        tipe: &str,
    ) -> Result<Animation, WriteError> {
        let items = parse_combine_items(data)?;
        let (parent_type, child_type) = normalize_group_type(tipe);
        let mut transaction = self.pool.begin().await?;
        for item in &items {
            sqlx::query("UPDATE anims SET tipe = $2 WHERE id = $1")
                .bind(item.id)
                .bind(child_type)
                .execute(&mut *transaction)
                .await?;
        }
        let row = insert_anim(
            &mut transaction,
            operasi_id,
            skenario_id,
            name,
            username,
            keterangan,
            parent_type,
            data,
            "[]",
        )
        .await?;
        transaction.commit().await?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn delete_combine_anim(&self, id: Uuid) -> Result<(), WriteError> {
        let mut transaction = self.pool.begin().await?;
        let anim = get_anim_lookup(&mut transaction, id).await?;
        let items = parse_combine_items(anim.data.as_deref().unwrap_or(""))?;
        for item in items {
            let next_type = if is_hidden_group_type(&item.tipe) {
                match get_anim_lookup(&mut transaction, item.id).await {
                    Ok(child) => infer_anim_type_from_record(
                        child.data.as_deref().unwrap_or(""),
                        child.geom.as_deref().unwrap_or(""),
                    ),
                    Err(WriteError::NotFound(_)) => "remove".to_owned(),
                    Err(error) => return Err(error),
                }
            } else {
                item.tipe.clone()
            };
            sqlx::query("UPDATE anims SET tipe = $2 WHERE id = $1")
                .bind(item.id)
                .bind(next_type)
                .execute(&mut *transaction)
                .await?;
        }
        delete_anim_row(&mut transaction, id).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn update_combine_data(
        &self,
        id: Uuid,
        name: &str,
        username: &str,
        keterangan: &str,
        data: &str,
    ) -> Result<Animation, WriteError> {
        let mut transaction = self.pool.begin().await?;
        let parent = get_anim_lookup(&mut transaction, id).await?;
        let old_items = parse_combine_items_loose(parent.data.as_deref().unwrap_or(""));
        let new_items = parse_combine_items_loose(data);
        let (_, hidden_type) = normalize_group_type(parent.tipe.as_deref().unwrap_or(""));

        let new_ids = new_items.iter().map(|item| item.id).collect::<Vec<_>>();
        for item in &new_items {
            sqlx::query("UPDATE anims SET tipe = $2 WHERE id = $1")
                .bind(item.id)
                .bind(hidden_type)
                .execute(&mut *transaction)
                .await?;
        }
        for old in old_items {
            if new_ids.contains(&old.id) {
                continue;
            }
            let next_type = match get_anim_lookup(&mut transaction, old.id).await {
                Ok(child) => infer_anim_type_from_record(
                    child.data.as_deref().unwrap_or(""),
                    child.geom.as_deref().unwrap_or(""),
                ),
                Err(WriteError::NotFound(_)) => {
                    if is_hidden_group_type(&old.tipe) {
                        "remove".to_owned()
                    } else {
                        old.tipe.clone()
                    }
                }
                Err(error) => return Err(error),
            };
            sqlx::query("UPDATE anims SET tipe = $2 WHERE id = $1")
                .bind(old.id)
                .bind(next_type)
                .execute(&mut *transaction)
                .await?;
        }

        let row = sqlx::query_as::<_, AnimRow>(
            "UPDATE anims
             SET name = $2, username = $3, keterangan = $4, data = $5, updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, username, keterangan, tipe, data, geom,
                       speed::int8 AS speed, detik::int8 AS detik, urut, replay, autoplay,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(username)
        .bind(keterangan)
        .bind(data)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(WriteError::NotFound("anim not found"))?;
        transaction.commit().await?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_shoot_data(
        &self,
        id: Uuid,
        name: &str,
        username: &str,
        keterangan: &str,
        data: &str,
    ) -> Result<Animation, WriteError> {
        let mut transaction = self.pool.begin().await?;
        let anim = get_anim_lookup(&mut transaction, id).await?;
        let old_shoot: ShootData = serde_json::from_str(anim.data.as_deref().unwrap_or(""))
            .map_err(|error| WriteError::Invalid(format!("invalid shoot data: {error}")))?;
        let new_shoot: ShootData = serde_json::from_str(data)
            .map_err(|_| WriteError::Invalid("invalid shoot data".to_owned()))?;
        if old_shoot.marker_id.is_empty() || new_shoot.marker_id.is_empty() {
            return Err(WriteError::Invalid("marker id is required".to_owned()));
        }
        let old_marker_id = Uuid::parse_str(&old_shoot.marker_id)
            .map_err(|error| WriteError::Invalid(format!("invalid marker id: {error}")))?;
        let new_marker_id = Uuid::parse_str(&new_shoot.marker_id)
            .map_err(|error| WriteError::Invalid(format!("invalid marker id: {error}")))?;

        if old_marker_id != new_marker_id {
            let old_marker = sqlx::query_as::<_, MarkerShootRow>(
                "SELECT id, skenario_id, shoot FROM markers WHERE id = $1",
            )
            .bind(old_marker_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(WriteError::NotFound("marker not found"))?;
            let shoots = parse_shoot_list(old_marker.shoot.as_deref().unwrap_or(""))?;
            let filtered = shoots
                .into_iter()
                .filter(|item| item.tipe != old_shoot.tipe)
                .collect::<Vec<_>>();
            let value = if filtered.is_empty() {
                String::new()
            } else {
                go_json_string(&filtered)?
            };
            sqlx::query("UPDATE markers SET shoot = $2, updated_at = NOW() WHERE id = $1")
                .bind(old_marker_id)
                .bind(value)
                .execute(&mut *transaction)
                .await?;
        }

        let new_marker = sqlx::query_as::<_, MarkerShootRow>(
            "SELECT id, skenario_id, shoot FROM markers WHERE id = $1",
        )
        .bind(new_marker_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(WriteError::NotFound("marker not found"))?;
        let shoots = parse_shoot_list(new_marker.shoot.as_deref().unwrap_or(""))?;
        let mut filtered = Vec::with_capacity(shoots.len() + 1);
        for row_shoot in shoots {
            if old_marker_id == new_marker_id && row_shoot.tipe == old_shoot.tipe {
                continue;
            }
            if old_marker_id != new_marker_id && row_shoot.tipe == new_shoot.tipe {
                return Err(WriteError::Conflict(format!(
                    "tipe {} shoot sudah ada",
                    row_shoot.tipe
                )));
            }
            filtered.push(row_shoot);
        }
        if filtered
            .iter()
            .any(|row_shoot| row_shoot.tipe == new_shoot.tipe)
        {
            return Err(WriteError::Conflict(format!(
                "tipe {} shoot sudah ada",
                new_shoot.tipe
            )));
        }
        filtered.push(new_shoot);
        let value = go_json_string(&filtered)?;
        sqlx::query("UPDATE markers SET shoot = $2, updated_at = NOW() WHERE id = $1")
            .bind(new_marker_id)
            .bind(value)
            .execute(&mut *transaction)
            .await?;

        let row = sqlx::query_as::<_, AnimRow>(
            "UPDATE anims
             SET name = $2, username = $3, keterangan = $4, data = $5, geom = '[]',
                 updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, username, keterangan, tipe, data, geom,
                       speed::int8 AS speed, detik::int8 AS detik, urut, replay, autoplay,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(username)
        .bind(keterangan)
        .bind(data)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(WriteError::NotFound("anim not found"))?;
        transaction.commit().await?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_spawn_data(
        &self,
        id: Uuid,
        name: &str,
        username: &str,
        keterangan: &str,
        data: &str,
    ) -> Result<Animation, WriteError> {
        let mut transaction = self.pool.begin().await?;
        let anim = get_anim_lookup(&mut transaction, id).await?;
        let old_datas = parse_spawn_data_list(anim.data.as_deref().unwrap_or(""))?;
        let new_datas = parse_spawn_data_list(data)?;

        let old_ids = old_datas
            .iter()
            .filter(|item| !item.marker_id.is_empty())
            .map(|item| item.marker_id.clone())
            .collect::<Vec<_>>();
        let new_ids = new_datas
            .iter()
            .filter(|item| !item.marker_id.is_empty())
            .map(|item| item.marker_id.clone())
            .collect::<Vec<_>>();

        for marker_id in &old_ids {
            if new_ids.contains(marker_id) {
                continue;
            }
            let uuid = Uuid::parse_str(marker_id)
                .map_err(|error| WriteError::Invalid(format!("invalid marker id: {error}")))?;
            update_marker_spawn_flag(&mut transaction, uuid, 0).await?;
        }
        for marker_id in &new_ids {
            if old_ids.contains(marker_id) {
                continue;
            }
            let uuid = Uuid::parse_str(marker_id)
                .map_err(|error| WriteError::Invalid(format!("invalid marker id: {error}")))?;
            let is_spawn: Option<i32> =
                sqlx::query_scalar("SELECT is_spawn FROM markers WHERE id = $1")
                    .bind(uuid)
                    .fetch_optional(&mut *transaction)
                    .await?
                    .ok_or(WriteError::NotFound("marker not found"))?;
            if is_spawn.unwrap_or(0) == 1 {
                return Err(WriteError::Conflict(
                    "marker already assigned to spawn".to_owned(),
                ));
            }
            update_marker_spawn_flag(&mut transaction, uuid, 1).await?;
        }

        let row = sqlx::query_as::<_, AnimRow>(
            "UPDATE anims
             SET name = $2, username = $3, keterangan = $4, data = $5, geom = '[]',
                 updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, username, keterangan, tipe, data, geom,
                       speed::int8 AS speed, detik::int8 AS detik, urut, replay, autoplay,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(username)
        .bind(keterangan)
        .bind(data)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(WriteError::NotFound("anim not found"))?;
        transaction.commit().await?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn delete_spawn_anim(&self, id: Uuid) -> Result<(), WriteError> {
        let mut transaction = self.pool.begin().await?;
        let anim = get_anim_lookup(&mut transaction, id).await?;
        for data in parse_spawn_data_list(anim.data.as_deref().unwrap_or(""))? {
            let marker_id = Uuid::parse_str(&data.marker_id)
                .map_err(|error| WriteError::Invalid(format!("invalid marker id: {error}")))?;
            update_marker_spawn_flag(&mut transaction, marker_id, 0).await?;
        }
        delete_anim_row(&mut transaction, id).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn delete_shoot_anim(&self, id: Uuid) -> Result<(), WriteError> {
        let mut transaction = self.pool.begin().await?;
        let anim = get_anim_lookup(&mut transaction, id).await?;
        let shoot: ShootData = serde_json::from_str(anim.data.as_deref().unwrap_or(""))
            .map_err(|error| WriteError::Invalid(format!("invalid shoot data: {error}")))?;
        let marker_id = Uuid::parse_str(&shoot.marker_id)
            .map_err(|error| WriteError::Invalid(format!("invalid marker id: {error}")))?;
        let marker = sqlx::query_as::<_, MarkerShootRow>(
            "SELECT id, skenario_id, shoot FROM markers WHERE id = $1",
        )
        .bind(marker_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(WriteError::NotFound("marker not found"))?;
        let shoots = parse_shoot_list(marker.shoot.as_deref().unwrap_or(""))?;
        let filtered = shoots
            .into_iter()
            .filter(|item| item.tipe != shoot.tipe)
            .collect::<Vec<_>>();
        let value = if filtered.is_empty() {
            String::new()
        } else {
            go_json_string(&filtered)?
        };
        sqlx::query("UPDATE markers SET shoot = $2, updated_at = NOW() WHERE id = $1")
            .bind(marker_id)
            .bind(value)
            .execute(&mut *transaction)
            .await?;
        delete_anim_row(&mut transaction, id).await?;
        transaction.commit().await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_particle_anim(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        name: &str,
        username: &str,
        keterangan: &str,
        tipe: &str,
        data: &str,
        pos_x: f64,
        pos_y: f64,
        is_spawn: i32,
    ) -> Result<Animation, WriteError> {
        let mut transaction = self.pool.begin().await?;
        let particle = sqlx::query_as::<_, ParticleRow>(
            "INSERT INTO particles (tipe, operasi_id, skenario_id, name, data, pos_x, pos_y, is_spawn)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id, skenario_id, tipe, name, data, pos_x, pos_y, altitude, is_spawn,
                       created_at, updated_at",
        )
        .bind(tipe)
        .bind(operasi_id)
        .bind(skenario_id)
        .bind(name)
        .bind(data)
        .bind(pos_x)
        .bind(pos_y)
        .bind(is_spawn)
        .fetch_one(&mut *transaction)
        .await?;
        let spawn_data = ParticleSpawnData {
            particle_id: particle.id.to_string(),
            tipe: particle.tipe.clone().unwrap_or_default(),
            pos_x: particle.pos_x.unwrap_or(0.0),
            pos_y: particle.pos_y.unwrap_or(0.0),
            is_spawn,
            name: name.to_owned(),
        };
        let data_json = go_json_string(&spawn_data)?;
        let row = insert_anim(
            &mut transaction,
            operasi_id,
            skenario_id,
            name,
            username,
            keterangan,
            "spawn_particle",
            &data_json,
            "[]",
        )
        .await?;
        transaction.commit().await?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn update_particle_data(
        &self,
        id: Uuid,
        name: &str,
        username: &str,
        keterangan: &str,
        tipe: &str,
        data: &str,
    ) -> Result<Animation, WriteError> {
        let mut transaction = self.pool.begin().await?;
        let anim = get_anim_lookup(&mut transaction, id).await?;
        let payload: ParticleSpawnData =
            serde_json::from_str(anim.data.as_deref().unwrap_or(""))
                .map_err(|error| WriteError::Invalid(format!("invalid particle data: {error}")))?;
        let particle_id = Uuid::parse_str(&payload.particle_id)
            .map_err(|error| WriteError::Invalid(format!("invalid particle id: {error}")))?;
        let updated = sqlx::query(
            "UPDATE particles SET name = $2, data = $3, tipe = $4, updated_at = NOW()
             WHERE id = $1",
        )
        .bind(particle_id)
        .bind(name)
        .bind(data)
        .bind(tipe)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(WriteError::NotFound("particle not found"));
        }
        let new_data = ParticleSpawnData {
            particle_id: payload.particle_id,
            tipe: tipe.to_owned(),
            pos_x: payload.pos_x,
            pos_y: payload.pos_y,
            is_spawn: payload.is_spawn,
            name: name.to_owned(),
        };
        let data_json = go_json_string(&new_data)?;
        let row = sqlx::query_as::<_, AnimRow>(
            "UPDATE anims
             SET name = $2, username = $3, keterangan = $4, data = $5, geom = '[]',
                 updated_at = NOW()
             WHERE id = $1
             RETURNING id, skenario_id, name, username, keterangan, tipe, data, geom,
                       speed::int8 AS speed, detik::int8 AS detik, urut, replay, autoplay,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(username)
        .bind(keterangan)
        .bind(data_json)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(WriteError::NotFound("anim not found"))?;
        transaction.commit().await?;
        let context = self.context(row.skenario_id).await?;
        Ok(row.into_contract(&context))
    }

    pub async fn delete_particle_anim(&self, id: Uuid) -> Result<(), WriteError> {
        let mut transaction = self.pool.begin().await?;
        let anim = get_anim_lookup(&mut transaction, id).await?;
        if anim.tipe.as_deref() == Some("spawn_particle") {
            let payload: ParticleSpawnData =
                serde_json::from_str(anim.data.as_deref().unwrap_or("")).map_err(|error| {
                    WriteError::Invalid(format!("invalid particle data: {error}"))
                })?;
            let particle_id = Uuid::parse_str(&payload.particle_id)
                .map_err(|error| WriteError::Invalid(format!("invalid particle id: {error}")))?;
            sqlx::query("DELETE FROM particles WHERE id = $1")
                .bind(particle_id)
                .execute(&mut *transaction)
                .await?;
        }
        delete_anim_row(&mut transaction, id).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn update_anim_order(&self, items: &[AnimOrder]) -> Result<(), WriteError> {
        let mut transaction = self.pool.begin().await?;
        for item in items {
            let result =
                sqlx::query("UPDATE anims SET urut = $2, updated_at = NOW() WHERE id = $1")
                    .bind(item.id)
                    .bind(item.urut)
                    .execute(&mut *transaction)
                    .await?;
            if result.rows_affected() == 0 {
                return Err(WriteError::NotFound("anim not found"));
            }
        }
        transaction.commit().await?;
        Ok(())
    }

    // ---------------------------------------------------------------- snapshots

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_snapshot(
        &self,
        operasi_id: Uuid,
        skenario_id: Uuid,
        enabled: bool,
        map_state: Value,
        markers_state: Value,
        anims_state: Value,
        updated_by: Uuid,
    ) -> Result<Snapshot, WriteError> {
        let row = sqlx::query_as::<_, SnapshotRow>(
            "INSERT INTO game_session_snapshots (
                 operasi_id, skenario_id, snapshot_enabled, map_state, markers_state,
                 anims_state, updated_by, version
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, 1)
             ON CONFLICT (skenario_id) DO UPDATE SET
                 operasi_id = EXCLUDED.operasi_id,
                 snapshot_enabled = EXCLUDED.snapshot_enabled,
                 map_state = EXCLUDED.map_state,
                 markers_state = EXCLUDED.markers_state,
                 anims_state = EXCLUDED.anims_state,
                 updated_by = EXCLUDED.updated_by,
                 version = game_session_snapshots.version + 1,
                 updated_at = NOW()
             RETURNING id, operasi_id, skenario_id, snapshot_enabled, map_state,
                       markers_state, anims_state, updated_by, updated_at, version",
        )
        .bind(operasi_id)
        .bind(skenario_id)
        .bind(enabled)
        .bind(map_state)
        .bind(markers_state)
        .bind(anims_state)
        .bind(updated_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_write_error(error, "invalid snapshot"))?;
        Ok(row.into_contract())
    }

    pub async fn toggle_snapshot(
        &self,
        skenario_id: Uuid,
        enabled: bool,
    ) -> Result<Snapshot, WriteError> {
        let row = sqlx::query_as::<_, SnapshotRow>(
            "UPDATE game_session_snapshots
             SET snapshot_enabled = $2, updated_at = NOW()
             WHERE skenario_id = $1
             RETURNING id, operasi_id, skenario_id, snapshot_enabled, map_state,
                       markers_state, anims_state, updated_by, updated_at, version",
        )
        .bind(skenario_id)
        .bind(enabled)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WriteError::NotFound("snapshot not found, save first"))?;
        Ok(row.into_contract())
    }

    pub async fn delete_snapshot(&self, skenario_id: Uuid) -> Result<(), WriteError> {
        sqlx::query("DELETE FROM game_session_snapshots WHERE skenario_id = $1")
            .bind(skenario_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---------------------------------------------------------------- commands

    #[allow(clippy::too_many_arguments)]
    pub async fn save_command(
        &self,
        is_save: i32,
        operasi_id: Uuid,
        skenario_id: Uuid,
        marker_id: Uuid,
        command: &str,
        data: &str,
    ) -> Result<(), WriteError> {
        if is_save != 1 {
            return Ok(());
        }
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO commands (operasi_id, skenario_id, marker_id, command, data)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(operasi_id)
        .bind(skenario_id)
        .bind(marker_id)
        .bind(command)
        .bind(data)
        .execute(&mut *transaction)
        .await?;

        match command {
            "ZOOM" => {
                let zoom: ZoomData = serde_json::from_str(data)
                    .map_err(|error| WriteError::Invalid(format!("invalid zoom data: {error}")))?;
                let existing: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM last_maps WHERE skenario_id = $1 ORDER BY created_at DESC LIMIT 1",
                )
                .bind(skenario_id)
                .fetch_optional(&mut *transaction)
                .await?;
                if existing.is_some() {
                    sqlx::query(
                        "UPDATE last_maps
                         SET center_x = $2, center_y = $3, zoom = $4, pitch = $5,
                             bearing = $6, updated_at = NOW()
                         WHERE skenario_id = $1",
                    )
                    .bind(skenario_id)
                    .bind(zoom.center.x)
                    .bind(zoom.center.y)
                    .bind(zoom.zoom)
                    .bind(zoom.pitch)
                    .bind(zoom.bearing)
                    .execute(&mut *transaction)
                    .await?;
                } else {
                    sqlx::query(
                        "INSERT INTO last_maps (
                             operasi_id, skenario_id, center_x, center_y, zoom, pitch, bearing
                         ) VALUES ($1, $2, $3, $4, $5, $6, $7)",
                    )
                    .bind(operasi_id)
                    .bind(skenario_id)
                    .bind(zoom.center.x)
                    .bind(zoom.center.y)
                    .bind(zoom.zoom)
                    .bind(zoom.pitch)
                    .bind(zoom.bearing)
                    .execute(&mut *transaction)
                    .await?;
                }
            }
            "MOVE" | "ROT" => {
                let existing: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM last_positions WHERE marker_id = $1
                     ORDER BY created_at DESC LIMIT 1",
                )
                .bind(marker_id)
                .fetch_optional(&mut *transaction)
                .await?;
                if command == "MOVE" {
                    let value: MoveData = serde_json::from_str(data).map_err(|error| {
                        WriteError::Invalid(format!("invalid move data: {error}"))
                    })?;
                    if existing.is_some() {
                        sqlx::query(
                            "UPDATE last_positions
                             SET pos_x = $2, pos_y = $3, updated_at = NOW()
                             WHERE marker_id = $1",
                        )
                        .bind(marker_id)
                        .bind(value.x)
                        .bind(value.y)
                        .execute(&mut *transaction)
                        .await?;
                    } else {
                        let marker = marker_pose(&mut transaction, marker_id).await?;
                        sqlx::query(
                            "INSERT INTO last_positions (
                                 marker_id, pos_x, pos_y, rot_x, rot_y, rot_z, operasi_id,
                                 skenario_id
                             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                        )
                        .bind(marker_id)
                        .bind(value.x)
                        .bind(value.y)
                        .bind(marker.rot_x)
                        .bind(marker.rot_y)
                        .bind(marker.rot_z)
                        .bind(operasi_id)
                        .bind(skenario_id)
                        .execute(&mut *transaction)
                        .await?;
                    }
                } else {
                    let value: RotData = serde_json::from_str(data).map_err(|error| {
                        WriteError::Invalid(format!("invalid rot data: {error}"))
                    })?;
                    if existing.is_some() {
                        sqlx::query(
                            "UPDATE last_positions
                             SET rot_x = $2, rot_y = $3, rot_z = $4, updated_at = NOW()
                             WHERE marker_id = $1",
                        )
                        .bind(marker_id)
                        .bind(value.x)
                        .bind(value.y)
                        .bind(value.z)
                        .execute(&mut *transaction)
                        .await?;
                    } else {
                        let marker = marker_pose(&mut transaction, marker_id).await?;
                        sqlx::query(
                            "INSERT INTO last_positions (
                                 marker_id, pos_x, pos_y, rot_x, rot_y, rot_z, operasi_id,
                                 skenario_id
                             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                        )
                        .bind(marker_id)
                        .bind(marker.pos_x)
                        .bind(marker.pos_y)
                        .bind(value.x)
                        .bind(value.y)
                        .bind(value.z)
                        .bind(operasi_id)
                        .bind(skenario_id)
                        .execute(&mut *transaction)
                        .await?;
                    }
                }
            }
            _ => {}
        }

        transaction.commit().await?;
        Ok(())
    }

    /// Returns the last map and positions for a scenario. Used after commands so
    /// the write path can expose the same contract as the read path.
    pub async fn last_positions(
        &self,
        skenario_id: Uuid,
    ) -> Result<twg_contracts::last_positions::LastPositionMap, WriteError> {
        Ok(self.read.last_positions(skenario_id).await?)
    }
}

#[allow(clippy::too_many_arguments)]
async fn insert_anim(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    operasi_id: Uuid,
    skenario_id: Uuid,
    name: &str,
    username: &str,
    keterangan: &str,
    tipe: &str,
    data: &str,
    geom: &str,
) -> Result<AnimRow, WriteError> {
    let row = sqlx::query_as::<_, AnimRow>(
        "INSERT INTO anims (
             operasi_id, skenario_id, name, username, keterangan, tipe, data, geom,
             speed, detik
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 0, 0)
         RETURNING id, skenario_id, name, username, keterangan, tipe, data, geom,
                   speed::int8 AS speed, detik::int8 AS detik, urut, replay, autoplay,
                   created_at, updated_at",
    )
    .bind(operasi_id)
    .bind(skenario_id)
    .bind(name)
    .bind(username)
    .bind(keterangan)
    .bind(tipe)
    .bind(data)
    .bind(geom)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|error| map_write_error(error, "anim already exists"))?;
    Ok(row)
}

async fn get_anim_lookup(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
) -> Result<AnimLookupRow, WriteError> {
    sqlx::query_as::<_, AnimLookupRow>(
        "SELECT id, skenario_id, tipe, data, geom FROM anims WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(WriteError::NotFound("anim not found"))
}

async fn delete_anim_row(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
) -> Result<(), WriteError> {
    let result = sqlx::query("DELETE FROM anims WHERE id = $1")
        .bind(id)
        .execute(&mut **transaction)
        .await?;
    if result.rows_affected() == 0 {
        return Err(WriteError::NotFound("anim not found"));
    }
    Ok(())
}

async fn update_marker_spawn_flag(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    marker_id: Uuid,
    is_spawn: i32,
) -> Result<(), WriteError> {
    let result = sqlx::query("UPDATE markers SET is_spawn = $2, updated_at = NOW() WHERE id = $1")
        .bind(marker_id)
        .bind(is_spawn)
        .execute(&mut **transaction)
        .await?;
    if result.rows_affected() == 0 {
        return Err(WriteError::NotFound("marker not found"));
    }
    Ok(())
}

async fn marker_pose(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    marker_id: Uuid,
) -> Result<MarkerPoseRow, WriteError> {
    sqlx::query_as::<_, MarkerPoseRow>(
        "SELECT pos_x, pos_y, rot_x, rot_y, rot_z FROM markers WHERE id = $1",
    )
    .bind(marker_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(WriteError::NotFound("marker not found"))
}

fn parse_combine_items(data: &str) -> Result<Vec<CombineItem>, WriteError> {
    if let Ok(items) = serde_json::from_str::<Vec<CombineItem>>(data)
        && !items.is_empty()
    {
        return Ok(items);
    }
    let payload: CombinePayload = serde_json::from_str(data)
        .map_err(|_| WriteError::Invalid("invalid combine data".to_owned()))?;
    if !payload.items.is_empty() {
        return Ok(payload.items);
    }
    if !payload.data.is_empty() {
        return Ok(payload.data);
    }
    if !payload.anims.is_empty() {
        return Ok(payload.anims);
    }
    Ok(Vec::new())
}

fn parse_combine_items_loose(data: &str) -> Vec<CombineItem> {
    #[derive(Deserialize)]
    struct LooseItem {
        id: String,
        #[serde(default)]
        tipe: String,
    }
    #[derive(Deserialize)]
    struct LoosePayload {
        #[serde(default)]
        items: Vec<LooseItem>,
        #[serde(default)]
        data: Vec<LooseItem>,
        #[serde(default)]
        anims: Vec<LooseItem>,
    }

    fn convert(items: Vec<LooseItem>) -> Vec<CombineItem> {
        items
            .into_iter()
            .filter_map(|item| {
                let id = Uuid::parse_str(item.id.trim()).ok()?;
                Some(CombineItem {
                    id,
                    tipe: item.tipe,
                })
            })
            .collect()
    }

    if let Ok(items) = serde_json::from_str::<Vec<LooseItem>>(data) {
        convert(items)
    } else if let Ok(payload) = serde_json::from_str::<LoosePayload>(data) {
        if !payload.items.is_empty() {
            convert(payload.items)
        } else if !payload.data.is_empty() {
            convert(payload.data)
        } else if !payload.anims.is_empty() {
            convert(payload.anims)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    }
}

fn normalize_group_type(raw: &str) -> (&'static str, &'static str) {
    if raw.trim().eq_ignore_ascii_case("sequence") {
        ("sequence", "is_sequence")
    } else {
        ("combine", "is_combine")
    }
}

fn is_hidden_group_type(tipe: &str) -> bool {
    tipe == "is_sequence" || tipe == "is_combine"
}

fn infer_route_type_from_geom(geom: &str) -> String {
    let trimmed = geom.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return "route".to_owned();
    }
    let Ok(points) = serde_json::from_str::<Vec<Value>>(trimmed) else {
        return "route".to_owned();
    };
    for point in points {
        if let Some(z) = point.get("z").and_then(Value::as_f64)
            && z != 0.0
        {
            return "route_3d".to_owned();
        }
    }
    "route".to_owned()
}

fn infer_anim_type_from_record(data: &str, geom: &str) -> String {
    let trimmed = data.trim();
    if !trimmed.is_empty() {
        if let Ok(Value::Object(object)) = serde_json::from_str::<Value>(trimmed) {
            let has = |key: &str| object.contains_key(key);
            if has("seconds") || has("second") || has("detik") || has("ms") {
                return "delay".to_owned();
            }
            if has("particle_id") {
                return "spawn_particle".to_owned();
            }
            if has("route_anim_id") && (has("rot_x") || has("scale_x") || has("duration_ms")) {
                return "route_transform".to_owned();
            }
            if has("marker_id") {
                if has("tipe") {
                    return "shoot".to_owned();
                }
                if has("rot_x") || has("scale_x") || has("duration_ms") {
                    return "rot_scale".to_owned();
                }
            }
        }
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(trimmed)
            && let Some(Value::Object(first)) = items.first()
        {
            let has = |key: &str| first.contains_key(key);
            if has("particle_id") {
                return "remove_particle".to_owned();
            }
            if has("unit_id") || has("pos_x") || has("pos_y") {
                return "spawn".to_owned();
            }
            if has("offsetX") || has("offsetY") || has("mode") || has("jarak") {
                return infer_route_type_from_geom(geom);
            }
            if has("marker_id") {
                return "remove".to_owned();
            }
        }
    }
    if !geom.trim().is_empty() && geom.trim() != "[]" {
        return infer_route_type_from_geom(geom);
    }
    "remove".to_owned()
}

fn parse_spawn_data_list(data: &str) -> Result<Vec<SpawnData>, WriteError> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if let Ok(items) = serde_json::from_str::<Vec<SpawnData>>(data) {
        return Ok(items);
    }
    let single: SpawnData = serde_json::from_str(data)
        .map_err(|_| WriteError::Invalid("invalid spawn data".to_owned()))?;
    Ok(vec![single])
}

fn parse_shoot_list(data: &str) -> Result<Vec<ShootData>, WriteError> {
    if data.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(data)
        .map_err(|error| WriteError::Invalid(format!("invalid shoot data: {error}")))
}

#[derive(Debug, FromRow)]
struct MarkerRow {
    id: Uuid,
    skenario_id: Uuid,
    name: Option<String>,
    unit_id: Option<Uuid>,
    kategori: Option<String>,
    jumlah: Option<i32>,
    rot_x: Option<f64>,
    rot_y: Option<f64>,
    rot_z: Option<f64>,
    pos_x: Option<f64>,
    pos_y: Option<f64>,
    scale: Option<f64>,
    scale_y: Option<f64>,
    scale_z: Option<f64>,
    altitude: Option<f64>,
    is_spawn: Option<i32>,
    obstacle: Option<bool>,
    keterangan: Option<String>,
    shoot: Option<String>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl MarkerRow {
    fn into_contract(self, context: &ScenarioContext) -> Marker {
        let scale = self.scale.unwrap_or(1.0);
        let scale_y = self.scale_y.filter(|value| *value != 0.0).unwrap_or(scale);
        let scale_z = self.scale_z.filter(|value| *value != 0.0).unwrap_or(scale);
        Marker {
            id: self.id,
            name: self.name.unwrap_or_default(),
            operasi: context.operation.clone(),
            skenario: context.scenario.clone(),
            unit_id: self.unit_id.map(|id| id.to_string()).unwrap_or_default(),
            kategori: self.kategori.unwrap_or_default(),
            jumlah: self.jumlah.unwrap_or(0),
            rot_x: self.rot_x.unwrap_or(0.0),
            rot_y: self.rot_y.unwrap_or(0.0),
            rot_z: self.rot_z.unwrap_or(0.0),
            pos_x: self.pos_x.unwrap_or(0.0),
            pos_y: self.pos_y.unwrap_or(0.0),
            scale,
            scale_y,
            scale_z,
            altitude: self.altitude.unwrap_or(0.0),
            is_spawn: self.is_spawn.unwrap_or(0),
            obstacle: self.obstacle.unwrap_or(true),
            keterangan: self.keterangan.unwrap_or_default(),
            shoot: self.shoot.unwrap_or_default(),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct MarkerCopyRow {
    operasi_id: Option<Uuid>,
    skenario_id: Option<Uuid>,
    unit_id: Option<Uuid>,
    name: Option<String>,
    jumlah: Option<i32>,
    rot_x: Option<f64>,
    rot_y: Option<f64>,
    rot_z: Option<f64>,
    keterangan: Option<String>,
    kategori: Option<String>,
    scale: Option<f64>,
    is_spawn: Option<i32>,
    shoot: Option<String>,
    scale_y: Option<f64>,
    scale_z: Option<f64>,
    altitude: Option<f64>,
    obstacle: Option<bool>,
}

#[derive(Debug, FromRow)]
struct BuildingRow {
    id: Uuid,
    skenario_id: Uuid,
    name: Option<String>,
    keterangan: Option<String>,
    color: Option<String>,
    geom: Option<String>,
    height: Option<f64>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl BuildingRow {
    fn into_contract(self, context: &ScenarioContext) -> Building {
        Building {
            id: self.id,
            name: self.name.unwrap_or_default(),
            operasi: context.operation.clone(),
            skenario: context.scenario.clone(),
            keterangan: self.keterangan.unwrap_or_default(),
            color: self.color.unwrap_or_default(),
            geom: self.geom.unwrap_or_default(),
            height: self.height.unwrap_or(0.0),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct LineRow {
    id: Uuid,
    skenario_id: Uuid,
    name: Option<String>,
    color: Option<String>,
    width: Option<f64>,
    alpha: Option<f64>,
    geom: Option<String>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl LineRow {
    fn into_contract(self, context: &ScenarioContext) -> Line {
        Line {
            id: self.id,
            name: self.name.unwrap_or_default(),
            operasi: context.operation.clone(),
            skenario: context.scenario.clone(),
            color: self.color.unwrap_or_default(),
            width: self.width.unwrap_or(0.0),
            alpha: self.alpha.unwrap_or(0.0),
            geom: self.geom.unwrap_or_default(),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct TextMarkerRow {
    id: Uuid,
    skenario_id: Uuid,
    text: Option<String>,
    color: Option<String>,
    font_size: Option<f64>,
    align: Option<String>,
    wrap_text: Option<bool>,
    pos_x: Option<f64>,
    pos_y: Option<f64>,
    altitude: Option<f64>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl TextMarkerRow {
    fn into_contract(self, context: &ScenarioContext) -> TextMarker {
        TextMarker {
            id: self.id,
            text: self.text.unwrap_or_default(),
            operasi: context.operation.clone(),
            skenario: context.scenario.clone(),
            color: self.color.unwrap_or_default(),
            font_size: self.font_size.unwrap_or(0.0),
            align: parse_text_align(self.align.as_deref()),
            wrap_text: self.wrap_text.unwrap_or(false),
            pos_x: self.pos_x.unwrap_or(0.0),
            pos_y: self.pos_y.unwrap_or(0.0),
            altitude: self.altitude.unwrap_or(0.0),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

fn text_align_str(align: TextAlign) -> &'static str {
    match align {
        TextAlign::Left => "left",
        TextAlign::Center => "center",
        TextAlign::Right => "right",
    }
}

fn parse_text_align(value: Option<&str>) -> TextAlign {
    match value {
        Some("left") => TextAlign::Left,
        Some("right") => TextAlign::Right,
        _ => TextAlign::Center,
    }
}

#[derive(Debug, FromRow)]
struct CategoryRow {
    id: Uuid,
    name: Option<String>,
    icon: Option<String>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl CategoryRow {
    fn into_contract(self) -> Category {
        Category {
            id: self.id,
            name: self.name.unwrap_or_default(),
            icon: self.icon.unwrap_or_default(),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct OperationRow {
    id: Uuid,
    name: Option<String>,
    satker_id: Option<Uuid>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl OperationRow {
    fn into_contract(self) -> Operation {
        Operation {
            id: self.id,
            name: self.name.unwrap_or_default(),
            satker_id: self.satker_id,
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct ScenarioLookupRow {
    id: Uuid,
    #[allow(dead_code)]
    operasi_id: Option<Uuid>,
}

#[derive(Debug, FromRow)]
struct CopyScenarioRow {
    #[allow(dead_code)]
    id: Uuid,
    operasi_id: Uuid,
    center_x: Option<f64>,
    center_y: Option<f64>,
    zoom: Option<f64>,
    min_zoom: Option<f64>,
    max_zoom: Option<f64>,
    pitch: Option<f64>,
    bearing: Option<f64>,
    map_mode: Option<String>,
    base_layer: Option<String>,
    sky_mode: Option<String>,
    indoor_config: Option<String>,
}

#[derive(Debug, FromRow)]
struct CopyMarkerRow {
    id: Uuid,
    unit_id: Option<Uuid>,
    name: Option<String>,
    jumlah: Option<i32>,
    rot_x: Option<f64>,
    rot_y: Option<f64>,
    rot_z: Option<f64>,
    pos_x: Option<f64>,
    pos_y: Option<f64>,
    keterangan: Option<String>,
    kategori: Option<String>,
    is_spawn: Option<i32>,
    shoot: Option<String>,
    scale: Option<f64>,
    scale_y: Option<f64>,
    scale_z: Option<f64>,
    altitude: Option<f64>,
    obstacle: Option<bool>,
}

#[derive(Debug, FromRow)]
struct CopyBuildingRow {
    name: Option<String>,
    keterangan: Option<String>,
    geom: Option<String>,
    color: Option<String>,
    height: Option<f64>,
}

#[derive(Debug, FromRow)]
struct CopyLineRow {
    name: Option<String>,
    color: Option<String>,
    width: Option<f64>,
    alpha: Option<f64>,
    geom: Option<String>,
}

#[derive(Debug, FromRow)]
struct CopyTextMarkerRow {
    text: String,
    color: String,
    font_size: f64,
    align: String,
    wrap_text: bool,
    pos_x: f64,
    pos_y: f64,
    altitude: f64,
}

#[derive(Debug, FromRow)]
struct CopyParticleRow {
    id: Uuid,
    name: Option<String>,
    pos_x: Option<f64>,
    pos_y: Option<f64>,
    data: Option<String>,
    is_spawn: Option<i32>,
    tipe: Option<String>,
    altitude: Option<f64>,
}

#[derive(Debug, FromRow)]
struct CopyAnimRow {
    id: Uuid,
    name: Option<String>,
    username: Option<String>,
    keterangan: Option<String>,
    tipe: Option<String>,
    data: Option<String>,
    geom: Option<String>,
    speed: Option<i32>,
    detik: Option<i32>,
    urut: Option<i32>,
    replay: Option<bool>,
    autoplay: Option<bool>,
}

#[derive(Debug, FromRow)]
struct UserRow {
    id: Uuid,
    name: Option<String>,
    username: Option<String>,
    role: Option<String>,
    satker_id: Option<Uuid>,
    profile_img: Option<String>,
    units: Option<String>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl UserRow {
    fn into_contract(self) -> User {
        User {
            id: self.id,
            name: self.name.unwrap_or_default(),
            username: self.username.unwrap_or_default(),
            role: self
                .role
                .as_deref()
                .and_then(|role| role.parse().ok())
                .unwrap_or(twg_domain::Role::User),
            profile_img: self.profile_img.unwrap_or_default(),
            units: self.units.unwrap_or_else(|| "[]".to_owned()),
            satker_id: self.satker_id,
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct SatkerRow {
    id: Uuid,
    code: Option<String>,
    name: Option<String>,
    is_active: Option<bool>,
    created_by: Option<Uuid>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl SatkerRow {
    fn into_contract(self) -> Satker {
        Satker {
            id: self.id,
            code: self.code.unwrap_or_default(),
            name: self.name.unwrap_or_default(),
            is_active: self.is_active.unwrap_or(false),
            created_by: self.created_by.unwrap_or_default(),
            created_at: self.created_at.unwrap_or_default(),
            updated_at: self.updated_at.unwrap_or_default(),
        }
    }
}

#[derive(Debug, FromRow)]
struct AlurRow {
    id: Uuid,
    alur: Option<String>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl AlurRow {
    fn into_contract(self, context: &ScenarioContext) -> Alur {
        Alur {
            id: self.id,
            operasi: context.operation.clone(),
            skenario: context.scenario.clone(),
            alur: self.alur.unwrap_or_default(),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct AlurWithScenarioRow {
    id: Uuid,
    skenario_id: Uuid,
    alur: Option<String>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl AlurWithScenarioRow {
    fn into_contract(self, context: &ScenarioContext) -> Alur {
        Alur {
            id: self.id,
            operasi: context.operation.clone(),
            skenario: context.scenario.clone(),
            alur: self.alur.unwrap_or_default(),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

fn remap_json_data_string(raw: &str, replacements: &HashMap<String, String>) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return raw.to_owned();
    }
    let Ok(mut payload) = serde_json::from_str::<Value>(trimmed) else {
        return raw.to_owned();
    };
    remap_json_value(&mut payload, replacements);
    serde_json::to_string(&payload).unwrap_or_else(|_| raw.to_owned())
}

fn remap_json_value(value: &mut Value, replacements: &HashMap<String, String>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if let Value::String(raw) = child
                    && (key == "data" || key == "geom")
                    && let Some(remapped) = try_remap_nested_json(raw, replacements)
                {
                    *child = Value::String(remapped);
                    continue;
                }
                remap_json_value(child, replacements);
            }
        }
        Value::Array(items) => {
            for item in items {
                remap_json_value(item, replacements);
            }
        }
        Value::String(raw) => {
            if let Some(replacement) = replacements.get(raw) {
                *raw = replacement.clone();
            }
        }
        _ => {}
    }
}

fn try_remap_nested_json(raw: &str, replacements: &HashMap<String, String>) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || (!trimmed.starts_with('{') && !trimmed.starts_with('[')) {
        return None;
    }
    let mut payload = serde_json::from_str::<Value>(trimmed).ok()?;
    remap_json_value(&mut payload, replacements);
    serde_json::to_string(&payload).ok()
}

/// Serializes JSON with Go's float formatting: whole float values are written
/// without a trailing `.0`, matching `encoding/json`.
pub fn go_json_string<T: Serialize>(value: &T) -> Result<String, WriteError> {
    let raw = serde_json::to_string(value)
        .map_err(|error| WriteError::Invalid(format!("invalid json data: {error}")))?;
    Ok(go_float_literals(&raw))
}

fn go_float_literals(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut characters = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(character) = characters.next() {
        if in_string {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            output.push(character);
            continue;
        }
        if character == '-' || character.is_ascii_digit() {
            let mut token = String::new();
            token.push(character);
            while let Some(&next) = characters.peek() {
                if next.is_ascii_digit() || matches!(next, '.' | 'e' | 'E' | '+' | '-') {
                    token.push(characters.next().expect("peeked character"));
                } else {
                    break;
                }
            }
            output.push_str(&go_number(&token));
            continue;
        }
        output.push(character);
    }
    output
}

fn go_number(token: &str) -> String {
    if token.contains('e') || token.contains('E') {
        return token.to_owned();
    }
    if let Some(stripped) = token.strip_suffix(".0") {
        return stripped.to_owned();
    }
    if token.contains('.') {
        return token.trim_end_matches('0').trim_end_matches('.').to_owned();
    }
    token.to_owned()
}

#[derive(Debug, FromRow)]
struct AnimRow {
    id: Uuid,
    skenario_id: Uuid,
    name: Option<String>,
    username: Option<String>,
    keterangan: Option<String>,
    tipe: Option<String>,
    data: Option<String>,
    geom: Option<String>,
    speed: Option<i64>,
    detik: Option<i64>,
    urut: Option<i32>,
    replay: Option<bool>,
    autoplay: Option<bool>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl AnimRow {
    fn into_contract(self, context: &ScenarioContext) -> Animation {
        Animation {
            id: self.id,
            operasi: context.operation.clone(),
            skenario: context.scenario.clone(),
            name: self.name.unwrap_or_default(),
            username: self.username.unwrap_or_default(),
            keterangan: self.keterangan.unwrap_or_default(),
            tipe: self.tipe.unwrap_or_default(),
            data: self.data.unwrap_or_default(),
            geom: self.geom.unwrap_or_default(),
            speed: self.speed.unwrap_or(0),
            detik: self.detik.unwrap_or(0),
            urut: self.urut.unwrap_or(0),
            replay: self.replay.unwrap_or(false),
            autoplay: self.autoplay.unwrap_or(false),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct AnimLookupRow {
    #[allow(dead_code)]
    id: Uuid,
    #[allow(dead_code)]
    skenario_id: Uuid,
    tipe: Option<String>,
    data: Option<String>,
    geom: Option<String>,
}

#[derive(Debug, FromRow)]
struct SpawnMarkerRow {
    id: Uuid,
    #[allow(dead_code)]
    skenario_id: Uuid,
    name: Option<String>,
    unit_id: Option<Uuid>,
    kategori: Option<String>,
    rot_x: Option<f64>,
    rot_y: Option<f64>,
    rot_z: Option<f64>,
    pos_x: Option<f64>,
    pos_y: Option<f64>,
    scale: Option<f64>,
    scale_y: Option<f64>,
    scale_z: Option<f64>,
}

#[derive(Debug, FromRow)]
struct MarkerShootRow {
    #[allow(dead_code)]
    id: Uuid,
    #[allow(dead_code)]
    skenario_id: Uuid,
    shoot: Option<String>,
}

#[derive(Debug, FromRow)]
struct ParticleRow {
    id: Uuid,
    skenario_id: Uuid,
    tipe: Option<String>,
    name: Option<String>,
    data: Option<String>,
    pos_x: Option<f64>,
    pos_y: Option<f64>,
    altitude: Option<f64>,
    is_spawn: Option<i32>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl ParticleRow {
    fn into_contract(self, context: &ScenarioContext) -> Particle {
        Particle {
            id: self.id,
            name: self.name.unwrap_or_default(),
            operasi: context.operation.clone(),
            skenario: context.scenario.clone(),
            tipe: self.tipe.unwrap_or_default(),
            data: self.data.unwrap_or_default(),
            pos_x: self.pos_x.unwrap_or(0.0),
            pos_y: self.pos_y.unwrap_or(0.0),
            altitude: self.altitude.unwrap_or(0.0),
            is_spawn: self.is_spawn.unwrap_or(0),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct MarkerPoseRow {
    pos_x: Option<f64>,
    pos_y: Option<f64>,
    rot_x: Option<f64>,
    rot_y: Option<f64>,
    rot_z: Option<f64>,
}

#[derive(Debug, FromRow)]
struct SnapshotRow {
    id: Uuid,
    operasi_id: Option<Uuid>,
    skenario_id: Option<Uuid>,
    snapshot_enabled: Option<bool>,
    map_state: Option<Value>,
    markers_state: Option<Value>,
    anims_state: Option<Value>,
    updated_by: Option<Uuid>,
    updated_at: Option<NaiveDateTime>,
    version: Option<i32>,
}

impl SnapshotRow {
    fn into_contract(self) -> Snapshot {
        Snapshot {
            id: self.id,
            operasi_id: self.operasi_id.unwrap_or_default(),
            skenario_id: self.skenario_id.unwrap_or_default(),
            snapshot_enabled: self.snapshot_enabled.unwrap_or(false),
            map_state: self.map_state.unwrap_or(Value::Null),
            markers_state: self.markers_state.unwrap_or(Value::Null),
            anims_state: self.anims_state.unwrap_or(Value::Null),
            updated_by: self.updated_by.unwrap_or_default(),
            updated_at: self.updated_at.unwrap_or_default(),
            version: self.version.unwrap_or(0),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Center {
    x: f64,
    y: f64,
}

#[derive(Debug, Deserialize)]
struct ZoomData {
    center: Center,
    zoom: f64,
    pitch: f64,
    bearing: f64,
}

#[derive(Debug, Deserialize)]
struct MoveData {
    x: f64,
    y: f64,
}

#[derive(Debug, Deserialize)]
struct RotData {
    x: f64,
    y: f64,
    z: f64,
}

#[cfg(test)]
mod tests {
    use super::go_json_string;

    #[test]
    fn go_float_literals_strip_whole_fraction() {
        assert_eq!(
            go_json_string(&vec![0.0_f64, 1.5, 3.0]).unwrap(),
            "[0,1.5,3]"
        );
    }

    #[test]
    fn go_float_literals_leave_strings_untouched() {
        assert_eq!(go_json_string(&"a 0.0 b").unwrap(), "\"a 0.0 b\"");
    }
}
