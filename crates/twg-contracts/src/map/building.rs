use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::operations::Operation;
use crate::scenarios::Scenario;
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Building {
    pub id: Uuid,
    pub name: String,
    pub operasi: Operation,
    pub skenario: Scenario,
    pub keterangan: String,
    pub color: String,
    pub geom: String,
    pub height: f64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildingCreateRequest {
    pub name: String,
    pub operasi_id: String,
    pub skenario_id: String,
    pub color: String,
    pub geom: String,
    pub height: f64,
    #[serde(default)]
    pub keterangan: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildingUpdateRequest {
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub keterangan: String,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildingGeomUpdateRequest {
    pub geom: String,
}
