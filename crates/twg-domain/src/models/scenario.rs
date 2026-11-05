use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Id, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapMode {
    Outdoor,
    Indoor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseLayer {
    Osm,
    Satellite,
    GridDark,
    GridLight,
    BlankDark,
    BlankLight,
    FloorPlan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub id: Id,
    pub operation_id: Id,
    pub satker_id: Option<Id>,
    pub name: String,
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
    pub min_zoom: f64,
    pub max_zoom: f64,
    pub pitch: f64,
    pub bearing: f64,
    pub map_mode: MapMode,
    pub base_layer: BaseLayer,
    pub indoor_config: Value,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
