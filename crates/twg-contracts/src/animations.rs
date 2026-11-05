use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::operations::Operation;
use crate::scenarios::Scenario;
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    pub id: Uuid,
    pub operasi: Operation,
    pub skenario: Scenario,
    pub name: String,
    pub username: String,
    pub keterangan: String,
    pub tipe: String,
    pub data: String,
    pub geom: String,
    pub speed: i64,
    pub detik: i64,
    pub urut: i32,
    pub replay: bool,
    pub autoplay: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationCreateRequest {
    pub operasi_id: String,
    pub skenario_id: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    pub tipe: String,
    pub data: String,
    pub geom: String,
    #[serde(default)]
    pub speed: i64,
    #[serde(default)]
    pub detik: i64,
    #[serde(default)]
    pub replay: bool,
    #[serde(default)]
    pub autoplay: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteAnimationV2Request {
    pub operasi_id: String,
    pub skenario_id: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    pub tipe: String,
    pub data: Value,
    pub geom: Value,
    #[serde(default)]
    pub speed: i64,
    #[serde(default)]
    pub detik: i64,
    #[serde(default)]
    pub replay: bool,
    #[serde(default)]
    pub autoplay: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnAnimationRequest {
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
    pub scale: f64,
    #[serde(default)]
    pub altitude: f64,
    pub marker_name: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnPickupRequest {
    pub operasi_id: String,
    pub skenario_id: String,
    #[serde(default)]
    pub marker_id: String,
    #[serde(default)]
    pub marker_ids: Vec<String>,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationRemoveRequest {
    pub operasi_id: String,
    pub skenario_id: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationCombineRequest {
    pub operasi_id: String,
    pub skenario_id: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    pub data: String,
    #[serde(default)]
    pub tipe: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationCombineItem {
    pub id: Uuid,
    pub tipe: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationCombineV2Request {
    pub operasi_id: String,
    pub skenario_id: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    pub data: Vec<AnimationCombineItem>,
    #[serde(default)]
    pub tipe: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShootAnimationRequest {
    pub operasi_id: String,
    pub skenario_id: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    pub data: String,
    pub marker_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShootDataV2 {
    pub marker_id: String,
    #[serde(default)]
    pub scale: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub dir_forward: f64,
    #[serde(default)]
    pub spread: f64,
    #[serde(default)]
    pub speed: f64,
    #[serde(default)]
    pub angle: f64,
    #[serde(default)]
    pub smoke_count: f64,
    pub tipe: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShootAnimationV2Request {
    pub operasi_id: String,
    pub skenario_id: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    pub data: ShootDataV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationDataUpdateRequest {
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    pub data: String,
    #[serde(default)]
    pub speed: i64,
    #[serde(default)]
    pub detik: i64,
    #[serde(default)]
    pub replay: bool,
    #[serde(default)]
    pub autoplay: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationDataV2UpdateRequest {
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    pub data: Value,
    #[serde(default)]
    pub speed: i64,
    #[serde(default)]
    pub detik: i64,
    #[serde(default)]
    pub replay: bool,
    #[serde(default)]
    pub autoplay: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationGeomUpdateRequest {
    pub geom: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationOrderRequest {
    pub urut: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationDeleteCombineDataRequest {
    pub tipe: String,
}
