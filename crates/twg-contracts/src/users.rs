use serde::{Deserialize, Serialize};
use twg_domain::Role;
use uuid::Uuid;

use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub username: String,
    pub role: Role,
    pub profile_img: String,
    pub units: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<Uuid>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCreateRequest {
    pub name: String,
    pub username: String,
    pub password: String,
    pub units: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<String>,
    #[serde(default)]
    pub profile_img: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserUpdateRequest {
    pub name: String,
    pub units: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<String>,
    #[serde(default)]
    pub profile_img: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateProfileRequest {
    pub name: String,
    #[serde(default)]
    pub profile_img: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: String,
    pub is_single_device: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnlineUser {
    pub id: String,
    pub name: String,
    pub username: String,
    pub role: Role,
    pub profile_img: String,
    pub active_session_count: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<Timestamp>,
    pub sessions: Vec<Session>,
}
