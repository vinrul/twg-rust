use std::env;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;
use twg_auth::AuthConfig;

use crate::assets::ConvertConfig;

pub const DEFAULT_ADDRESS: &str = "127.0.0.1:8010";
pub const DEFAULT_MAX_BODY_BYTES: usize = 100 * 1024 * 1024;
pub const DEFAULT_RATE_LIMIT: u32 = 10;
pub const DEFAULT_RATE_WINDOW_SECONDS: u64 = 60;
pub const DEFAULT_WS_TICKET_TTL_SECONDS: u64 = 30;
pub const DEFAULT_WS_HISTORY_LIMIT: usize = 256;
pub const DEFAULT_ASSETS_DIR: &str = "asset-store";
pub const DEFAULT_GLTF_TRANSFORM: &str = "gltf-transform";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitConfig {
    pub attempts: u32,
    pub window: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            attempts: DEFAULT_RATE_LIMIT,
            window: Duration::from_secs(DEFAULT_RATE_WINDOW_SECONDS),
        }
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub address: String,
    pub database_url: String,
    pub cors_domains: Vec<String>,
    pub max_body_bytes: usize,
    pub uploads_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub convert: ConvertConfig,
    pub auth: AuthConfig,
    pub rate_limit: RateLimitConfig,
    pub ws_ticket_ttl: Duration,
    pub ws_history_limit: usize,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();
        let _ = dotenvy::from_filename("app.env");
        if let Ok(path) = env::var("APP_ENV_FILE") {
            dotenvy::from_filename(path)?;
        }

        let database_url = env::var("DB_SOURCE")
            .or_else(|_| env::var("DATABASE_URL"))
            .map_err(|_| ConfigError::Missing("DB_SOURCE or DATABASE_URL"))?;
        let token_key = env::var("TOKEN_SYMMETRIC_KEY")
            .map_err(|_| ConfigError::Missing("TOKEN_SYMMETRIC_KEY"))?;
        let mut auth = AuthConfig::new(token_key.as_bytes())?;
        if let Ok(value) = env::var("ACCESS_TOKEN_DURATION") {
            auth.access_token_ttl = parse_duration(&value)?;
        }
        if let Ok(value) = env::var("REFRESH_TOKEN_DURATION") {
            auth.refresh_token_ttl = parse_duration(&value)?;
        }
        if let Ok(value) = env::var("SINGLE_DEVICE_ROLES") {
            auth.single_device_roles = parse_list(&value);
        }

        let cors_domains = env::var("CORS_DOMAIN")
            .map(|value| parse_list(&value))
            .unwrap_or_else(|_| vec!["http://localhost:5173".to_owned()]);
        if cors_domains.is_empty() {
            return Err(ConfigError::Empty("CORS_DOMAIN"));
        }

        let max_body_bytes = env::var("MAX_BODY_BYTES")
            .ok()
            .map(|value| value.parse::<usize>())
            .transpose()?
            .unwrap_or(DEFAULT_MAX_BODY_BYTES)
            .max(1_024);
        let uploads_dir = env::var("UPLOADS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("uploads"));
        let assets_dir = env::var("ASSETS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_ASSETS_DIR));
        let convert = ConvertConfig {
            gltf_transform: env::var("GLTF_TRANSFORM_BIN")
                .unwrap_or_else(|_| DEFAULT_GLTF_TRANSFORM.to_owned()),
            lod2_ratio: env::var("ASSET_LOD2_RATIO")
                .ok()
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(0.5)
                .clamp(0.01, 1.0),
            lod2_error: env::var("ASSET_LOD2_ERROR")
                .ok()
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(0.001)
                .clamp(0.00001, 100.0),
        };
        let attempts = env::var("AUTH_RATE_LIMIT")
            .ok()
            .map(|value| value.parse::<u32>())
            .transpose()?
            .unwrap_or(DEFAULT_RATE_LIMIT);
        let window_seconds = env::var("AUTH_RATE_WINDOW_SECONDS")
            .ok()
            .map(|value| value.parse::<u64>())
            .transpose()?
            .unwrap_or(DEFAULT_RATE_WINDOW_SECONDS)
            .max(1);
        let ws_ticket_ttl = env::var("WS_TICKET_TTL_SECONDS")
            .ok()
            .map(|value| value.parse::<u64>())
            .transpose()?
            .unwrap_or(DEFAULT_WS_TICKET_TTL_SECONDS)
            .clamp(5, 300);
        let ws_history_limit = env::var("WS_HISTORY_LIMIT")
            .ok()
            .map(|value| value.parse::<usize>())
            .transpose()?
            .unwrap_or(DEFAULT_WS_HISTORY_LIMIT)
            .max(16);

        Ok(Self {
            address: env::var("HTTP_SERVER_ADDRESS").unwrap_or_else(|_| DEFAULT_ADDRESS.to_owned()),
            database_url,
            cors_domains,
            max_body_bytes,
            uploads_dir,
            assets_dir,
            convert,
            auth,
            rate_limit: RateLimitConfig {
                attempts,
                window: Duration::from_secs(window_seconds),
            },
            ws_ticket_ttl: Duration::from_secs(ws_ticket_ttl),
            ws_history_limit,
        })
    }
}

