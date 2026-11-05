use crate::AuthError;

/// Cost 10 keeps newly created hashes compatible with the legacy Go server,
/// which used `bcrypt.DefaultCost`.
const BCRYPT_COST: u32 = 10;
const MIN_PASSWORD_BYTES: usize = 8;
const MAX_PASSWORD_BYTES: usize = 72;

pub fn verify_password(password: &str, hash: &str) -> bool {
    bcrypt::verify(password, hash).unwrap_or(false)
}

pub fn hash_password(password: &str) -> Result<String, AuthError> {
    if password.len() < MIN_PASSWORD_BYTES {
        return Err(AuthError::WeakPassword);
    }
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(AuthError::PasswordTooLong);
    }
    bcrypt::hash(password, BCRYPT_COST).map_err(|_| AuthError::PasswordHash)
}

#[cfg(test)]
mod tests {
    use super::{hash_password, verify_password};
    use crate::AuthError;

    #[test]
    fn verifies_the_legacy_bcrypt_fixture_hash() {
        let hash = "$2a$10$7BuDXppm38Mpqv/04I3qQOjuBC4HqDGjjgBtUziquCOMW0iS2aV9m";
        assert!(verify_password("Baseline#2026", hash));
        assert!(!verify_password("wrong-password", hash));
    }

    #[test]
    fn hashes_new_passwords_with_bcrypt() {
        let hash = hash_password("NewPassword#2026").unwrap();
        assert!(verify_password("NewPassword#2026", &hash));
        assert!(!verify_password("OtherPassword#2026", &hash));
    }

    #[test]
    fn rejects_passwords_outside_bcrypt_bounds() {
        assert_eq!(hash_password("short"), Err(AuthError::WeakPassword));
        assert_eq!(
            hash_password(&"x".repeat(73)),
            Err(AuthError::PasswordTooLong)
        );
    }
}
