//! Signed auto-update support for `twg-desktop`.
//!
//! The update manifest is a small JSON document signed with Ed25519. The
//! private key never ships with the client; the public key is embedded here
//! from `assets/update-public-key.txt`. Artifacts are verified with the
//! SHA-256 hash from the signed manifest before they are installed.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const DEFAULT_MANIFEST_URL: &str = "http://103.179.218.76:8122/updates/twg-desktop/stable.json";

const PUBLIC_KEY: &str = include_str!("../../../assets/update-public-key.txt");
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateManifest {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub channel: String,
    pub version: String,
    pub build: u64,
    #[serde(default)]
    pub min_supported_build: u64,
    #[serde(default)]
    pub mandatory: bool,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub platforms: BTreeMap<String, Artifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    #[serde(default)]
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct UpdatePlan {
    pub manifest: UpdateManifest,
    pub artifact: Artifact,
    /// Raw manifest bytes used for signature verification and diagnostics.
    pub manifest_bytes: Vec<u8>,
}

impl UpdatePlan {
    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    pub fn build(&self) -> u64 {
        self.manifest.build
    }

    pub fn notes(&self) -> &str {
        &self.manifest.notes
    }

    pub fn size(&self) -> u64 {
        self.artifact.size
    }

    pub fn mandatory(&self, current_build: u64) -> bool {
        self.manifest.mandatory || current_build < self.manifest.min_supported_build
    }
}

#[derive(Debug, Clone)]
pub enum CheckOutcome {
    UpToDate,
    Available(Box<UpdatePlan>),
}

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("jaringan: {0}")]
    Network(String),
    #[error("HTTP {0} untuk {1}")]
    Http(u16, String),
    #[error("manifest tidak valid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("schema manifest tidak didukung: {0}")]
    UnsupportedSchema(u32),
    #[error("signature manifest tidak valid")]
    Signature,
    #[error("public key update tidak valid: {0}")]
    PublicKey(String),
    #[error("artefak untuk platform {0} tidak tersedia")]
    MissingPlatform(String),
    #[error("artefak update tidak valid: {0}")]
    InvalidArtifact(String),
    #[error("SHA-256 tidak cocok (harus {expected}, didapat {actual})")]
    HashMismatch { expected: String, actual: String },
    #[error("ukuran file tidak cocok (harus {expected}, didapat {actual})")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("gagal menyimpan update: {0}")]
    Io(#[from] std::io::Error),
    #[error("gagal memasang update: {0}")]
    Install(String),
}

/// Platform key used in the manifest, e.g. `windows-x86_64` or
/// `linux-x86_64-appimage`. Linux separates AppImage and plain-binary
/// installs so the updater never replaces one kind with the other.
pub fn platform_key() -> String {
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_some() {
        return match std::env::consts::ARCH {
            "x86_64" => "linux-x86_64-appimage",
            "aarch64" => "linux-aarch64-appimage",
            _ => "unsupported",
        }
        .to_owned();
    }
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "windows-x86_64",
        ("windows", "aarch64") => "windows-aarch64",
        ("linux", "x86_64") => "linux-x86_64-binary",
        ("linux", "aarch64") => "linux-aarch64-binary",
        ("macos", "aarch64") => "macos-aarch64",
        ("macos", "x86_64") => "macos-x86_64",
        _ => "unsupported",
    }
    .to_owned()
}

/// Checks the signed manifest against the build number compiled into the app.
pub fn check(manifest_url: &str, current_build: u64) -> Result<CheckOutcome, UpdateError> {
    let manifest_bytes = fetch(manifest_url)?;
    let signature = String::from_utf8(fetch(&format!("{manifest_url}.sig"))?)
        .map_err(|error| UpdateError::InvalidArtifact(error.to_string()))?;
    verify_manifest_signature(&manifest_bytes, &signature)?;
    let manifest: UpdateManifest = serde_json::from_slice(&manifest_bytes)?;
    if manifest.schema != 1 {
        return Err(UpdateError::UnsupportedSchema(manifest.schema));
    }
    if manifest.build <= current_build {
        return Ok(CheckOutcome::UpToDate);
    }
    let platform = platform_key();
    let Some(artifact) = manifest.platforms.get(&platform).cloned() else {
        return Err(UpdateError::MissingPlatform(platform));
    };
    validate_artifact(&artifact)?;
    Ok(CheckOutcome::Available(Box::new(UpdatePlan {
        manifest,
        artifact,
        manifest_bytes,
    })))
}

