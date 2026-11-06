use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use twg_contracts::auth::UserIdentity;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredSession {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: String,
    pub refresh_expires_at: String,
    pub device_id: String,
    pub user: UserIdentity,
}

/// Persistence for the signed-in session.
///
/// Desktop uses the OS credential store; web uses `localStorage` (see the
/// security note in `docs/PHASE8_REPORT.md`).
pub trait TokenStore: Send + Sync {
    fn load(&self) -> Option<StoredSession>;
    fn save(&self, session: &StoredSession);
    fn clear(&self);
}

#[derive(Default)]
pub struct MemoryTokenStore {
    session: Mutex<Option<StoredSession>>,
}

impl MemoryTokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_session(session: StoredSession) -> Self {
        Self {
            session: Mutex::new(Some(session)),
        }
    }
}

impl TokenStore for MemoryTokenStore {
    fn load(&self) -> Option<StoredSession> {
        self.session.lock().expect("token store poisoned").clone()
    }

    fn save(&self, session: &StoredSession) {
        *self.session.lock().expect("token store poisoned") = Some(session.clone());
    }

    fn clear(&self) {
        *self.session.lock().expect("token store poisoned") = None;
    }
}

/// Device id is stable per installation. It is generated once and reused for
/// login, refresh, and every authenticated request.
pub fn generate_device_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::{MemoryTokenStore, StoredSession, TokenStore};
    use twg_contracts::auth::UserIdentity;
    use twg_domain::Role;
    use uuid::Uuid;

    fn stored() -> StoredSession {
        StoredSession {
            access_token: "access".to_owned(),
            refresh_token: "refresh".to_owned(),
            access_expires_at: "2026-01-01T00:00:00+00:00".to_owned(),
            refresh_expires_at: "2026-01-08T00:00:00+00:00".to_owned(),
            device_id: "device".to_owned(),
            user: UserIdentity {
                id: Uuid::nil(),
                name: "User".to_owned(),
                username: "user".to_owned(),
                role: Role::Admin,
                satker_id: None,
                profile_img: String::new(),
                units: Vec::new(),
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            },
        }
    }

    #[test]
    fn memory_store_round_trips() {
        let store = MemoryTokenStore::new();
        assert!(store.load().is_none());
        store.save(&stored());
        assert_eq!(store.load().unwrap().access_token, "access");
        store.clear();
        assert!(store.load().is_none());
    }
}
