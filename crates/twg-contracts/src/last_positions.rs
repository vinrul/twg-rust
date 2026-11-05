use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastMap {
    pub id: Uuid,
    pub operasi_id: String,
    pub skenario_id: String,
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
    pub id: Uuid,
    pub operasi_id: String,
    pub skenario_id: String,
    pub marker_id: String,
    pub rot_x: f64,
    pub rot_y: f64,
    pub rot_z: f64,
    pub pos_x: f64,
    pub pos_y: f64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastPositionMap {
    pub last_map: LastMap,
    pub last_position: Vec<LastPosition>,
}
