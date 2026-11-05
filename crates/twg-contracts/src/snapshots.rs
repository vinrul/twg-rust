use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::time::NaiveTimestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: Uuid,
    pub operasi_id: Uuid,
    pub skenario_id: Uuid,
    pub snapshot_enabled: bool,
    pub map_state: Value,
    pub markers_state: Value,
    pub anims_state: Value,
    pub updated_by: Uuid,
    pub updated_at: NaiveTimestamp,
    pub version: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotUpsertRequest {
    pub operasi_id: String,
    pub skenario_id: String,
    #[serde(default)]
    pub snapshot_enabled: bool,
    #[serde(default)]
    pub map_state: Value,
    #[serde(default)]
    pub markers_state: Value,
    #[serde(default)]
    pub anims_state: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotToggleRequest {
    pub snapshot_enabled: bool,
}
