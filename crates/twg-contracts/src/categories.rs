use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    pub icon: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryRequest {
    pub name: String,
    #[serde(default)]
    pub icon: String,
}
