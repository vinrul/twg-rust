use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Id, Timestamp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSessionSnapshot {
    pub id: Id,
    pub operation_id: Id,
    pub scenario_id: Id,
    pub snapshot_enabled: bool,
    pub map_state: Value,
    pub markers_state: Value,
    pub animations_state: Value,
    pub updated_by: Id,
    pub updated_at: Timestamp,
    pub version: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastMap {
    pub id: Id,
    pub operation_id: Id,
    pub scenario_id: Id,
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
    pub bearing: f64,
    pub pitch: f64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastPosition {
    pub id: Id,
    pub operation_id: Id,
    pub scenario_id: Id,
    pub marker_id: Id,
    pub rotation_x: f64,
    pub rotation_y: f64,
    pub rotation_z: f64,
    pub pos_x: f64,
    pub pos_y: f64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
