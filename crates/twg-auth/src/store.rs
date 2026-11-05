use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;
use twg_domain::Role;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUser {
    pub id: Uuid,
    pub name: String,
    pub username: String,
    pub password_hash: String,
    pub role: Role,
    pub satker_id: Option<Uuid>,
    pub profile_img: String,
    pub units: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: String,
    pub refresh_token_hash: String,
    pub is_single_device: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotateOutcome {
    Rotated(SessionRecord),
    Revoked,
    HashMismatch,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthStoreError {
    #[error("user not found")]
    NotFound,
    #[error("already logged in on another device")]
    AlreadyLoggedIn,
    #[error("auth store error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait AuthStore: Send + Sync {
    async fn find_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<AuthUser>, AuthStoreError>;

    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<AuthUser>, AuthStoreError>;

    async fn ensure_single_device_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        refresh_token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<SessionRecord, AuthStoreError>;

    async fn upsert_multi_device_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        refresh_token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<SessionRecord, AuthStoreError>;

    async fn find_session_by_device(
        &self,
        user_id: Uuid,
        device_id: &str,
    ) -> Result<Option<SessionRecord>, AuthStoreError>;

    async fn rotate_refresh_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        expected_hash: &str,
        new_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<RotateOutcome, AuthStoreError>;

    async fn touch_session(
        &self,
        session_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), AuthStoreError>;

    async fn revoke_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), AuthStoreError>;

    async fn update_password(
        &self,
        user_id: Uuid,
        password_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<(), AuthStoreError>;
}

#[derive(Clone, Default)]
pub struct InMemoryAuthStore {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    users: BTreeMap<Uuid, AuthUser>,
    sessions: BTreeMap<Uuid, SessionRecord>,
}

impl InMemoryAuthStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_user(&self, user: AuthUser) {
        self.inner
            .lock()
            .expect("in-memory auth store poisoned")
            .users
            .insert(user.id, user);
    }

    pub fn active_session_count(&self) -> usize {
        self.inner
            .lock()
            .expect("in-memory auth store poisoned")
            .sessions
            .values()
            .filter(|session| session.revoked_at.is_none())
            .count()
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, AuthStoreError> {
        self.inner
            .lock()
            .map_err(|_| AuthStoreError::Internal("in-memory auth store poisoned".to_owned()))
    }
}

#[async_trait]
impl AuthStore for InMemoryAuthStore {
    async fn find_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<AuthUser>, AuthStoreError> {
        let inner = self.lock()?;
        Ok(inner
            .users
            .values()
            .find(|user| user.username == username)
            .cloned())
    }

    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<AuthUser>, AuthStoreError> {
        let inner = self.lock()?;
        Ok(inner.users.get(&user_id).cloned())
    }

    async fn ensure_single_device_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        refresh_token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<SessionRecord, AuthStoreError> {
        let mut inner = self.lock()?;
        if !inner.users.contains_key(&user_id) {
            return Err(AuthStoreError::NotFound);
        }
        if let Some(session) = inner.sessions.values_mut().find(|session| {
            session.user_id == user_id && session.revoked_at.is_none() && session.is_single_device
        }) {
            if session.device_id != device_id {
                return Err(AuthStoreError::AlreadyLoggedIn);
            }
            session.refresh_token_hash = refresh_token_hash.to_owned();
            session.updated_at = now;
            session.last_used_at = Some(now);
            return Ok(session.clone());
        }
        if let Some(session) = inner.sessions.values_mut().find(|session| {
            session.user_id == user_id
                && session.device_id == device_id
                && session.revoked_at.is_none()
        }) {
            session.refresh_token_hash = refresh_token_hash.to_owned();
            session.is_single_device = true;
            session.updated_at = now;
            session.last_used_at = Some(now);
            return Ok(session.clone());
        }
        let session = SessionRecord {
            id: Uuid::new_v4(),
            user_id,
            device_id: device_id.to_owned(),
            refresh_token_hash: refresh_token_hash.to_owned(),
            is_single_device: true,
            created_at: now,
            updated_at: now,
            last_used_at: Some(now),
            revoked_at: None,
        };
        inner.sessions.insert(session.id, session.clone());
        Ok(session)
    }

    async fn upsert_multi_device_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        refresh_token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<SessionRecord, AuthStoreError> {
        let mut inner = self.lock()?;
        if !inner.users.contains_key(&user_id) {
            return Err(AuthStoreError::NotFound);
        }
        if let Some(session) = inner.sessions.values_mut().find(|session| {
            session.user_id == user_id
                && session.device_id == device_id
                && session.revoked_at.is_none()
        }) {
            session.refresh_token_hash = refresh_token_hash.to_owned();
            session.is_single_device = false;
            session.updated_at = now;
            session.last_used_at = Some(now);
            return Ok(session.clone());
        }
        let session = SessionRecord {
            id: Uuid::new_v4(),
            user_id,
            device_id: device_id.to_owned(),
            refresh_token_hash: refresh_token_hash.to_owned(),
            is_single_device: false,
            created_at: now,
            updated_at: now,
            last_used_at: Some(now),
            revoked_at: None,
        };
        inner.sessions.insert(session.id, session.clone());
        Ok(session)
    }

    async fn find_session_by_device(
        &self,
        user_id: Uuid,
        device_id: &str,
    ) -> Result<Option<SessionRecord>, AuthStoreError> {
        let inner = self.lock()?;
        Ok(inner
            .sessions
            .values()
            .find(|session| {
                session.user_id == user_id
                    && session.device_id == device_id
                    && session.revoked_at.is_none()
            })
            .cloned())
    }

    async fn rotate_refresh_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        expected_hash: &str,
        new_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<RotateOutcome, AuthStoreError> {
        let mut inner = self.lock()?;
        let Some(session) = inner.sessions.values_mut().find(|session| {
            session.user_id == user_id
                && session.device_id == device_id
                && session.revoked_at.is_none()
        }) else {
            return Ok(RotateOutcome::Revoked);
        };
        if session.refresh_token_hash != expected_hash {
            return Ok(RotateOutcome::HashMismatch);
        }
        session.refresh_token_hash = new_hash.to_owned();
        session.updated_at = now;
        session.last_used_at = Some(now);
        Ok(RotateOutcome::Rotated(session.clone()))
    }

    async fn touch_session(
        &self,
        session_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), AuthStoreError> {
        let mut inner = self.lock()?;
        let Some(session) = inner.sessions.get_mut(&session_id) else {
            return Ok(());
        };
        if session.revoked_at.is_none() {
            session.last_used_at = Some(now);
            session.updated_at = now;
        }
        Ok(())
    }

    async fn revoke_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), AuthStoreError> {
        let mut inner = self.lock()?;
        for session in inner.sessions.values_mut() {
            if session.user_id == user_id
                && session.device_id == device_id
                && session.revoked_at.is_none()
            {
                session.revoked_at = Some(now);
                session.updated_at = now;
            }
        }
        Ok(())
    }

    async fn update_password(
        &self,
        user_id: Uuid,
        password_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<(), AuthStoreError> {
        let mut inner = self.lock()?;
        let Some(user) = inner.users.get_mut(&user_id) else {
            return Err(AuthStoreError::NotFound);
        };
        user.password_hash = password_hash.to_owned();
        user.updated_at = now;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use twg_domain::Role;
    use uuid::Uuid;

    use super::{AuthStore, AuthStoreError, AuthUser, InMemoryAuthStore, RotateOutcome};

    fn user(id: u128) -> AuthUser {
        let now = Utc::now();
        AuthUser {
            id: Uuid::from_u128(id),
            name: "Test".to_owned(),
            username: format!("user{id}"),
            password_hash: "hash".to_owned(),
            role: Role::User,
            satker_id: None,
            profile_img: String::new(),
            units: "[]".to_owned(),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn single_device_session_is_locked_to_the_first_device() {
        let store = InMemoryAuthStore::new();
        store.insert_user(user(1));
        let now = Utc::now();
        let session = store
            .ensure_single_device_session(Uuid::from_u128(1), "a", "hash-a", now)
            .await
            .unwrap();
        assert!(session.is_single_device);

        let error = store
            .ensure_single_device_session(Uuid::from_u128(1), "b", "hash-b", now)
            .await
            .unwrap_err();
        assert_eq!(error, AuthStoreError::AlreadyLoggedIn);
    }

    #[tokio::test]
    async fn refresh_rotation_is_single_flight() {
        let store = InMemoryAuthStore::new();
        store.insert_user(user(2));
        let now = Utc::now();
        store
            .upsert_multi_device_session(Uuid::from_u128(2), "a", "old", now)
            .await
            .unwrap();
        let outcome = store
            .rotate_refresh_session(Uuid::from_u128(2), "a", "old", "new", now)
            .await
            .unwrap();
        assert!(matches!(outcome, RotateOutcome::Rotated(_)));
        let replay = store
            .rotate_refresh_session(Uuid::from_u128(2), "a", "old", "newer", now)
            .await
            .unwrap();
        assert_eq!(replay, RotateOutcome::HashMismatch);
    }

    #[tokio::test]
    async fn revoked_sessions_are_not_returned() {
        let store = InMemoryAuthStore::new();
        store.insert_user(user(3));
        let now = Utc::now();
        store
            .upsert_multi_device_session(Uuid::from_u128(3), "a", "hash", now)
            .await
            .unwrap();
        store
            .revoke_session(Uuid::from_u128(3), "a", now)
            .await
            .unwrap();
        assert!(
            store
                .find_session_by_device(Uuid::from_u128(3), "a")
                .await
                .unwrap()
                .is_none()
        );
    }
}
