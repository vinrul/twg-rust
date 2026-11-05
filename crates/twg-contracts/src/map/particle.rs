use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::operations::Operation;
use crate::scenarios::Scenario;
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Particle {
    pub id: Uuid,
    pub name: String,
    pub operasi: Operation,
    pub skenario: Scenario,
    pub tipe: String,
    pub data: String,
    pub pos_x: f64,
    pub pos_y: f64,
    pub is_spawn: i32,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticleCreateRequest {
    pub operasi_id: String,
    pub skenario_id: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    #[serde(default)]
    pub data: String,
    pub tipe: String,
    pub is_spawn: String,
    pub pos_x: f64,
    pub pos_y: f64,
}