impl std::fmt::Debug for ServerConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServerConfig")
            .field("address", &self.address)
            .field("database_url", &"<redacted>")
            .field("cors_domains", &self.cors_domains)
            .field("max_body_bytes", &self.max_body_bytes)
            .field("uploads_dir", &self.uploads_dir)
            .field("assets_dir", &self.assets_dir)
            .field("convert", &self.convert)
            .field("auth", &self.auth)
            .field("rate_limit", &self.rate_limit)
            .field("ws_ticket_ttl", &self.ws_ticket_ttl)
            .field("ws_history_limit", &self.ws_history_limit)
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing environment variable: {0}")]
    Missing(&'static str),
    #[error("empty environment variable: {0}")]
    Empty(&'static str),
    #[error("invalid duration: {0}")]
    InvalidDuration(String),
    #[error("invalid number: {0}")]
    InvalidNumber(#[from] std::num::ParseIntError),
    #[error("invalid auth config: {0}")]
    Auth(#[from] twg_auth::AuthConfigError),
    #[error("cannot load environment file: {0}")]
    EnvFile(#[from] dotenvy::Error),
}

pub fn parse_duration(value: &str) -> Result<Duration, ConfigError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ConfigError::InvalidDuration(value.to_owned()));
    }
    let (number, multiplier) = match value.as_bytes().last().copied() {
        Some(b's') => (&value[..value.len() - 1], 1),
        Some(b'm') => (&value[..value.len() - 1], 60),
        Some(b'h') => (&value[..value.len() - 1], 60 * 60),
        Some(b'd') => (&value[..value.len() - 1], 24 * 60 * 60),
        _ => (value, 1),
    };
    let number = number
        .parse::<u64>()
        .map_err(|_| ConfigError::InvalidDuration(value.to_owned()))?;
    Ok(Duration::from_secs(number.saturating_mul(multiplier)))
}

pub fn parse_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_duration, parse_list};
    use std::time::Duration;

    #[test]
    fn parses_go_style_durations() {
        assert_eq!(parse_duration("24h").unwrap(), Duration::from_secs(86_400));
        assert_eq!(
            parse_duration("168h").unwrap(),
            Duration::from_secs(604_800)
        );
        assert_eq!(parse_duration("90m").unwrap(), Duration::from_secs(5_400));
        assert_eq!(parse_duration("45s").unwrap(), Duration::from_secs(45));
        assert_eq!(parse_duration("7d").unwrap(), Duration::from_secs(604_800));
        assert_eq!(parse_duration("30").unwrap(), Duration::from_secs(30));
    }

    #[test]
    fn rejects_invalid_durations() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("12x").is_err());
    }

    #[test]
    fn parses_comma_lists() {
        assert_eq!(
            parse_list(" admin, admin_satker , "),
            vec!["admin".to_owned(), "admin_satker".to_owned()]
        );
        assert!(parse_list(" , ").is_empty());
    }
}
