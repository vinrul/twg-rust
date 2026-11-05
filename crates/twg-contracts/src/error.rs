use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("invalid legacy units JSON: {0}")]
    InvalidUnits(#[from] serde_json::Error),
    #[error("invalid contract field: {0}")]
    InvalidField(String),
}
