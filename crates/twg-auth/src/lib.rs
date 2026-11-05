use thiserror::Error;
use twg_domain::SessionSubject;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthConfig {
    pub access_token_ttl_seconds: i64,
    pub refresh_token_ttl_seconds: i64,
    pub single_device_roles: Vec<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            access_token_ttl_seconds: 24 * 60 * 60,
            refresh_token_ttl_seconds: 7 * 24 * 60 * 60,
            single_device_roles: vec!["admin".to_owned(), "admin_satker".to_owned()],
        }
    }
}

impl AuthConfig {
    pub fn requires_single_device(&self, role: &str) -> bool {
        self.single_device_roles.iter().any(|value| value == role)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthContext {
    pub subject: SessionSubject,
}

impl AuthContext {
    pub const fn new(subject: SessionSubject) -> Self {
        Self { subject }
    }

    pub fn can_access_satker(&self, resource_satker_id: Option<Uuid>) -> bool {
        self.subject.can_access_satker(resource_satker_id)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("authorization header is not provided")]
    MissingToken,
    #[error("invalid authorization header format")]
    InvalidAuthorizationHeader,
    #[error("token has expired")]
    ExpiredToken,
    #[error("session has been revoked")]
    RevokedSession,
    #[error("device id is required")]
    MissingDeviceId,
    #[error("role is not allowed")]
    ForbiddenRole,
}

#[cfg(test)]
mod tests {
    use super::{AuthConfig, AuthContext, AuthError};
    use twg_domain::{Role, SessionSubject};
    use uuid::Uuid;

    #[test]
    fn default_single_device_roles_match_baseline() {
        let config = AuthConfig::default();
        assert!(config.requires_single_device("admin"));
        assert!(config.requires_single_device("admin_satker"));
        assert!(!config.requires_single_device("admin_monitor"));
        assert!(!config.requires_single_device("user"));
    }

    #[test]
    fn auth_context_delegates_satker_scope() {
        let actor = Uuid::from_u128(1);
        let context = AuthContext::new(SessionSubject {
            user_id: Uuid::from_u128(2),
            role: Role::UserSatker,
            satker_id: Some(actor),
        });

        assert!(context.can_access_satker(Some(actor)));
        assert!(!context.can_access_satker(None));
    }

    #[test]
    fn errors_have_stable_messages() {
        assert_eq!(
            AuthError::MissingToken.to_string(),
            "authorization header is not provided"
        );
    }
}
