use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub id: Uuid,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<Uuid>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationRequest {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeScenario {
    pub name: String,
    #[serde(rename = "skenario_id")]
    pub scenario_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeOperation {
    pub name: String,
    #[serde(rename = "operasi_id")]
    pub operation_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<Uuid>,
    pub skenario: Vec<HomeScenario>,
}
