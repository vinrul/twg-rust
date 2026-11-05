use serde::{Deserialize, Serialize};

use super::{Id, Timestamp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Satker {
    pub id: Id,
    pub code: String,
    pub name: String,
    pub is_active: bool,
    pub created_by: Id,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
