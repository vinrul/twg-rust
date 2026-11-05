use serde::{Deserialize, Serialize};

use super::{Id, Timestamp};
use crate::geo::ColorHex;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub id: Id,
    pub operation_id: Id,
    pub scenario_id: Id,
    pub name: String,
    pub color: ColorHex,
    pub width: f64,
    pub alpha: f64,
    pub geometry_json: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