fn validate_artifact(artifact: &Artifact) -> Result<(), UpdateError> {
    if !artifact.url.starts_with("http://") && !artifact.url.starts_with("https://") {
        return Err(UpdateError::InvalidArtifact(format!(
            "URL tidak didukung: {}",
            artifact.url
        )));
    }
    if artifact.sha256.len() != 64 || !artifact.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(UpdateError::InvalidArtifact(
            "sha256 harus 64 karakter heksadesimal".to_owned(),
        ));
    }
    Ok(())
}

/// Downloads the artifact to the update cache and verifies its SHA-256.
pub fn download<F>(plan: &UpdatePlan, mut progress: F) -> Result<PathBuf, UpdateError>
where
    F: FnMut(u64, Option<u64>),
{
    let directory = cache_dir();
    fs::create_dir_all(&directory)?;
    let filename = format!(
        "twg-desktop-{}-{}.download",
        plan.manifest.version, plan.artifact.kind
    );
    let target = directory.join(filename);
    if target.exists() {
        let _ = fs::remove_file(&target);
    }

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(DOWNLOAD_TIMEOUT))
        .build()
        .into();
    let mut response = agent
        .get(&plan.artifact.url)
        .call()
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(UpdateError::Http(status, plan.artifact.url.clone()));
    }
    let total = (plan.artifact.size > 0).then_some(plan.artifact.size);
    let mut reader = response.body_mut().as_reader();
    let mut file = fs::File::create(&target)?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                file.write_all(&buffer[..read])?;
                hasher.update(&buffer[..read]);
                downloaded += read as u64;
                progress(downloaded, total);
            }
            Err(error) => {
                let _ = fs::remove_file(&target);
                return Err(UpdateError::Io(error));
            }
        }
    }
    file.flush()?;
    drop(file);

    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(plan.artifact.sha256.trim()) {
        let _ = fs::remove_file(&target);
        return Err(UpdateError::HashMismatch {
            expected: plan.artifact.sha256.clone(),
            actual,
        });
    }
    if plan.artifact.size > 0 && downloaded != plan.artifact.size {
        let _ = fs::remove_file(&target);
        return Err(UpdateError::SizeMismatch {
            expected: plan.artifact.size,
            actual: downloaded,
        });
    }
    Ok(target)
}

/// Installs the downloaded artifact and returns the path that should be
/// launched for the new version.
pub fn install(downloaded: &Path, current_build: u64) -> Result<PathBuf, UpdateError> {
    #[cfg(target_os = "windows")]
    {
        install_windows(downloaded, current_build)
    }
    #[cfg(target_os = "linux")]
    {
        install_linux(downloaded, current_build)
    }
    #[cfg(target_os = "macos")]
    {
        install_macos(downloaded, current_build)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = (downloaded, current_build);
        Err(UpdateError::Install(
            "platform ini belum mendukung update otomatis".to_owned(),
        ))
    }
}

/// Starts the new version. On macOS this opens the app bundle, elsewhere it
/// executes the new binary directly.
pub fn launch(path: &Path) -> Result<(), UpdateError> {
    #[cfg(target_os = "macos")]
    if path.extension().is_some_and(|extension| extension == "app") {
        std::process::Command::new("open")
            .arg("-n")
            .arg(path)
            .spawn()
            .map_err(|error| UpdateError::Install(error.to_string()))?;
        return Ok(());
    }
    std::process::Command::new(path)
        .spawn()
        .map_err(|error| UpdateError::Install(error.to_string()))?;
    Ok(())
}

/// Best-effort removal of backups left by a previous update.
pub fn cleanup_old_versions() {
    #[cfg(target_os = "windows")]
    if let Ok(current) = std::env::current_exe()
        && let Some(parent) = current.parent()
        && let Some(name) = current.file_name().and_then(|name| name.to_str())
    {
        remove_backups(parent, name);
    }

    #[cfg(target_os = "linux")]
    {
        let target = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .or_else(|| std::env::current_exe().ok());
        if let Some(target) = target
            && let Some(parent) = target.parent()
            && let Some(name) = target.file_name().and_then(|name| name.to_str())
        {
            remove_backups(parent, name);
        }
    }

    #[cfg(target_os = "macos")]
    if let Ok(current) = std::env::current_exe()
        && let Some(app) = app_bundle(&current)
        && let Some(parent) = app.parent()
        && let Ok(entries) = fs::read_dir(parent)
    {
        let app_name = app
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        let prefix = format!(".{app_name}.old-");
        let prefix = format!(
            ".{}.old-",
            app.file_name().unwrap_or_default().to_string_lossy()
        );
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&prefix) || name.starts_with(".twg-update-") {
                let path = entry.path();
                if path.is_dir() {
                    let _ = fs::remove_dir_all(path);
                } else {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn remove_backups(directory: &Path, filename: &str) {
    let prefix = format!("{filename}.old-");
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) || name == format!("{filename}.new") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn fetch(url: &str) -> Result<Vec<u8>, UpdateError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(CHECK_TIMEOUT))
        .build()
        .into();
    let mut response = agent
        .get(url)
        .call()
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(UpdateError::Http(status, url.to_owned()));
    }
    response
        .body_mut()
        .read_to_vec()
        .map_err(|error| UpdateError::Network(error.to_string()))
}

