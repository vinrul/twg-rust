use serde::{Deserialize, Serialize};
use twg_domain::Role;
use uuid::Uuid;

use crate::time::Timestamp;
use crate::units::Unit;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub device_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
    #[serde(default)]
    pub device_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogoutRequest {
    #[serde(default)]
    pub device_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserIdentity {
    pub id: Uuid,
    pub name: String,
    pub username: String,
    pub role: Role,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<Uuid>,
    pub profile_img: String,
    pub units: Vec<Unit>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginResponse {
    pub user: UserIdentity,
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: Timestamp,
    pub refresh_token_expires_at: Timestamp,
}
