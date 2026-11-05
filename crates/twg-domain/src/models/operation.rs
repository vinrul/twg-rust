use serde::{Deserialize, Serialize};

use super::{Id, Timestamp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub id: Id,
    pub name: String,
    pub satker_id: Option<Id>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
