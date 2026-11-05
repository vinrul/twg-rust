use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::operations::Operation;
use crate::scenarios::Scenario;
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub id: Uuid,
    pub name: String,
    pub operasi: Operation,
    pub skenario: Scenario,
    pub color: String,
    pub width: f64,
    pub alpha: f64,
    pub geom: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineCreateRequest {
    pub name: String,
    pub operasi_id: String,
    pub skenario_id: String,
    pub color: String,
    pub width: f64,
    pub alpha: f64,
    pub geom: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineUpdateRequest {
    pub name: String,
    pub color: String,
    pub width: f64,
    pub alpha: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineGeomUpdateRequest {
    pub geom: String,
}
