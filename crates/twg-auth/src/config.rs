use std::fmt;
use std::time::Duration;

use thiserror::Error;
use twg_domain::Role;

pub const TOKEN_KEY_BYTES: usize = 32;

#[derive(Clone, PartialEq, Eq)]
pub struct AuthConfig {
    pub token_symmetric_key: [u8; TOKEN_KEY_BYTES],
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
    pub single_device_roles: Vec<String>,
}

impl AuthConfig {
    pub fn new(token_symmetric_key: impl AsRef<[u8]>) -> Result<Self, AuthConfigError> {
        let key = token_symmetric_key.as_ref();
        if key.len() != TOKEN_KEY_BYTES {
            return Err(AuthConfigError::InvalidTokenKey(key.len()));
        }
        let mut token_symmetric_key = [0u8; TOKEN_KEY_BYTES];
        token_symmetric_key.copy_from_slice(key);
        Ok(Self {
            token_symmetric_key,
            access_token_ttl: Duration::from_secs(24 * 60 * 60),
            refresh_token_ttl: Duration::from_secs(7 * 24 * 60 * 60),
            single_device_roles: vec!["admin".to_owned(), "admin_satker".to_owned()],
        })
    }

    pub fn with_ttls(mut self, access: Duration, refresh: Duration) -> Self {
        self.access_token_ttl = access;
        self.refresh_token_ttl = refresh;
        self
    }

    pub fn with_single_device_roles(mut self, roles: Vec<String>) -> Self {
        self.single_device_roles = roles;
        self
    }

    pub fn requires_single_device(&self, role: Role) -> bool {
        self.requires_single_device_name(&role.to_string())
    }

    pub fn requires_single_device_name(&self, role: &str) -> bool {
        self.single_device_roles
            .iter()
            .any(|value| value.eq_ignore_ascii_case(role))
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            token_symmetric_key: [0x42; TOKEN_KEY_BYTES],
            access_token_ttl: Duration::from_secs(24 * 60 * 60),
            refresh_token_ttl: Duration::from_secs(7 * 24 * 60 * 60),
            single_device_roles: vec!["admin".to_owned(), "admin_satker".to_owned()],
        }
    }
}

impl fmt::Debug for AuthConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthConfig")
            .field("token_symmetric_key", &"<redacted>")
            .field("access_token_ttl", &self.access_token_ttl)
            .field("refresh_token_ttl", &self.refresh_token_ttl)
            .field("single_device_roles", &self.single_device_roles)
            .finish()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthConfigError {
    #[error("TOKEN_SYMMETRIC_KEY must be exactly 32 bytes, got {0}")]
    InvalidTokenKey(usize),
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{AuthConfig, AuthConfigError};
    use twg_domain::Role;

    #[test]
    fn default_single_device_roles_match_baseline() {
        let config = AuthConfig::default();
        assert!(config.requires_single_device(Role::Admin));
        assert!(config.requires_single_device(Role::AdminSatker));
        assert!(!config.requires_single_device(Role::AdminMonitor));
        assert!(!config.requires_single_device(Role::User));
    }

    #[test]
    fn token_key_must_be_32_bytes() {
        assert!(AuthConfig::new("short").is_err());
        assert_eq!(
            AuthConfig::new("short").unwrap_err(),
            AuthConfigError::InvalidTokenKey(5)
        );
        assert!(AuthConfig::new([7u8; 32]).is_ok());
    }

    #[test]
    fn debug_never_prints_the_key() {
        let config = AuthConfig::new([0xAB; 32]).unwrap();
        let rendered = format!("{config:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("171"));
    }

    #[test]
    fn ttls_can_be_overridden() {
        let config =
            AuthConfig::default().with_ttls(Duration::from_secs(60), Duration::from_secs(120));
        assert_eq!(config.access_token_ttl.as_secs(), 60);
        assert_eq!(config.refresh_token_ttl.as_secs(), 120);
    }
}