fn verify_manifest_signature(manifest: &[u8], signature: &str) -> Result<(), UpdateError> {
    let public_key = BASE64
        .decode(PUBLIC_KEY.trim())
        .map_err(|error| UpdateError::PublicKey(error.to_string()))?;
    let signature = BASE64
        .decode(signature.trim())
        .map_err(|error| UpdateError::InvalidArtifact(error.to_string()))?;
    verify_ed25519(&public_key, manifest, &signature)
}

fn verify_ed25519(public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<(), UpdateError> {
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key)
        .verify(message, signature)
        .map_err(|_| UpdateError::Signature)
}

/// Directory used for downloaded update artifacts.
pub fn cache_dir() -> PathBuf {
    if let Ok(value) = std::env::var("TWG_UPDATE_CACHE")
        && !value.trim().is_empty()
    {
        return PathBuf::from(value);
    }
    if cfg!(windows)
        && let Ok(base) = std::env::var("LOCALAPPDATA")
    {
        return PathBuf::from(base).join("twg").join("updates");
    }
    if cfg!(target_os = "macos")
        && let Ok(home) = std::env::var("HOME")
    {
        return PathBuf::from(home)
            .join("Library")
            .join("Caches")
            .join("twg")
            .join("updates");
    }
    if let Ok(base) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(base).join("twg").join("updates");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("twg")
            .join("updates");
    }
    std::env::temp_dir().join("twg").join("updates")
}

#[cfg(target_os = "windows")]
fn install_windows(downloaded: &Path, current_build: u64) -> Result<PathBuf, UpdateError> {
    let current = std::env::current_exe()?;
    let parent = current
        .parent()
        .ok_or_else(|| UpdateError::Install("folder aplikasi tidak ditemukan".to_owned()))?;
    let name = current
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| UpdateError::Install("nama exe tidak valid".to_owned()))?;
    let staged = parent.join(format!("{name}.new"));
    let backup = parent.join(format!("{name}.old-{current_build}"));
    fs::copy(downloaded, &staged)?;
    let _ = fs::remove_file(&backup);
    fs::rename(&current, &backup).map_err(|error| {
        UpdateError::Install(format!(
            "tidak bisa mengganti {name} (folder mungkin read-only atau exe terkunci): {error}"
        ))
    })?;
    if let Err(error) = fs::rename(&staged, &current) {
        let _ = fs::rename(&backup, &current);
        return Err(UpdateError::Install(format!(
            "gagal memasang exe baru: {error}"
        )));
    }
    Ok(current)
}

#[cfg(target_os = "linux")]
fn install_linux(downloaded: &Path, current_build: u64) -> Result<PathBuf, UpdateError> {
    use std::os::unix::fs::PermissionsExt;

    let target = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| std::env::current_exe().ok())
        .ok_or_else(|| UpdateError::Install("path aplikasi tidak ditemukan".to_owned()))?;
    let parent = target
        .parent()
        .ok_or_else(|| UpdateError::Install("folder aplikasi tidak ditemukan".to_owned()))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| UpdateError::Install("nama aplikasi tidak valid".to_owned()))?;
    let staged = parent.join(format!("{name}.new"));
    let backup = parent.join(format!("{name}.old-{current_build}"));
    fs::copy(downloaded, &staged)?;
    fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))?;
    let _ = fs::remove_file(&backup);
    fs::rename(&target, &backup).map_err(|error| {
        UpdateError::Install(format!(
            "tidak bisa mengganti {name} (folder mungkin read-only): {error}"
        ))
    })?;
    if let Err(error) = fs::rename(&staged, &target) {
        let _ = fs::rename(&backup, &target);
        return Err(UpdateError::Install(format!(
            "gagal memasang file baru: {error}"
        )));
    }
    Ok(target)
}

