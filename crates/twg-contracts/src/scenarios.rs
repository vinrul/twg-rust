use serde::{Deserialize, Serialize};
use serde_json::Value;
use twg_domain::{BaseLayer, MapMode};
use uuid::Uuid;

use crate::operations::Operation;
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub id: Uuid,
    pub name: String,
    pub operasi: Operation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<Uuid>,
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
    pub min_zoom: f64,
    pub max_zoom: f64,
    pub pitch: f64,
    pub bearing: f64,
    pub map_mode: MapMode,
    pub base_layer: BaseLayer,
    #[serde(default = "empty_object")]
    pub indoor_config: Value,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioCreateRequest {
    pub name: String,
    pub operasi_id: String,
    #[serde(default)]
    pub center_x: f64,
    #[serde(default)]
    pub center_y: f64,
    pub zoom: f64,
    #[serde(default)]
    pub min_zoom: f64,
    pub max_zoom: f64,
    #[serde(default)]
    pub pitch: f64,
    #[serde(default)]
    pub bearing: f64,
    #[serde(default)]
    pub map_mode: String,
    #[serde(default)]
    pub base_layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indoor_config: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioUpdateRequest {
    #[serde(default)]
    pub center_x: f64,
    #[serde(default)]
    pub center_y: f64,
    pub zoom: f64,
    #[serde(default)]
    pub min_zoom: f64,
    #[serde(default)]
    pub max_zoom: f64,
    #[serde(default)]
    pub pitch: f64,
    #[serde(default)]
    pub bearing: f64,
    #[serde(default)]
    pub map_mode: String,
    #[serde(default)]
    pub base_layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indoor_config: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioNameUpdateRequest {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyScenarioRequest {
    pub name: String,
    pub src_skenario_id: String,
    pub tgr_operasi_id: String,
}
