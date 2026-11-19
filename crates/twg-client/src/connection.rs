//! Per-computer server connection choice for the desktop client.
//!
//! The public server is fixed (address intentionally not shown in the UI); the
//! local option points at a server the user runs on their own machine or LAN.
//! Settings are stored next to the session file so each computer keeps its own
//! choice.

use serde::{Deserialize, Serialize};

pub const PUBLIC_HOST: &str = "103.179.218.76";
pub const PUBLIC_PORT: u16 = 8122;
pub const DEFAULT_LOCAL_HOST: &str = "127.0.0.1";
pub const DEFAULT_LOCAL_PORT: u16 = 8122;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionMode {
    #[default]
    Public,
    Local,
}

impl ConnectionMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Public => "Publik",
            Self::Local => "Lokal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionSettings {
    pub mode: ConnectionMode,
    pub local_host: String,
    pub local_port: u16,
}

impl Default for ConnectionSettings {
    fn default() -> Self {
        Self {
            mode: ConnectionMode::Public,
            local_host: DEFAULT_LOCAL_HOST.to_owned(),
            local_port: DEFAULT_LOCAL_PORT,
        }
    }
}

impl ConnectionSettings {
    pub fn public_base_url() -> String {
        format!("http://{PUBLIC_HOST}:{PUBLIC_PORT}")
    }

    pub fn local_base_url(&self) -> String {
        format!("http://{}:{}", self.local_host.trim(), self.local_port)
    }

    pub fn base_url(&self) -> String {
        match self.mode {
            ConnectionMode::Public => Self::public_base_url(),
            ConnectionMode::Local => self.local_base_url(),
        }
    }

    /// Cleans up values loaded from disk or typed into the form: strips a
    /// pasted scheme/trailing slash and falls back to the defaults when the
    /// local address is unusable.
    pub fn normalized(mut self) -> Self {
        self.local_host = normalize_host(&self.local_host);
        if self.local_host.is_empty() {
            self.local_host = DEFAULT_LOCAL_HOST.to_owned();
        }
        if self.local_port == 0 {
            self.local_port = DEFAULT_LOCAL_PORT;
        }
        self
    }

    /// Text safe to show in the UI; the public address is never revealed.
    pub fn label(&self) -> String {
        match self.mode {
            ConnectionMode::Public => "Publik".to_owned(),
            ConnectionMode::Local => {
                let local = self.clone().normalized();
                format!("Lokal ({}:{})", local.local_host, local.local_port)
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn default_path() -> std::path::PathBuf {
        if let Ok(path) = std::env::var("TWG_CONNECTION_FILE") {
            return path.into();
        }
        crate::native::config_dir().join("connection.json")
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Self {
        Self::load_from(Self::default_path())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from(path: impl AsRef<std::path::Path>) -> Self {
        let Ok(json) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str::<Self>(&json)
            .map(Self::normalized)
            .unwrap_or_default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self) -> Result<(), String> {
        let path = Self::default_path();
        let parent = path
            .parent()
            .ok_or_else(|| "Lokasi berkas pengaturan koneksi tidak valid.".to_owned())?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Gagal membuat folder pengaturan: {error}"))?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|error| format!("Gagal menyiapkan pengaturan koneksi: {error}"))?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, json)
            .map_err(|error| format!("Gagal menulis pengaturan koneksi: {error}"))?;
        std::fs::rename(&temporary, &path)
            .map_err(|error| format!("Gagal menyimpan pengaturan koneksi: {error}"))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Self {
        Self::default()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save(&self) -> Result<(), String> {
        Ok(())
    }
}

/// Accepts `host`, `host:port`, `http://host`, or `http://host/` and returns
/// just the host part. IPv6 literals in brackets are preserved.
pub fn normalize_host(raw: &str) -> String {
    let mut host = raw.trim();
    for prefix in ["http://", "https://"] {
        if let Some(rest) = host.strip_prefix(prefix) {
            host = rest;
            break;
        }
    }
    let host = host.split('/').next().unwrap_or(host);
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    if host.starts_with('[') {
        return host
            .split_once(']')
            .map(|(inner, _)| format!("{inner}]"))
            .unwrap_or_else(|| host.to_owned());
    }
    match host.rsplit_once(':') {
        Some((name, port)) if port.chars().all(|c| c.is_ascii_digit()) => name.to_owned(),
        _ => host.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectionMode, ConnectionSettings, normalize_host};

    #[test]
    fn public_mode_uses_the_fixed_server() {
        let settings = ConnectionSettings::default();
        assert_eq!(settings.mode, ConnectionMode::Public);
        assert_eq!(settings.base_url(), "http://103.179.218.76:8122");
        assert_eq!(settings.label(), "Publik");
    }

    #[test]
    fn local_mode_uses_the_authored_address() {
        let settings = ConnectionSettings {
            mode: ConnectionMode::Local,
            local_host: "192.168.1.50".to_owned(),
            local_port: 9000,
        };
        assert_eq!(settings.base_url(), "http://192.168.1.50:9000");
        assert_eq!(settings.label(), "Lokal (192.168.1.50:9000)");
    }

    #[test]
    fn host_input_is_cleaned_up() {
        assert_eq!(normalize_host(" 127.0.0.1 "), "127.0.0.1");
        assert_eq!(normalize_host("http://127.0.0.1:8122/"), "127.0.0.1");
        assert_eq!(normalize_host("https://twg.local/"), "twg.local");
        assert_eq!(normalize_host("twg.local:9000"), "twg.local");
        assert_eq!(normalize_host("[::1]:8122"), "[::1]");
        let settings = ConnectionSettings {
            mode: ConnectionMode::Local,
            local_host: "  ".to_owned(),
            local_port: 0,
        }
        .normalized();
        assert_eq!(settings.local_host, "127.0.0.1");
        assert_eq!(settings.local_port, 8122);
    }

    #[test]
    fn settings_round_trip_through_json() {
        let settings = ConnectionSettings {
            mode: ConnectionMode::Local,
            local_host: "10.0.0.5".to_owned(),
            local_port: 8122,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: ConnectionSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, settings);
    }
}
