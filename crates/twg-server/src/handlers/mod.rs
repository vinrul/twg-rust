pub mod admin;
pub mod auth;
pub mod health;
pub mod private;
pub mod public;

use crate::error::ApiError;
use crate::state::AppState;

pub fn read_store(state: &AppState) -> Result<&twg_storage::ReadStore, ApiError> {
    state
        .read_store
        .as_deref()
        .ok_or_else(|| ApiError::service_unavailable("database is not available"))
}
