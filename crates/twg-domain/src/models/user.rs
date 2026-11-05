use serde::{Deserialize, Serialize};

use super::{Id, Timestamp};
use crate::access::Role;
use crate::geo::Rotation3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Unit {
    pub id: String,
    pub name: String,
    pub height: f32,
    pub rotation: Rotation3,
    pub model_url: String,
    pub animation: bool,
    pub kategori: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub id: Id,
    pub name: String,
    pub username: String,
    pub role: Role,
    pub satker_id: Option<Id>,
    pub profile_img: String,
    pub units: Vec<Unit>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
