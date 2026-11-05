mod config;
mod password;
mod service;
mod store;
mod token;

use twg_domain::SessionSubject;
use uuid::Uuid;

pub use config::{AuthConfig, AuthConfigError, TOKEN_KEY_BYTES};
pub use password::{hash_password, verify_password};
pub use service::{
    AuthError, AuthErrorKind, AuthService, LoginResult, refresh_token_hash, user_identity,
};
pub use store::{
    AuthStore, AuthStoreError, AuthUser, InMemoryAuthStore, RotateOutcome, SessionRecord,
};
pub use token::{ACCESS_TOKEN_TYPE, PasetoMaker, REFRESH_TOKEN_TYPE, TokenClaims};

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn require_roles(&self, roles: &[twg_domain::Role]) -> Result<(), AuthError> {
        if roles.is_empty() || roles.contains(&self.subject.role) {
            return Ok(());
        }
        Err(AuthError::ForbiddenRole)
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthConfig, AuthContext, AuthError};
    use twg_domain::{Role, SessionSubject};
    use uuid::Uuid;

    #[test]
    fn default_single_device_roles_match_baseline() {
        let config = AuthConfig::default();
        assert!(config.requires_single_device(Role::Admin));
        assert!(config.requires_single_device(Role::AdminSatker));
        assert!(!config.requires_single_device(Role::AdminMonitor));
        assert!(!config.requires_single_device(Role::User));
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
    fn auth_context_checks_roles() {
        let context = AuthContext::new(SessionSubject {
            user_id: Uuid::from_u128(2),
            role: Role::AdminSatker,
            satker_id: Some(Uuid::from_u128(1)),
        });
        assert!(
            context
                .require_roles(&[Role::Admin, Role::AdminSatker])
                .is_ok()
        );
        assert_eq!(
            context.require_roles(&[Role::Admin]),
            Err(AuthError::ForbiddenRole)
        );
    }

    #[test]
    fn errors_have_stable_messages() {
        assert_eq!(
            AuthError::MissingToken.to_string(),
            "authorization header is not provided"
        );
    }
}
