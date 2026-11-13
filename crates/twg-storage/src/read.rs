use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
use serde_json::Value;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use twg_contracts::Timestamp;
use twg_contracts::alurs::Alur;
use twg_contracts::animations::Animation;
use twg_contracts::categories::Category;
use twg_contracts::last_positions::{LastMap, LastPosition, LastPositionMap};
use twg_contracts::map::{Building, Line, Marker, Particle, TextMarker};
use twg_contracts::operations::{HomeOperation, HomeScenario, Operation};
use twg_contracts::satkers::Satker;
use twg_contracts::scenarios::Scenario;
use twg_contracts::snapshots::Snapshot;
use twg_contracts::users::{OnlineUser, Session, User};
use twg_domain::{BaseLayer, MapMode, Role, TextAlign};
use uuid::Uuid;

#[derive(Clone)]
pub struct ReadStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SatkerScope {
    All,
    Satker(Uuid),
    Unscoped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserListScope {
    AllExceptAdmin,
    Satker(Uuid),
}

#[derive(Clone)]
pub(crate) struct ScenarioContext {
    pub(crate) operation: Operation,
    pub(crate) scenario: Scenario,
}

impl ReadStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_scenarios(&self, operasi_id: Uuid) -> Result<Vec<Scenario>, sqlx::Error> {
        let operation = self.operation_by_id(operasi_id).await?;
        let rows = sqlx::query_as::<_, ScenarioRow>(
            "SELECT id, operasi_id, satker_id, name, center_x, center_y, zoom, min_zoom,
                    max_zoom, pitch, bearing, map_mode, base_layer, sky_mode, indoor_config,
                    created_at, updated_at
             FROM skenarios
             WHERE operasi_id = $1
             ORDER BY created_at ASC",
        )
        .bind(operasi_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                row.into_contract(
                    operation
                        .clone()
                        .unwrap_or_else(|| zero_operation(operasi_id)),
                )
            })
            .collect())
    }

    pub async fn get_scenario(&self, id: Uuid) -> Result<Option<Scenario>, sqlx::Error> {
        let Some(row) = sqlx::query_as::<_, ScenarioRow>(
            "SELECT id, operasi_id, satker_id, name, center_x, center_y, zoom, min_zoom,
                    max_zoom, pitch, bearing, map_mode, base_layer, sky_mode, indoor_config,
                    created_at, updated_at
             FROM skenarios
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let operation_id = row.operasi_id;
        let operation = self.operation_by_id(operation_id).await?;
        Ok(Some(row.into_contract(
            operation.unwrap_or_else(|| zero_operation(operation_id)),
        )))
    }

    pub async fn list_markers(&self, skenario_id: Uuid) -> Result<Vec<Marker>, sqlx::Error> {
        let Some(context) = self.scenario_context(skenario_id).await? else {
            return Ok(Vec::new());
        };
        let rows = sqlx::query_as::<_, MarkerRow>(
            "SELECT id, name, unit_id, kategori, jumlah, rot_x, rot_y, rot_z, pos_x, pos_y,
                    scale, scale_y, scale_z, altitude, is_spawn, obstacle, keterangan, shoot,
                    created_at, updated_at
             FROM markers
             WHERE skenario_id = $1
             ORDER BY created_at DESC",
        )
        .bind(skenario_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| row.into_contract(&context))
            .collect())
    }

    pub async fn list_buildings(&self, skenario_id: Uuid) -> Result<Vec<Building>, sqlx::Error> {
        let Some(context) = self.scenario_context(skenario_id).await? else {
            return Ok(Vec::new());
        };
        let rows = sqlx::query_as::<_, BuildingRow>(
            "SELECT id, name, keterangan, color, geom, height, created_at, updated_at
             FROM buildings
             WHERE skenario_id = $1
             ORDER BY created_at DESC",
        )
        .bind(skenario_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| row.into_contract(&context))
            .collect())
    }

    pub async fn list_lines(&self, skenario_id: Uuid) -> Result<Vec<Line>, sqlx::Error> {
        let Some(context) = self.scenario_context(skenario_id).await? else {
            return Ok(Vec::new());
        };
        let rows = sqlx::query_as::<_, LineRow>(
            "SELECT id, name, color, width, alpha, geom, keterangan, created_at, updated_at
             FROM plot_lines
             WHERE skenario_id = $1
             ORDER BY created_at DESC",
        )
        .bind(skenario_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| row.into_contract(&context))
            .collect())
    }

    pub async fn list_particles(&self, skenario_id: Uuid) -> Result<Vec<Particle>, sqlx::Error> {
        let Some(context) = self.scenario_context(skenario_id).await? else {
            return Ok(Vec::new());
        };
        let rows = sqlx::query_as::<_, ParticleRow>(
            "SELECT id, name, tipe, data, pos_x, pos_y, altitude, is_spawn, created_at, updated_at
             FROM particles
             WHERE skenario_id = $1
             ORDER BY created_at DESC",
        )
        .bind(skenario_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| row.into_contract(&context))
            .collect())
    }

    pub async fn list_text_markers(
        &self,
        skenario_id: Uuid,
    ) -> Result<Vec<TextMarker>, sqlx::Error> {
        let Some(context) = self.scenario_context(skenario_id).await? else {
            return Ok(Vec::new());
        };
        let rows = sqlx::query_as::<_, TextMarkerRow>(
            "SELECT id, text, color, font_size, align, wrap_text, pos_x, pos_y, altitude,
                    created_at, updated_at
             FROM text_markers
             WHERE skenario_id = $1
             ORDER BY created_at DESC",
        )
        .bind(skenario_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| row.into_contract(&context))
            .collect())
    }

    pub async fn list_alurs(&self, skenario_id: Uuid) -> Result<Vec<Alur>, sqlx::Error> {
        let Some(context) = self.scenario_context(skenario_id).await? else {
            return Ok(Vec::new());
        };
        let rows = sqlx::query_as::<_, AlurRow>(
            "SELECT id, alur, created_at, updated_at
             FROM alurs
             WHERE skenario_id = $1
             ORDER BY created_at ASC",
        )
        .bind(skenario_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| row.into_contract(&context))
            .collect())
    }

    pub async fn list_anims(&self, skenario_id: Uuid) -> Result<Vec<Animation>, sqlx::Error> {
        let Some(context) = self.scenario_context(skenario_id).await? else {
            return Ok(Vec::new());
        };
        let rows = sqlx::query_as::<_, AnimRow>(
            "SELECT id, name, username, keterangan, tipe, data, geom, speed::int8 AS speed,
                    detik::int8 AS detik, urut, replay, autoplay, created_at, updated_at
             FROM anims
             WHERE skenario_id = $1 AND tipe NOT IN ('is_combine', 'is_sequence')
             ORDER BY urut ASC",
        )
        .bind(skenario_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| row.into_contract(&context))
            .collect())
    }

    pub async fn last_positions(&self, skenario_id: Uuid) -> Result<LastPositionMap, sqlx::Error> {
        let maps = sqlx::query_as::<_, LastMapRow>(
            "SELECT id, operasi_id, skenario_id, center_x, center_y, zoom, bearing, pitch,
                    created_at, updated_at
             FROM last_maps
             WHERE skenario_id = $1
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(skenario_id)
        .fetch_all(&self.pool)
        .await?;
        let positions = sqlx::query_as::<_, LastPositionRow>(
            "SELECT id, operasi_id, skenario_id, marker_id, rot_x, rot_y, rot_z, pos_x,
                    pos_y, created_at, updated_at
             FROM last_positions
             WHERE skenario_id = $1
             ORDER BY created_at ASC",
        )
        .bind(skenario_id)
        .fetch_all(&self.pool)
        .await?;

        let last_map = maps
            .into_iter()
            .next()
            .map(LastMapRow::into_contract)
            .unwrap_or_else(|| LastMap {
                id: Uuid::nil(),
                operasi_id: String::new(),
                skenario_id: String::new(),
                center_x: 0.0,
                center_y: 0.0,
                zoom: 0.0,
                bearing: 0.0,
                pitch: 0.0,
                created_at: zero_timestamp(),
                updated_at: zero_timestamp(),
            });
        Ok(LastPositionMap {
            last_map,
            last_position: positions
                .into_iter()
                .map(LastPositionRow::into_contract)
                .collect(),
        })
    }

    pub async fn snapshot(&self, skenario_id: Uuid) -> Result<Option<Snapshot>, sqlx::Error> {
        let row = sqlx::query_as::<_, SnapshotRow>(
            "SELECT id, operasi_id, skenario_id, snapshot_enabled, map_state, markers_state,
                    anims_state, updated_by, updated_at, version
             FROM game_session_snapshots
             WHERE skenario_id = $1",
        )
        .bind(skenario_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(SnapshotRow::into_contract))
    }

    pub async fn home_operations(
        &self,
        scope: SatkerScope,
    ) -> Result<Vec<HomeOperation>, sqlx::Error> {
        let operations = self.operations_for_scope(scope).await?;
        let mut result = Vec::with_capacity(operations.len());
        for operation in operations {
            let scenarios = sqlx::query_as::<_, (Uuid, Option<String>)>(
                "SELECT id, name FROM skenarios WHERE operasi_id = $1 ORDER BY created_at ASC",
            )
            .bind(operation.id)
            .fetch_all(&self.pool)
            .await?;
            result.push(HomeOperation {
                name: operation.name.clone(),
                operation_id: operation.id,
                satker_id: operation.satker_id,
                skenario: scenarios
                    .into_iter()
                    .map(|(scenario_id, name)| HomeScenario {
                        name: name.unwrap_or_default(),
                        scenario_id,
                    })
                    .collect(),
            });
        }
        Ok(result)
    }

    pub async fn get_home_operation(
        &self,
        operation_id: Uuid,
    ) -> Result<Option<HomeOperation>, sqlx::Error> {
        let Some(operation) = self.operation_by_id(operation_id).await? else {
            return Ok(None);
        };
        let scenarios = sqlx::query_as::<_, (Uuid, Option<String>)>(
            "SELECT id, name FROM skenarios WHERE operasi_id = $1 ORDER BY created_at ASC",
        )
        .bind(operation_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(Some(HomeOperation {
            name: operation.name,
            operation_id,
            satker_id: operation.satker_id,
            skenario: scenarios
                .into_iter()
                .map(|(scenario_id, name)| HomeScenario {
                    name: name.unwrap_or_default(),
                    scenario_id,
                })
                .collect(),
        }))
    }

    /// Returns the page and the total count after search/filtering.
    pub async fn list_operations(
        &self,
        scope: SatkerScope,
        search: &str,
        page_size: usize,
        page_number: usize,
    ) -> Result<(Vec<Operation>, usize), sqlx::Error> {
        let rows = self.operations_for_scope(scope).await?;
        let needle = search.trim().to_lowercase();
        let filtered = rows
            .into_iter()
            .filter(|operation| {
                needle.is_empty() || operation.name.to_lowercase().contains(&needle)
            })
            .collect::<Vec<_>>();
        let total = filtered.len();
        let offset = page_number.saturating_sub(1).saturating_mul(page_size);
        let page = filtered.into_iter().skip(offset).take(page_size).collect();
        Ok((page, total))
    }

    pub async fn list_users(&self, scope: UserListScope) -> Result<Vec<User>, sqlx::Error> {
        let rows = match scope {
            UserListScope::AllExceptAdmin => {
                sqlx::query_as::<_, UserRow>(
                    "SELECT id, name, username, role, satker_id, profile_img, units,
                            created_at, updated_at
                     FROM users
                     WHERE role != 'admin'",
                )
                .fetch_all(&self.pool)
                .await?
            }
            UserListScope::Satker(satker_id) => {
                sqlx::query_as::<_, UserRow>(
                    "SELECT id, name, username, role, satker_id, profile_img, units,
                            created_at, updated_at
                     FROM users
                     WHERE satker_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(satker_id)
                .fetch_all(&self.pool)
                .await?
            }
        };
        Ok(rows.into_iter().map(UserRow::into_contract).collect())
    }

    pub async fn list_satkers(
        &self,
        scope: SatkerScope,
        active: Option<bool>,
    ) -> Result<Vec<Satker>, sqlx::Error> {
        let rows = sqlx::query_as::<_, SatkerRow>(
            "SELECT id, code, name, is_active, created_by, created_at, updated_at
             FROM satkers
             ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .filter(|row| active.is_none_or(|active| row.is_active.unwrap_or(false) == active))
            .filter(|row| match scope {
                SatkerScope::All => true,
                SatkerScope::Satker(id) => row.id == id,
                SatkerScope::Unscoped => false,
            })
            .map(SatkerRow::into_contract)
            .collect())
    }

    pub async fn get_operation(&self, id: Uuid) -> Result<Option<Operation>, sqlx::Error> {
        self.operation_by_id(id).await
    }

    pub async fn get_user(&self, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, name, username, role, satker_id, profile_img, units,
                    created_at, updated_at
             FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(UserRow::into_contract))
    }

    pub async fn marker_scenario_id(&self, id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
        sqlx::query_scalar("SELECT skenario_id FROM markers WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn get_category(&self, id: Uuid) -> Result<Option<Category>, sqlx::Error> {
        let row = sqlx::query_as::<_, CategoryRow>(
            "SELECT id, name, icon, created_at, updated_at
             FROM categories
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(CategoryRow::into_contract))
    }

    pub async fn list_categories(&self) -> Result<Vec<Category>, sqlx::Error> {
        let rows = sqlx::query_as::<_, CategoryRow>(
            "SELECT id, name, icon, created_at, updated_at
             FROM categories
             ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(CategoryRow::into_contract).collect())
    }

    pub async fn list_online_users(&self) -> Result<Vec<OnlineUser>, sqlx::Error> {
        let users = sqlx::query_as::<_, UserRow>(
            "SELECT id, name, username, role, satker_id, profile_img, units,
                    created_at, updated_at
             FROM users",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut online = Vec::new();
        for user in users {
            let sessions = sqlx::query_as::<_, OnlineSessionRow>(
                "SELECT id, user_id, device_id, is_single_device, created_at, updated_at,
                        last_used_at
                 FROM user_sessions
                 WHERE user_id = $1 AND revoked_at IS NULL
                 ORDER BY last_used_at DESC NULLS LAST, created_at DESC",
            )
            .bind(user.id)
            .fetch_all(&self.pool)
            .await?;
            if sessions.is_empty() {
                continue;
            }
            let sessions = sessions
                .into_iter()
                .map(OnlineSessionRow::into_contract)
                .collect::<Vec<_>>();
            let last_used_at = sessions
                .iter()
                .filter_map(|session| session.last_used_at)
                .max();
            online.push(OnlineUser {
                id: user.id.to_string(),
                name: user.name.clone().unwrap_or_default(),
                username: user.username.clone().unwrap_or_default(),
                role: parse_role(user.role.as_deref()),
                profile_img: user.profile_img.clone().unwrap_or_default(),
                active_session_count: sessions.len() as i32,
                last_used_at,
                sessions,
            });
        }
        Ok(online)
    }

    async fn operations_for_scope(
        &self,
        scope: SatkerScope,
    ) -> Result<Vec<Operation>, sqlx::Error> {
        let rows = match scope {
            SatkerScope::All => {
                sqlx::query_as::<_, OperationRow>(
                    "SELECT id, name, satker_id, created_at, updated_at
                     FROM operasis
                     ORDER BY created_at DESC",
                )
                .fetch_all(&self.pool)
                .await?
            }
            SatkerScope::Satker(satker_id) => {
                sqlx::query_as::<_, OperationRow>(
                    "SELECT id, name, satker_id, created_at, updated_at
                     FROM operasis
                     WHERE satker_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(satker_id)
                .fetch_all(&self.pool)
                .await?
            }
            SatkerScope::Unscoped => {
                sqlx::query_as::<_, OperationRow>(
                    "SELECT id, name, satker_id, created_at, updated_at
                     FROM operasis
                     WHERE satker_id IS NULL
                     ORDER BY created_at DESC",
                )
                .fetch_all(&self.pool)
                .await?
            }
        };
        Ok(rows.into_iter().map(OperationRow::into_contract).collect())
    }

    async fn operation_by_id(&self, id: Uuid) -> Result<Option<Operation>, sqlx::Error> {
        let row = sqlx::query_as::<_, OperationRow>(
            "SELECT id, name, satker_id, created_at, updated_at
             FROM operasis
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(OperationRow::into_contract))
    }

    pub(crate) async fn scenario_context(
        &self,
        skenario_id: Uuid,
    ) -> Result<Option<ScenarioContext>, sqlx::Error> {
        let Some(scenario) = self.get_scenario(skenario_id).await? else {
            return Ok(None);
        };
        let operation = self
            .operation_by_id(scenario.operasi.id)
            .await?
            .unwrap_or_else(|| zero_operation(scenario.operasi.id));
        Ok(Some(ScenarioContext {
            operation,
            scenario,
        }))
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
struct ScenarioRow {
    id: Uuid,
    operasi_id: Uuid,
    satker_id: Option<Uuid>,
    name: Option<String>,
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
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl ScenarioRow {
    fn into_contract(self, operation: Operation) -> Scenario {
        Scenario {
            id: self.id,
            name: self.name.unwrap_or_default(),
            operasi: operation,
            satker_id: self.satker_id,
            center_x: self.center_x.unwrap_or(0.0),
            center_y: self.center_y.unwrap_or(0.0),
            zoom: self.zoom.unwrap_or(0.0),
            min_zoom: self.min_zoom.unwrap_or(0.0),
            max_zoom: self.max_zoom.unwrap_or(0.0),
            pitch: self.pitch.unwrap_or(0.0),
            bearing: self.bearing.unwrap_or(0.0),
            map_mode: parse_map_mode(self.map_mode.as_deref()),
            base_layer: parse_base_layer(self.base_layer.as_deref()),
            sky_mode: self.sky_mode.unwrap_or_else(|| "black".to_owned()),
            indoor_config: self
                .indoor_config
                .as_deref()
                .and_then(|value| serde_json::from_str(value).ok())
                .unwrap_or_else(|| Value::Object(serde_json::Map::new())),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct MarkerRow {
    id: Uuid,
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
struct BuildingRow {
    id: Uuid,
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
    name: Option<String>,
    color: Option<String>,
    width: Option<f64>,
    alpha: Option<f64>,
    geom: Option<String>,
    keterangan: Option<String>,
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
            keterangan: self.keterangan.unwrap_or_default(),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct ParticleRow {
    id: Uuid,
    name: Option<String>,
    tipe: Option<String>,
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
struct TextMarkerRow {
    id: Uuid,
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
struct AnimRow {
    id: Uuid,
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
struct LastMapRow {
    id: Uuid,
    operasi_id: Option<Uuid>,
    skenario_id: Option<Uuid>,
    center_x: Option<f64>,
    center_y: Option<f64>,
    zoom: Option<f64>,
    bearing: Option<f64>,
    pitch: Option<f64>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl LastMapRow {
    fn into_contract(self) -> LastMap {
        LastMap {
            id: self.id,
            operasi_id: self.operasi_id.map(|id| id.to_string()).unwrap_or_default(),
            skenario_id: self
                .skenario_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            center_x: self.center_x.unwrap_or(0.0),
            center_y: self.center_y.unwrap_or(0.0),
            zoom: self.zoom.unwrap_or(0.0),
            bearing: self.bearing.unwrap_or(0.0),
            pitch: self.pitch.unwrap_or(0.0),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
}

#[derive(Debug, FromRow)]
struct LastPositionRow {
    id: Uuid,
    operasi_id: Option<Uuid>,
    skenario_id: Option<Uuid>,
    marker_id: Option<Uuid>,
    rot_x: Option<f64>,
    rot_y: Option<f64>,
    rot_z: Option<f64>,
    pos_x: Option<f64>,
    pos_y: Option<f64>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl LastPositionRow {
    fn into_contract(self) -> LastPosition {
        LastPosition {
            id: self.id,
            operasi_id: self.operasi_id.map(|id| id.to_string()).unwrap_or_default(),
            skenario_id: self
                .skenario_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            marker_id: self.marker_id.map(|id| id.to_string()).unwrap_or_default(),
            rot_x: self.rot_x.unwrap_or(0.0),
            rot_y: self.rot_y.unwrap_or(0.0),
            rot_z: self.rot_z.unwrap_or(0.0),
            pos_x: self.pos_x.unwrap_or(0.0),
            pos_y: self.pos_y.unwrap_or(0.0),
            created_at: wita(self.created_at),
            updated_at: wita(self.updated_at),
        }
    }
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
            role: parse_role(self.role.as_deref()),
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
struct OnlineSessionRow {
    id: Uuid,
    user_id: Uuid,
    device_id: String,
    is_single_device: Option<bool>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

impl OnlineSessionRow {
    fn into_contract(self) -> Session {
        Session {
            id: self.id,
            user_id: self.user_id,
            device_id: self.device_id,
            is_single_device: self.is_single_device.unwrap_or(false),
            created_at: wita_from_utc(self.created_at),
            updated_at: wita_from_utc(self.updated_at),
            last_used_at: self.last_used_at.map(wita_from_utc),
        }
    }
}

fn zero_operation(id: Uuid) -> Operation {
    Operation {
        id,
        name: String::new(),
        satker_id: None,
        created_at: wita(None),
        updated_at: wita(None),
    }
}

/// Matches Go's zero `time.Time` serialization for an absent last map.
fn zero_timestamp() -> Timestamp {
    Utc.with_ymd_and_hms(1, 1, 1, 0, 0, 0)
        .single()
        .expect("valid zero timestamp")
        .with_timezone(&FixedOffset::east_opt(0).expect("UTC offset"))
}

pub(crate) fn wita(value: Option<NaiveDateTime>) -> Timestamp {
    let naive = value.unwrap_or_else(|| Utc::now().naive_utc());
    Utc.from_utc_datetime(&naive).with_timezone(&wita_offset())
}

fn wita_from_utc(value: DateTime<Utc>) -> Timestamp {
    value.with_timezone(&wita_offset())
}

fn wita_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 60 * 60).expect("valid WITA offset")
}

fn parse_role(value: Option<&str>) -> Role {
    match value {
        Some("admin") => Role::Admin,
        Some("admin_monitor") => Role::AdminMonitor,
        Some("admin_satker") => Role::AdminSatker,
        Some("user_satker") => Role::UserSatker,
        Some("big_screen") => Role::BigScreen,
        _ => Role::User,
    }
}

fn parse_map_mode(value: Option<&str>) -> MapMode {
    match value {
        Some("indoor") => MapMode::Indoor,
        _ => MapMode::Outdoor,
    }
}

fn parse_base_layer(value: Option<&str>) -> BaseLayer {
    match value {
        Some("satellite") => BaseLayer::Satellite,
        Some("grid_dark") => BaseLayer::GridDark,
        Some("grid_light") => BaseLayer::GridLight,
        Some("blank_dark") => BaseLayer::BlankDark,
        Some("blank_light") => BaseLayer::BlankLight,
        Some("floor_plan") => BaseLayer::FloorPlan,
        _ => BaseLayer::Osm,
    }
}

fn parse_text_align(value: Option<&str>) -> TextAlign {
    match value {
        Some("left") => TextAlign::Left,
        Some("right") => TextAlign::Right,
        _ => TextAlign::Center,
    }
}
