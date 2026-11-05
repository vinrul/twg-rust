use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use twg_domain::Role;
use uuid::Uuid;

use crate::{AuthConfig, AuthError};

pub const ACCESS_TOKEN_TYPE: &str = "access";
pub const REFRESH_TOKEN_TYPE: &str = "refresh";

const HEADER: &[u8] = b"v2.local.";
const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenClaims {
    pub id: Uuid,
    pub username: String,
    pub role: Role,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<Uuid>,
    #[serde(default)]
    pub token_type: String,
    pub issued_at: DateTime<Utc>,
    pub expired_at: DateTime<Utc>,
    /// Legacy Go tokens embed `exp` through `jwt.RegisteredClaims`.
    #[serde(default)]
    pub exp: i64,
}

impl TokenClaims {
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expired_at <= now || (self.exp > 0 && self.exp <= now.timestamp())
    }

    pub fn ttl_seconds(&self) -> i64 {
        (self.expired_at - self.issued_at).num_seconds()
    }
}

#[derive(Clone)]
pub struct PasetoMaker {
    key: [u8; 32],
}

impl PasetoMaker {
    pub fn new(config: &AuthConfig) -> Self {
        Self {
            key: config.token_symmetric_key,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_token(
        &self,
        user_id: Uuid,
        username: &str,
        role: Role,
        satker_id: Option<Uuid>,
        token_type: &str,
        ttl: Duration,
        now: DateTime<Utc>,
    ) -> Result<(String, TokenClaims), AuthError> {
        let ttl_seconds = i64::try_from(ttl.as_secs()).unwrap_or(i64::MAX);
        let expired_at = now + chrono::Duration::seconds(ttl_seconds);
        let claims = TokenClaims {
            id: user_id,
            username: username.to_owned(),
            role,
            satker_id,
            token_type: token_type.to_owned(),
            issued_at: now,
            expired_at,
            exp: expired_at.timestamp(),
        };
        let mut nonce = [0u8; NONCE_BYTES];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let token = self.encrypt(&claims, nonce)?;
        Ok((token, claims))
    }

    pub fn verify_token(&self, token: &str, now: DateTime<Utc>) -> Result<TokenClaims, AuthError> {
        let claims = self.decrypt(token)?;
        if claims.is_expired(now) {
            return Err(AuthError::ExpiredToken);
        }
        Ok(claims)
    }

    fn encrypt(&self, claims: &TokenClaims, nonce: [u8; NONCE_BYTES]) -> Result<String, AuthError> {
        let plaintext = serde_json::to_vec(claims).map_err(|_| AuthError::InvalidToken)?;
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let associated_data = pre_auth_encode(&[HEADER, &nonce, &[]]);
        let ciphertext = cipher
            .encrypt(
                &XNonce::from(nonce),
                chacha20poly1305::aead::Payload {
                    msg: &plaintext,
                    aad: &associated_data,
                },
            )
            .map_err(|_| AuthError::InvalidToken)?;
        let mut body = Vec::with_capacity(NONCE_BYTES + ciphertext.len());
        body.extend_from_slice(&nonce);
        body.extend_from_slice(&ciphertext);
        Ok(format!(
            "{}{}",
            std::str::from_utf8(HEADER).unwrap_or("v2.local."),
            URL_SAFE_NO_PAD.encode(body)
        ))
    }

    fn decrypt(&self, token: &str) -> Result<TokenClaims, AuthError> {
        let plaintext = self.decrypt_plaintext(token)?;
        serde_json::from_slice(&plaintext).map_err(|error| {
            tracing::warn!(%error, "failed to parse PASETO claims");
            AuthError::InvalidToken
        })
    }

    fn decrypt_plaintext(&self, token: &str) -> Result<Vec<u8>, AuthError> {
        let rest = token
            .strip_prefix("v2.local.")
            .ok_or(AuthError::InvalidToken)?;
        if rest.is_empty() {
            return Err(AuthError::InvalidToken);
        }
        let (encoded_body, footer) = match rest.split_once('.') {
            Some((body, footer)) => {
                if footer.contains('.') {
                    return Err(AuthError::InvalidToken);
                }
                let footer = URL_SAFE_NO_PAD
                    .decode(footer)
                    .map_err(|_| AuthError::InvalidToken)?;
                (body, footer)
            }
            None => (rest, Vec::new()),
        };
        let body = URL_SAFE_NO_PAD
            .decode(encoded_body)
            .map_err(|_| AuthError::InvalidToken)?;
        if body.len() < NONCE_BYTES + TAG_BYTES {
            return Err(AuthError::InvalidToken);
        }
        let (nonce, ciphertext) = body.split_at(NONCE_BYTES);
        let nonce: [u8; NONCE_BYTES] = nonce.try_into().map_err(|_| AuthError::InvalidToken)?;
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let associated_data = pre_auth_encode(&[HEADER, &nonce, &footer]);
        cipher
            .decrypt(
                &XNonce::from(nonce),
                chacha20poly1305::aead::Payload {
                    msg: ciphertext,
                    aad: &associated_data,
                },
            )
            .map_err(|_| {
                tracing::debug!("PASETO authentication failed");
                AuthError::InvalidToken
            })
    }
}

/// PASETO v2 `preAuthEncode`: little-endian piece count followed by each
/// piece's length and bytes. The Go `o1egl/paseto` implementation binds the
/// header, nonce, and footer with this layout.
fn pre_auth_encode(pieces: &[&[u8]]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(&(pieces.len() as i64).to_le_bytes());
    for piece in pieces {
        output.extend_from_slice(&(piece.len() as i64).to_le_bytes());
        output.extend_from_slice(piece);
    }
    output
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use base64::Engine as _;
    use chacha20poly1305::aead::Aead as _;
    use chacha20poly1305::{KeyInit as _, XChaCha20Poly1305, XNonce};
    use chrono::{Duration as ChronoDuration, Utc};
    use twg_domain::Role;
    use uuid::Uuid;

    use super::{ACCESS_TOKEN_TYPE, PasetoMaker, REFRESH_TOKEN_TYPE};
    use crate::{AuthConfig, AuthError};

    fn maker() -> PasetoMaker {
        PasetoMaker::new(&AuthConfig::default())
    }

    #[test]
    fn v2_local_round_trips_claims() {
        let maker = maker();
        let now = Utc::now();
        let (token, created) = maker
            .create_token(
                Uuid::from_u128(7),
                "baselineadmin",
                Role::Admin,
                None,
                ACCESS_TOKEN_TYPE,
                Duration::from_secs(60),
                now,
            )
            .unwrap();
        assert!(token.starts_with("v2.local."));
        let verified = maker.verify_token(&token, now).unwrap();
        assert_eq!(verified, created);
        assert_eq!(verified.role, Role::Admin);
        assert_eq!(verified.exp, created.expired_at.timestamp());
    }

    #[test]
    fn accepts_legacy_null_footer_tokens() {
        let config = AuthConfig::default();
        let maker = PasetoMaker::new(&config);
        let now = Utc::now();
        let claims = super::TokenClaims {
            id: Uuid::from_u128(7),
            username: "baselineuser".to_owned(),
            role: Role::User,
            satker_id: None,
            token_type: ACCESS_TOKEN_TYPE.to_owned(),
            issued_at: now,
            expired_at: now + ChronoDuration::hours(1),
            exp: (now + ChronoDuration::hours(1)).timestamp(),
        };
        let plaintext = serde_json::to_vec(&claims).unwrap();
        let nonce = [7u8; super::NONCE_BYTES];
        let footer = b"null";
        let associated_data = super::pre_auth_encode(&[super::HEADER, &nonce, footer]);
        let cipher = XChaCha20Poly1305::new((&config.token_symmetric_key).into());
        let ciphertext = cipher
            .encrypt(
                &XNonce::from(nonce),
                chacha20poly1305::aead::Payload {
                    msg: &plaintext,
                    aad: &associated_data,
                },
            )
            .unwrap();
        let mut body = nonce.to_vec();
        body.extend_from_slice(&ciphertext);
        let token = format!(
            "v2.local.{}.{}",
            super::URL_SAFE_NO_PAD.encode(body),
            super::URL_SAFE_NO_PAD.encode(footer)
        );
        let verified = maker.verify_token(&token, now).unwrap();
        assert_eq!(verified, claims);
    }

    #[test]
    fn expired_tokens_are_rejected() {
        let maker = maker();
        let now = Utc::now();
        let (token, _) = maker
            .create_token(
                Uuid::from_u128(7),
                "baselineadmin",
                Role::Admin,
                None,
                ACCESS_TOKEN_TYPE,
                Duration::from_secs(1),
                now - ChronoDuration::seconds(10),
            )
            .unwrap();
        assert_eq!(
            maker.verify_token(&token, now).unwrap_err(),
            AuthError::ExpiredToken
        );
    }

    #[test]
    fn tampered_tokens_are_rejected() {
        let maker = maker();
        let now = Utc::now();
        let (token, _) = maker
            .create_token(
                Uuid::from_u128(7),
                "baselineadmin",
                Role::Admin,
                None,
                REFRESH_TOKEN_TYPE,
                Duration::from_secs(60),
                now,
            )
            .unwrap();
        let mut bytes = token.into_bytes();
        let index = bytes.len() - 2;
        bytes[index] = if bytes[index] == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(bytes).unwrap();
        assert!(maker.verify_token(&tampered, now).is_err());
    }

    #[test]
    fn wrong_key_cannot_decrypt() {
        let now = Utc::now();
        let (token, _) = maker()
            .create_token(
                Uuid::from_u128(7),
                "baselineadmin",
                Role::Admin,
                None,
                ACCESS_TOKEN_TYPE,
                Duration::from_secs(60),
                now,
            )
            .unwrap();
        let other = PasetoMaker::new(&AuthConfig::new([9u8; 32]).unwrap());
        assert_eq!(
            other.verify_token(&token, now).unwrap_err(),
            AuthError::InvalidToken
        );
    }
}
