pub mod admin;
pub mod anims;
pub mod assets;
pub mod auth;
pub mod geometry;
pub mod health;
pub mod manage;
pub mod markers;
pub mod private;
pub mod public;
pub mod realtime;
pub mod sessions;
pub mod uploads;

use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

pub fn read_store(state: &AppState) -> Result<&twg_storage::ReadStore, ApiError> {
    state
        .read_store
        .as_deref()
        .ok_or_else(|| ApiError::service_unavailable("database is not available"))
}

pub fn write_store(state: &AppState) -> Result<&twg_storage::WriteStore, ApiError> {
    state
        .write_store
        .as_deref()
        .ok_or_else(|| ApiError::service_unavailable("database is not available"))
}

pub fn asset_store(state: &AppState) -> Result<&twg_storage::AssetStore, ApiError> {
    state
        .asset_store
        .as_deref()
        .ok_or_else(|| ApiError::service_unavailable("database is not available"))
}

pub fn parse_uuid(value: &str, field: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value.trim()).map_err(|_| ApiError::bad_request(format!("invalid {field}")))
}

pub fn compact_json<T: serde::Serialize>(value: &T) -> Result<String, ApiError> {
    twg_storage::go_json_string(value).map_err(Into::into)
}
