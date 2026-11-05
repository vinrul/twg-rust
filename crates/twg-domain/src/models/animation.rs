use serde::{Deserialize, Serialize};

use super::{Id, Timestamp};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationType {
    Route,
    Spawn,
    Remove,
    Combine,
    Sequence,
    Shoot,
    Particle,
    CameraFollow,
    Zoom,
    RotateScale,
    Delay,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    pub id: Id,
    pub operation_id: Id,
    pub scenario_id: Id,
    pub name: String,
    pub username: String,
    pub description: String,
    pub animation_type: AnimationType,
    pub data: String,
    pub geometry_json: String,
    pub speed: i64,
    pub duration_seconds: i64,
    pub order: i32,
    pub replay: bool,
    pub autoplay: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
