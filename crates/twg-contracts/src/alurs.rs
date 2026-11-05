use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::operations::Operation;
use crate::scenarios::Scenario;
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alur {
    pub id: Uuid,
    pub operasi: Operation,
    pub skenario: Scenario,
    pub alur: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlurCreateRequest {
    pub operasi_id: String,
    pub skenario_id: String,
    pub alur: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlurUpdateRequest {
    pub alur: String,
}
