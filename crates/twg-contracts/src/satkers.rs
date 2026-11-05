use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::time::NaiveTimestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Satker {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub is_active: bool,
    pub created_by: Uuid,
    pub created_at: NaiveTimestamp,
    pub updated_at: NaiveTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SatkerCreateRequest {
    pub code: String,
    pub name: String,
    pub is_active: bool,
}

pub type SatkerUpdateRequest = SatkerCreateRequest;
