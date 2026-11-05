use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeocodeResult {
    pub id: String,
    pub label: String,
    pub latitude: f64,
    pub longitude: f64,
    pub source: String,
}
