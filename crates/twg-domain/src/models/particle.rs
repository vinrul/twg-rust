use serde::{Deserialize, Serialize};

use super::{Id, Timestamp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Particle {
    pub id: Id,
    pub operation_id: Id,
    pub scenario_id: Id,
    pub name: String,
    pub kind: String,
    pub data: String,
    pub pos_x: f64,
    pub pos_y: f64,
    pub is_spawn: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