#[cfg(target_os = "macos")]
fn install_macos(downloaded: &Path, current_build: u64) -> Result<PathBuf, UpdateError> {
    let current = std::env::current_exe()?;
    let app = app_bundle(&current)
        .ok_or_else(|| UpdateError::Install("bundle .app tidak ditemukan".to_owned()))?;
    let parent = app
        .parent()
        .ok_or_else(|| UpdateError::Install("folder aplikasi tidak ditemukan".to_owned()))?;
    let staging = parent.join(format!(".twg-update-{current_build}"));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)?;
    let status = std::process::Command::new("ditto")
        .arg("-x")
        .arg("-k")
        .arg(downloaded)
        .arg(&staging)
        .status()
        .map_err(|error| UpdateError::Install(format!("ditto: {error}")))?;
    if !status.success() {
        return Err(UpdateError::Install(
            "gagal mengekstrak bundle update".to_owned(),
        ));
    }
    let new_app = fs::read_dir(&staging)?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| UpdateError::Install("bundle .app tidak ada di arsip".to_owned()))?;
    let name = app
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| UpdateError::Install("nama bundle tidak valid".to_owned()))?;
    let backup = parent.join(format!(".{name}.old-{current_build}"));
    let _ = fs::remove_dir_all(&backup);
    fs::rename(&app, &backup)
        .map_err(|error| UpdateError::Install(format!("tidak bisa mengganti bundle: {error}")))?;
    if let Err(error) = fs::rename(&new_app, &app) {
        let _ = fs::rename(&backup, &app);
        return Err(UpdateError::Install(format!(
            "gagal memasang bundle baru: {error}"
        )));
    }
    let _ = fs::remove_dir_all(&staging);
    Ok(app)
}

#[cfg(target_os = "macos")]
fn app_bundle(executable: &Path) -> Option<&Path> {
    executable
        .ancestors()
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
}

#[cfg(test)]
mod tests {
    use super::{
        Artifact, UpdateManifest, UpdatePlan, platform_key, verify_ed25519,
        verify_manifest_signature,
    };
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    #[test]
    fn manifest_parses_platform_artifacts() {
        let manifest: UpdateManifest = serde_json::from_str(
            r#"{
                "schema": 1,
                "version": "0.2.0",
                "build": 43,
                "mandatory": true,
                "notes": "fix",
                "platforms": {
                    "windows-x86_64": {
                        "kind": "exe",
                        "url": "http://example.test/twg.exe",
                        "size": 10,
                        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    }
                }
            }"#,
        )
        .unwrap();
        assert_eq!(manifest.build, 43);
        assert!(manifest.mandatory);
        assert_eq!(manifest.platforms["windows-x86_64"].kind, "exe");
    }

    #[test]
    fn plan_reports_mandatory_from_min_supported_build() {
        let plan = UpdatePlan {
            manifest: UpdateManifest {
                schema: 1,
                channel: "stable".to_owned(),
                version: "0.2.0".to_owned(),
                build: 43,
                min_supported_build: 40,
                mandatory: false,
                published_at: String::new(),
                notes: String::new(),
                platforms: std::collections::BTreeMap::new(),
            },
            artifact: Artifact {
                kind: "exe".to_owned(),
                url: "http://example.test/twg.exe".to_owned(),
                size: 1,
                sha256: "a".repeat(64),
            },
            manifest_bytes: Vec::new(),
        };
        assert!(plan.mandatory(39));
        assert!(!plan.mandatory(40));
    }

    #[test]
    fn ed25519_signatures_are_verified() {
        let random = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&random).unwrap();
        let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let message = b"signed update manifest";
        let signature = pair.sign(message);
        verify_ed25519(pair.public_key().as_ref(), message, signature.as_ref()).unwrap();
        verify_ed25519(pair.public_key().as_ref(), b"tampered", signature.as_ref()).unwrap_err();
    }

    #[test]
    fn manifest_signature_helper_rejects_bad_base64() {
        let error = verify_manifest_signature(b"{}", "not base64!").unwrap_err();
        assert!(matches!(
            error,
            super::UpdateError::InvalidArtifact(_) | super::UpdateError::Signature
        ));
    }

    #[test]
    fn platform_key_is_known() {
        assert_ne!(platform_key(), "unsupported");
        assert!(platform_key().contains('-'));
    }

    #[test]
    fn public_key_is_valid_base64() {
        let raw = BASE64.decode(super::PUBLIC_KEY.trim()).unwrap();
        assert_eq!(raw.len(), 32);
    }
}
