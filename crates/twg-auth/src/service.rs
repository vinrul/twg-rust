use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use sha2::{Digest, Sha256};
use thiserror::Error;
use twg_contracts::auth::{LoginResponse, UserIdentity};
use twg_contracts::{Timestamp, Unit};
use twg_domain::SessionSubject;

use crate::password::{hash_password, verify_password};
use crate::store::{AuthStore, AuthStoreError, AuthUser, RotateOutcome, SessionRecord};
use crate::token::{ACCESS_TOKEN_TYPE, PasetoMaker, REFRESH_TOKEN_TYPE, TokenClaims};
use crate::{AuthConfig, AuthConfigError};

const WITA_OFFSET_SECONDS: i32 = 8 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthErrorKind {
    Invalid,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Internal,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("authorization header is not provided")]
    MissingToken,
    #[error("invalid authorization header format")]
    InvalidAuthorizationHeader,
    #[error("unsupported authorization type {0}")]
    UnsupportedAuthorizationType(String),
    #[error("token has expired")]
    ExpiredToken,
    #[error("token is invalid")]
    InvalidToken,
    #[error("refresh token is not allowed for authorization header")]
    RefreshTokenNotAllowed,
    #[error("token is not an access token")]
    NotAccessToken,
    #[error("invalid refresh token")]
    InvalidRefreshToken,
    #[error("invalid refresh token type")]
    InvalidRefreshTokenType,
    #[error("invalid refresh token subject")]
    InvalidRefreshTokenSubject,
    #[error("session revoked")]
    RevokedSession,
    #[error("device_id is required")]
    MissingDeviceId,
    #[error("already logged in on another device")]
    AlreadyLoggedIn,
    #[error("forbidden role")]
    ForbiddenRole,
    #[error("invalid password")]
    InvalidPassword,
    #[error("user not found")]
    UserNotFound,
    #[error("password must be at least 8 characters")]
    WeakPassword,
    #[error("password must be at most 72 bytes")]
    PasswordTooLong,
    #[error("password hashing failed")]
    PasswordHash,
    #[error("internal server error")]
    Internal(String),
}

impl AuthError {
    pub fn kind(&self) -> AuthErrorKind {
        match self {
            Self::MissingToken
            | Self::InvalidAuthorizationHeader
            | Self::UnsupportedAuthorizationType(_)
            | Self::ExpiredToken
            | Self::InvalidToken
            | Self::RefreshTokenNotAllowed
            | Self::NotAccessToken
            | Self::InvalidRefreshToken
            | Self::InvalidRefreshTokenType
            | Self::InvalidRefreshTokenSubject
            | Self::RevokedSession
            | Self::InvalidPassword => AuthErrorKind::Unauthorized,
            Self::MissingDeviceId | Self::WeakPassword | Self::PasswordTooLong => {
                AuthErrorKind::Invalid
            }
            Self::AlreadyLoggedIn | Self::ForbiddenRole => AuthErrorKind::Forbidden,
            Self::UserNotFound => AuthErrorKind::NotFound,
            Self::PasswordHash | Self::Internal(_) => AuthErrorKind::Internal,
        }
    }

    pub fn is_unauthorized(&self) -> bool {
        self.kind() == AuthErrorKind::Unauthorized
    }
}

