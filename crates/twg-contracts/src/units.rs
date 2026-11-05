use crate::error::ContractError;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Unit {
    pub id: String,
    pub name: String,
    pub height: f32,
    pub rotation: UnitRotation,
    #[serde(rename = "modelUrl")]
    pub model_url: String,
    pub animation: bool,
    pub kategori: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UnitRotation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum UnitsPayload {
    List(Vec<Unit>),
    LegacyText(String),
}

impl UnitsPayload {
    pub fn into_units(self) -> Result<Vec<Unit>, ContractError> {
        match self {
            Self::List(units) => Ok(units),
            Self::LegacyText(value) if value.trim().is_empty() => Ok(Vec::new()),
            Self::LegacyText(value) => {
                serde_json::from_str(&value).map_err(ContractError::InvalidUnits)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UnitsPayload;

    #[test]
    fn parses_legacy_units_string() {
        let payload: UnitsPayload = serde_json::from_str("\"[]\"").unwrap();
        assert!(payload.into_units().unwrap().is_empty());
    }

    #[test]
    fn parses_units_array() {
        let payload: UnitsPayload = serde_json::from_str(
            r#"[{"id":"u1","name":"Unit","height":1.0,"rotation":{"x":0.0,"y":0.0,"z":0.0},"modelUrl":"/model.glb","animation":false,"kategori":"unit"}]"#,
        )
        .unwrap();
        let units = payload.into_units().unwrap();
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].model_url, "/model.glb");
    }

    #[test]
    fn empty_legacy_string_is_empty_list() {
        let payload = UnitsPayload::LegacyText(String::new());
        assert!(payload.into_units().unwrap().is_empty());
    }
}
