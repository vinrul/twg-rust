use serde::{Deserialize, Serialize};

use super::{Id, Timestamp};
use crate::geo::ColorHex;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Building {
    pub id: Id,
    pub operation_id: Id,
    pub scenario_id: Id,
    pub name: String,
    pub description: String,
    pub color: ColorHex,
    pub geometry_json: String,
    pub height: f64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
