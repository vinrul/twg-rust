use serde::{Deserialize, Serialize};

use super::{Id, Timestamp};
use crate::geo::{GeoCoordinate, Rotation3, Scale3};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub id: Id,
    pub operation_id: Id,
    pub scenario_id: Id,
    pub unit_id: Id,
    pub name: String,
    pub category: String,
    pub count: i32,
    pub position: GeoCoordinate,
    pub rotation: Rotation3,
    pub scale: Scale3,
    pub is_spawn: bool,
    pub shoot: String,
    pub description: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
