use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid coordinates: {0}")]
    InvalidCoordinate(String),
    #[error("invalid color: {0}")]
    InvalidColor(String),
    #[error("invalid map view: {0}")]
    InvalidMapView(String),
    #[error("resource is outside the actor's satker scope")]
    SatkerScopeViolation,
}
