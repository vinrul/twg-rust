use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::operations::Operation;
use crate::scenarios::Scenario;
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub id: Uuid,
    pub name: String,
    pub operasi: Operation,
    pub skenario: Scenario,
    pub unit_id: String,
    pub kategori: String,
    pub jumlah: i32,
    pub rot_x: f64,
    pub rot_y: f64,
    pub rot_z: f64,
    pub pos_x: f64,
    pub pos_y: f64,
    pub scale: f64,
    pub scale_y: f64,
    pub scale_z: f64,
    pub altitude: f64,
    pub is_spawn: i32,
    pub keterangan: String,
    pub shoot: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkerCreateRequest {
    pub name: String,
    pub operasi_id: String,
    pub skenario_id: String,
    pub unit_id: String,
    #[serde(default)]
    pub kategori: String,
    #[serde(default)]
    pub rot_x: f64,
    #[serde(default)]
    pub rot_y: f64,
    #[serde(default)]
    pub rot_z: f64,
    pub pos_x: f64,
    pub pos_y: f64,
    pub jumlah: i32,
    #[serde(default)]
    pub keterangan: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkerGeomRequest {
    #[serde(default)]
    pub rot_x: f64,
    #[serde(default)]
    pub rot_y: f64,
    #[serde(default)]
    pub rot_z: f64,
    pub pos_x: f64,
    pub pos_y: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkerNameRequest {
    pub name: String,
    pub jumlah: i32,
    #[serde(default)]
    pub keterangan: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkerScaleRequest {
    pub scale: f64,
    #[serde(default)]
    pub scale_y: f64,
    #[serde(default)]
    pub scale_z: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkerAltitudeRequest {
    #[serde(default)]
    pub altitude: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkerRotateRequest {
    #[serde(default)]
    pub rotasi: f64,
    #[serde(default)]
    pub rotasi_x: f64,
    #[serde(default)]
    pub rotasi_y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkerDuplicateRequest {
    pub pos_x: f64,
    pub pos_y: f64,
}
