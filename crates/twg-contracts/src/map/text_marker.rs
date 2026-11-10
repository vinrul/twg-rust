use serde::{Deserialize, Serialize};
use twg_domain::TextAlign;
use uuid::Uuid;

use crate::operations::Operation;
use crate::scenarios::Scenario;
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMarker {
    pub id: Uuid,
    pub text: String,
    pub operasi: Operation,
    pub skenario: Scenario,
    pub color: String,
    pub font_size: f64,
    pub align: TextAlign,
    pub wrap_text: bool,
    pub pos_x: f64,
    pub pos_y: f64,
    #[serde(default)]
    pub altitude: f64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMarkerCreateRequest {
    pub text: String,
    pub operasi_id: String,
    pub skenario_id: String,
    pub color: String,
    pub font_size: f64,
    pub align: TextAlign,
    #[serde(default)]
    pub wrap_text: bool,
    pub pos_x: f64,
    pub pos_y: f64,
    #[serde(default)]
    pub altitude: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMarkerUpdateRequest {
    pub text: String,
    pub color: String,
    pub font_size: f64,
    pub align: TextAlign,
    #[serde(default)]
    pub wrap_text: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMarkerGeomUpdateRequest {
    pub pos_x: f64,
    pub pos_y: f64,
    #[serde(default)]
    pub altitude: f64,
}
