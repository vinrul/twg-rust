use serde::{Deserialize, Serialize};

use super::{Id, Timestamp};
use crate::geo::ColorHex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMarker {
    pub id: Id,
    pub operation_id: Id,
    pub scenario_id: Id,
    pub text: String,
    pub color: ColorHex,
    pub font_size: f64,
    pub align: TextAlign,
    pub wrap_text: bool,
    pub pos_x: f64,
    pub pos_y: f64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