impl From<AuthStoreError> for AuthError {
    fn from(error: AuthStoreError) -> Self {
        match error {
            AuthStoreError::NotFound => Self::UserNotFound,
            AuthStoreError::AlreadyLoggedIn => Self::AlreadyLoggedIn,
            AuthStoreError::Internal(message) => Self::Internal(message),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoginResult {
    pub response: LoginResponse,
    pub subject: SessionSubject,
}

#[derive(Clone)]
pub struct AuthService {
    store: Arc<dyn AuthStore>,
    config: AuthConfig,
    maker: PasetoMaker,
}

impl AuthService {
    pub fn new(store: Arc<dyn AuthStore>, config: AuthConfig) -> Self {
        let maker = PasetoMaker::new(&config);
        Self {
            store,
            config,
            maker,
        }
    }

    pub fn from_key(
        store: Arc<dyn AuthStore>,
        key: impl AsRef<[u8]>,
    ) -> Result<Self, AuthConfigError> {
        Ok(Self::new(store, AuthConfig::new(key)?))
    }

    pub fn config(&self) -> &AuthConfig {
        &self.config
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        device_id: &str,
    ) -> Result<LoginResult, AuthError> {
        let username = username.trim();
        let user = self
            .store
            .find_user_by_username(username)
            .await?
            .ok_or(AuthError::UserNotFound)?;
        if !verify_password(password, &user.password_hash) {
            return Err(AuthError::InvalidPassword);
        }
        let device_id = normalize_device_id(device_id)?;
        let now = Utc::now();
        let tokens = self.issue_tokens(&user, now)?;
        let refresh_hash = refresh_token_hash(&tokens.refresh_token, &self.config);

        if self.config.requires_single_device(user.role) {
            self.store
                .ensure_single_device_session(user.id, device_id, &refresh_hash, now)
                .await?;
        } else {
            self.store
                .upsert_multi_device_session(user.id, device_id, &refresh_hash, now)
                .await?;
        }

        LoginResult::from_tokens(user, tokens)
    }

    pub async fn refresh(
        &self,
        refresh_token: &str,
        device_id: &str,
    ) -> Result<LoginResult, AuthError> {
        let now = Utc::now();
        let claims = self.maker.verify_token(refresh_token, now)?;
        self.validate_refresh_claims(&claims)?;
        let user = self
            .store
            .find_user_by_id(claims.id)
            .await?
            .ok_or(AuthError::InvalidRefreshTokenSubject)?;
        if user.username != claims.username || user.role != claims.role {
            return Err(AuthError::InvalidRefreshTokenSubject);
        }
        let device_id = normalize_device_id(device_id)?;
        let tokens = self.issue_tokens(&user, now)?;
        let old_hash = refresh_token_hash(refresh_token, &self.config);
        let new_hash = refresh_token_hash(&tokens.refresh_token, &self.config);

        match self
            .store
            .rotate_refresh_session(user.id, device_id, &old_hash, &new_hash, now)
            .await?
        {
            RotateOutcome::Rotated(_) => LoginResult::from_tokens(user, tokens),
            RotateOutcome::Revoked => Err(AuthError::RevokedSession),
            RotateOutcome::HashMismatch => Err(AuthError::InvalidRefreshToken),
        }
    }

    /// Refreshes the identity payload and rotates the session refresh hash so
    /// the new refresh token remains usable.
    pub async fn identify(
        &self,
        subject: &SessionSubject,
        device_id: &str,
    ) -> Result<LoginResult, AuthError> {
        let user = self
            .store
            .find_user_by_id(subject.user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;
        let device_id = normalize_device_id(device_id)?;
        let now = Utc::now();
        let tokens = self.issue_tokens(&user, now)?;
        let refresh_hash = refresh_token_hash(&tokens.refresh_token, &self.config);
        if self.config.requires_single_device(user.role) {
            self.store
                .ensure_single_device_session(user.id, device_id, &refresh_hash, now)
                .await?;
        } else {
            self.store
                .upsert_multi_device_session(user.id, device_id, &refresh_hash, now)
                .await?;
        }
        LoginResult::from_tokens(user, tokens)
    }

    pub async fn logout(&self, subject: &SessionSubject, device_id: &str) -> Result<(), AuthError> {
        let device_id = normalize_device_id(device_id)?;
        self.store
            .revoke_session(subject.user_id, device_id, Utc::now())
            .await?;
        Ok(())
    }

    pub async fn change_password(
        &self,
        subject: &SessionSubject,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        let user = self
            .store
            .find_user_by_id(subject.user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;
        if !verify_password(old_password, &user.password_hash) {
            return Err(AuthError::InvalidPassword);
        }
        let password_hash = hash_password(new_password)?;
        self.store
            .update_password(user.id, &password_hash, Utc::now())
            .await?;
        Ok(())
    }

    pub fn authenticate_access(&self, token: &str) -> Result<SessionSubject, AuthError> {
        let now = Utc::now();
        let claims = self.maker.verify_token(token, now)?;
        self.validate_access_claims(&claims)?;
        Ok(SessionSubject {
            user_id: claims.id,
            role: claims.role,
            satker_id: claims.satker_id,
        })
    }

    pub async fn validate_session(
        &self,
        subject: &SessionSubject,
        device_id: &str,
    ) -> Result<SessionRecord, AuthError> {
        let device_id = normalize_device_id(device_id)?;
        let session = self
            .store
            .find_session_by_device(subject.user_id, device_id)
            .await?
            .ok_or(AuthError::RevokedSession)?;
        self.store.touch_session(session.id, Utc::now()).await?;
        Ok(session)
    }

    fn issue_tokens(&self, user: &AuthUser, now: DateTime<Utc>) -> Result<IssuedTokens, AuthError> {
        let (access_token, access_claims) = self.maker.create_token(
            user.id,
            &user.username,
            user.role,
            user.satker_id,
            ACCESS_TOKEN_TYPE,
            self.config.access_token_ttl,
            now,
        )?;
        let (refresh_token, refresh_claims) = self.maker.create_token(
            user.id,
            &user.username,
            user.role,
            user.satker_id,
            REFRESH_TOKEN_TYPE,
            self.config.refresh_token_ttl,
            now,
        )?;
        Ok(IssuedTokens {
            access_token,
            refresh_token,
            access_expires_at: to_wita(access_claims.expired_at),
            refresh_expires_at: to_wita(refresh_claims.expired_at),
        })
    }

    fn validate_access_claims(&self, claims: &TokenClaims) -> Result<(), AuthError> {
        if claims.token_type == REFRESH_TOKEN_TYPE {
            return Err(AuthError::RefreshTokenNotAllowed);
        }
        if claims.token_type.is_empty() {
            let max_ttl = i64::try_from(self.config.access_token_ttl.as_secs()).unwrap_or(i64::MAX);
            if claims.ttl_seconds() > max_ttl + 60 {
                return Err(AuthError::NotAccessToken);
            }
            return Ok(());
        }
        if claims.token_type == ACCESS_TOKEN_TYPE {
            return Ok(());
        }
        Err(AuthError::NotAccessToken)
    }

    fn validate_refresh_claims(&self, claims: &TokenClaims) -> Result<(), AuthError> {
        if claims.token_type == REFRESH_TOKEN_TYPE {
            return Ok(());
        }
        if claims.token_type.is_empty() {
            let expected =
                i64::try_from(self.config.refresh_token_ttl.as_secs()).unwrap_or(i64::MAX);
            if claims.ttl_seconds() < expected.saturating_sub(60) {
                return Err(AuthError::InvalidRefreshTokenType);
            }
            return Ok(());
        }
        Err(AuthError::InvalidRefreshTokenType)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IssuedTokens {
    access_token: String,
    refresh_token: String,
    access_expires_at: Timestamp,
    refresh_expires_at: Timestamp,
}

impl LoginResult {
    fn from_tokens(user: AuthUser, tokens: IssuedTokens) -> Result<Self, AuthError> {
        let subject = SessionSubject {
            user_id: user.id,
            role: user.role,
            satker_id: user.satker_id,
        };
        Ok(Self {
            response: LoginResponse {
                user: user_identity(&user),
                access_token: tokens.access_token,
                refresh_token: tokens.refresh_token,
                access_token_expires_at: tokens.access_expires_at,
                refresh_token_expires_at: tokens.refresh_expires_at,
            },
            subject,
        })
    }
}

pub fn user_identity(user: &AuthUser) -> UserIdentity {
    let units = serde_json::from_str::<Vec<Unit>>(&user.units).unwrap_or_default();
    UserIdentity {
        id: user.id,
        name: user.name.clone(),
        username: user.username.clone(),
        role: user.role,
        satker_id: user.satker_id,
        profile_img: user.profile_img.clone(),
        units,
        created_at: to_wita(user.created_at),
        updated_at: to_wita(user.updated_at),
    }
}

pub fn refresh_token_hash(refresh_token: &str, config: &AuthConfig) -> String {
    let mut hasher = Sha256::new();
    hasher.update(refresh_token.as_bytes());
    hasher.update(b":");
    hasher.update(config.token_symmetric_key);
    hex::encode(hasher.finalize())
}

fn normalize_device_id(device_id: &str) -> Result<&str, AuthError> {
    let device_id = device_id.trim();
    if device_id.is_empty() {
        return Err(AuthError::MissingDeviceId);
    }
    Ok(device_id)
}

fn to_wita(value: DateTime<Utc>) -> Timestamp {
    let offset = FixedOffset::east_opt(WITA_OFFSET_SECONDS).expect("valid WITA offset");
    value.with_timezone(&offset)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::Utc;
    use twg_domain::Role;
    use uuid::Uuid;

    use super::{AuthError, AuthService};
    use crate::password::hash_password;
    use crate::store::{AuthStoreError, AuthUser, InMemoryAuthStore};
    use crate::{AuthConfig, PasetoMaker};

    const LEGACY_HASH: &str = "$2a$10$7BuDXppm38Mpqv/04I3qQOjuBC4HqDGjjgBtUziquCOMW0iS2aV9m";

    fn seeded_store() -> InMemoryAuthStore {
        let store = InMemoryAuthStore::new();
        let now = Utc::now();
        let roles = [
            (1, "baselineadmin", Role::Admin, None),
            (2, "baselinemonitor", Role::AdminMonitor, None),
            (
                3,
                "baselineadminsatker",
                Role::AdminSatker,
                Some(Uuid::from_u128(11)),
            ),
            (
                4,
                "baselineusersatker",
                Role::UserSatker,
                Some(Uuid::from_u128(11)),
            ),
            (5, "baselineuser", Role::User, None),
        ];
        for (id, username, role, satker_id) in roles {
            store.insert_user(AuthUser {
                id: Uuid::from_u128(id),
                name: username.to_owned(),
                username: username.to_owned(),
                password_hash: LEGACY_HASH.to_owned(),
                role,
                satker_id,
                profile_img: String::new(),
                units: "[]".to_owned(),
                created_at: now,
                updated_at: now,
            });
        }
        store
    }

    fn service(store: InMemoryAuthStore) -> AuthService {
        AuthService::new(Arc::new(store), AuthConfig::default())
    }

    #[tokio::test]
    async fn logs_in_every_baseline_role() {
        let store = seeded_store();
        let service = service(store);
        for username in [
            "baselineadmin",
            "baselinemonitor",
            "baselineadminsatker",
            "baselineusersatker",
            "baselineuser",
        ] {
            let result = service
                .login(username, "Baseline#2026", "device-1")
                .await
                .unwrap();
            assert_eq!(result.response.user.username, username);
            assert!(result.response.access_token.starts_with("v2.local."));
            assert_eq!(
                result.response.user.created_at.offset().local_minus_utc(),
                8 * 60 * 60
            );
        }
    }

    #[tokio::test]
    async fn rejects_wrong_password_and_missing_device() {
        let service = service(seeded_store());
        assert_eq!(
            service
                .login("baselineadmin", "wrong-password", "device-1")
                .await
                .unwrap_err(),
            AuthError::InvalidPassword
        );
        assert_eq!(
            service
                .login("baselineadmin", "Baseline#2026", "  ")
                .await
                .unwrap_err(),
            AuthError::MissingDeviceId
        );
    }

    #[tokio::test]
    async fn single_device_roles_reject_a_second_device() {
        let service = service(seeded_store());
        service
            .login("baselineadmin", "Baseline#2026", "device-1")
            .await
            .unwrap();
        assert_eq!(
            service
                .login("baselineadmin", "Baseline#2026", "device-2")
                .await
                .unwrap_err(),
            AuthError::AlreadyLoggedIn
        );
    }

    #[tokio::test]
    async fn refresh_rotates_once_and_reuse_is_rejected() {
        let service = service(seeded_store());
        let login = service
            .login("baselineuser", "Baseline#2026", "device-1")
            .await
            .unwrap();
        let refreshed = service
            .refresh(&login.response.refresh_token, "device-1")
            .await
            .unwrap();
        assert_ne!(
            login.response.refresh_token,
            refreshed.response.refresh_token
        );
        assert_eq!(
            service
                .refresh(&login.response.refresh_token, "device-1")
                .await
                .unwrap_err(),
            AuthError::InvalidRefreshToken
        );
    }

    #[tokio::test]
    async fn access_and_refresh_tokens_cannot_be_swapped() {
        let service = service(seeded_store());
        let login = service
            .login("baselineuser", "Baseline#2026", "device-1")
            .await
            .unwrap();
        assert_eq!(
            service
                .refresh(&login.response.access_token, "device-1")
                .await
                .unwrap_err(),
            AuthError::InvalidRefreshTokenType
        );
        assert_eq!(
            service
                .authenticate_access(&login.response.refresh_token)
                .unwrap_err(),
            AuthError::RefreshTokenNotAllowed
        );
    }

    #[tokio::test]
    async fn logout_revokes_the_session() {
        let service = service(seeded_store());
        let login = service
            .login("baselineuser", "Baseline#2026", "device-1")
            .await
            .unwrap();
        service.logout(&login.subject, "device-1").await.unwrap();
        assert_eq!(
            service
                .validate_session(&login.subject, "device-1")
                .await
                .unwrap_err(),
            AuthError::RevokedSession
        );
    }

    #[tokio::test]
    async fn identify_rotates_the_session_hash() {
        let store = seeded_store();
        let service = service(store.clone());
        let login = service
            .login("baselineuser", "Baseline#2026", "device-1")
            .await
            .unwrap();
        let identified = service.identify(&login.subject, "device-1").await.unwrap();
        assert_eq!(
            service
                .refresh(&login.response.refresh_token, "device-1")
                .await
                .unwrap_err(),
            AuthError::InvalidRefreshToken
        );
        service
            .refresh(&identified.response.refresh_token, "device-1")
            .await
            .unwrap();
        assert_eq!(store.active_session_count(), 1);
    }

    #[tokio::test]
    async fn change_password_verifies_the_old_password() {
        let service = service(seeded_store());
        let login = service
            .login("baselineuser", "Baseline#2026", "device-1")
            .await
            .unwrap();
        assert_eq!(
            service
                .change_password(&login.subject, "wrong-password", "NewPassword#2026")
                .await
                .unwrap_err(),
            AuthError::InvalidPassword
        );
        service
            .change_password(&login.subject, "Baseline#2026", "NewPassword#2026")
            .await
            .unwrap();
        service
            .login("baselineuser", "NewPassword#2026", "device-1")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn tokens_do_not_carry_the_refresh_hash() {
        let config = AuthConfig::default();
        let maker = PasetoMaker::new(&config);
        let (token, claims) = maker
            .create_token(
                Uuid::from_u128(1),
                "baselineuser",
                Role::User,
                None,
                "refresh",
                Duration::from_secs(60),
                Utc::now(),
            )
            .unwrap();
        assert!(!token.contains(&claims.id.to_string()));
        assert!(!token.contains("Baseline#2026"));
        let hash = super::refresh_token_hash(&token, &config);
        assert_eq!(hash.len(), 64);
        assert!(!hash.contains(&token[..16]));
    }

    #[test]
    fn store_errors_map_to_auth_errors() {
        assert_eq!(
            AuthError::from(AuthStoreError::NotFound),
            AuthError::UserNotFound
        );
        assert_eq!(
            AuthError::from(AuthStoreError::AlreadyLoggedIn),
            AuthError::AlreadyLoggedIn
        );
        let _ = hash_password("Baseline#2026").unwrap();
    }
}
