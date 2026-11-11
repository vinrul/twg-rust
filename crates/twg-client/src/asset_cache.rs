//! Offline asset cache for the desktop build.
//!
//! The desktop keeps a mirror of the server asset library under
//! `TWG_ASSET_CACHE` (default: platform cache dir). Before entering the map or
//! a simulation the UI asks the server for `/private/assets/manifest`; only the
//! files whose asset `version` changed (or that are missing) are downloaded.
//! Everything the editor/game loads afterwards comes from this local mirror,
//! so play works without a network connection.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use twg_contracts::assets::{Asset, AssetManifest, AssetSourceSlot, AssetVariantKind};
use uuid::Uuid;

const MANIFEST_FILE: &str = "manifest.json";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// One file that has to exist in the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedFileRef {
    pub asset_id: Uuid,
    pub slot: AssetSourceSlot,
    pub filename: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: i64,
}

/// Progress shared with the UI thread while a sync runs.
pub use twg_contracts::assets::SyncProgress;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncReport {
    pub downloaded: usize,
    pub bytes: i64,
    pub removed: usize,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredManifest {
    saved_at: String,
    manifest: AssetManifest,
}

/// Local mirror of the server asset library.
#[derive(Debug, Clone)]
pub struct AssetCache {
    root: PathBuf,
    manifest: Option<AssetManifest>,
}

impl AssetCache {
    pub fn new() -> Self {
        Self::with_root(default_root())
    }

    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let manifest = read_manifest(&root);
        Self { root, manifest }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> Option<&AssetManifest> {
        self.manifest.as_ref()
    }

    pub fn asset(&self, asset_id: Uuid) -> Option<&Asset> {
        self.manifest
            .as_ref()?
            .assets
            .iter()
            .find(|asset| asset.id == asset_id)
    }

    pub fn local_path(&self, asset_id: Uuid, slot: AssetSourceSlot, filename: &str) -> PathBuf {
        self.root
            .join(asset_id.to_string())
            .join(slot.to_string())
            .join(filename)
    }

    /// True when the cache already mirrors `remote` (same versions, files on
    /// disk).
    pub fn is_current(&self, remote: &AssetManifest) -> bool {
        let Some(local) = &self.manifest else {
            return false;
        };
        if local.assets.len() != remote.assets.len() {
            return false;
        }
        let same_versions = local.assets.iter().all(|local_asset| {
            remote
                .assets
                .iter()
                .find(|remote_asset| remote_asset.id == local_asset.id)
                .is_some_and(|remote_asset| remote_asset.version == local_asset.version)
        });
        same_versions && self.plan(remote).is_empty()
    }

    /// Files that are missing from disk for `remote`. When the local manifest
    /// knows this asset with a different version, every one of its files is
    /// planned again: a conversion or re-upload can overwrite a file whose size
    /// happens to stay the same.
    pub fn plan(&self, remote: &AssetManifest) -> Vec<CachedFileRef> {
        remote
            .assets
            .iter()
            .flat_map(|asset| {
                let changed = self
                    .manifest
                    .as_ref()
                    .and_then(|local| {
                        local
                            .assets
                            .iter()
                            .find(|local_asset| local_asset.id == asset.id)
                    })
                    .is_some_and(|local_asset| local_asset.version != asset.version);
                asset_files(asset).into_iter().filter(move |file| {
                    if changed {
                        return true;
                    }
                    let path = self.local_path(file.asset_id, file.slot, &file.filename);
                    match fs::metadata(&path) {
                        Ok(metadata) => metadata.len() != file.size_bytes.max(0) as u64,
                        Err(_) => true,
                    }
                })
            })
            .collect()
    }

    /// Downloads everything missing and rewrites `manifest.json` when done.
    ///
    /// The manifest is only updated after all files succeed, so an interrupted
    /// sync simply leaves the previous consistent snapshot in place.
    pub fn sync(
        &mut self,
        remote: &AssetManifest,
        base_url: &str,
        progress: &std::sync::Mutex<SyncProgress>,
    ) -> Result<SyncReport, String> {
        let base_url = base_url.trim_end_matches('/');
        let planned = self.plan(remote);
        {
            let mut guard = progress.lock().expect("progress poisoned");
            guard.total = planned.len();
            guard.done = 0;
            guard.current.clear();
            guard.error = None;
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(DOWNLOAD_TIMEOUT))
            .build()
            .into();
        let mut report = SyncReport::default();
        for file in &planned {
            {
                let mut guard = progress.lock().expect("progress poisoned");
                guard.current = format!("{}/{}", file.slot, file.filename);
            }
            let url = format!("{base_url}{}", file.url);
            let bytes = download(&agent, &url)?;
            if !file.sha256.is_empty() && sha256_hex(&bytes) != file.sha256 {
                return Err(format!(
                    "checksum mismatch for {}/{}",
                    file.slot, file.filename
                ));
            }
            let path = self.local_path(file.asset_id, file.slot, &file.filename);
            write_atomic(&path, &bytes)?;
            report.downloaded += 1;
            report.bytes += bytes.len() as i64;
            let mut guard = progress.lock().expect("progress poisoned");
            guard.done += 1;
        }
        report.removed = self.prune(remote)?;
        let stored = StoredManifest {
            saved_at: chrono::Utc::now().to_rfc3339(),
            manifest: remote.clone(),
        };
        let json = serde_json::to_vec_pretty(&stored).map_err(|error| error.to_string())?;
        write_atomic(&self.root.join(MANIFEST_FILE), &json)?;
        self.manifest = Some(remote.clone());
        Ok(report)
    }

    fn prune(&self, remote: &AssetManifest) -> Result<usize, String> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Ok(0);
        };
        let keep = remote
            .assets
            .iter()
            .map(|asset| asset.id.to_string())
            .collect::<std::collections::HashSet<_>>();
        let mut removed = 0;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == MANIFEST_FILE || keep.contains(&name) {
                continue;
            }
            if Uuid::parse_str(&name).is_err() {
                continue;
            }
            fs::remove_dir_all(entry.path()).map_err(|error| error.to_string())?;
            removed += 1;
        }
        Ok(removed)
    }

    /// Resolves the file to use for one source slot. Converted variants win
    /// when `prefer_variants` is true, otherwise the original upload is used.
    pub fn resolve(
        &self,
        asset: &Asset,
        slot: AssetSourceSlot,
        prefer_variants: bool,
    ) -> Option<(Option<AssetVariantKind>, PathBuf)> {
        let source = asset.source(slot)?;
        if prefer_variants {
            for kind in [
                AssetVariantKind::DracoLod2,
                AssetVariantKind::Lod2,
                AssetVariantKind::Draco,
            ] {
                if let Some(variant) = source.variant(kind) {
                    let path = self.local_path(asset.id, slot, &variant.filename);
                    if path.is_file() {
                        return Some((Some(kind), path));
                    }
                }
            }
        }
        let path = self.local_path(asset.id, slot, &source.file.filename);
        path.is_file().then_some((None, path))
    }
}

impl Default for AssetCache {
    fn default() -> Self {
        Self::new()
    }
}

pub fn asset_files(asset: &Asset) -> Vec<CachedFileRef> {
    let mut files = Vec::new();
    for source in &asset.sources {
        files.push(CachedFileRef {
            asset_id: asset.id,
            slot: source.slot,
            filename: source.file.filename.clone(),
            url: source.file.url.clone(),
            sha256: source.file.sha256.clone(),
            size_bytes: source.file.size_bytes,
        });
        for variant in &source.variants {
            files.push(CachedFileRef {
                asset_id: asset.id,
                slot: source.slot,
                filename: variant.filename.clone(),
                url: variant.url.clone(),
                sha256: variant.sha256.clone(),
                size_bytes: variant.size_bytes,
            });
        }
    }
    files
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Platform cache directory, overridable with `TWG_ASSET_CACHE`.
///
/// Relative overrides are resolved against the current directory so the Bevy
/// asset source (which resolves relative paths against the executable) and the
/// cache writer agree on the same location.
pub fn default_root() -> PathBuf {
    if let Ok(value) = std::env::var("TWG_ASSET_CACHE")
        && !value.trim().is_empty()
    {
        let path = PathBuf::from(value);
        return std::path::absolute(&path).unwrap_or(path);
    }
    #[cfg(target_os = "macos")]
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join("Library/Caches/twg/assets");
    }
    #[cfg(target_os = "windows")]
    if let Ok(base) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(base).join("twg").join("assets");
    }
    if let Ok(base) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(base).join("twg").join("assets");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("twg")
            .join("assets");
    }
    std::env::temp_dir().join("twg").join("assets")
}

fn read_manifest(root: &Path) -> Option<AssetManifest> {
    let bytes = fs::read(root.join(MANIFEST_FILE)).ok()?;
    let stored: StoredManifest = serde_json::from_slice(&bytes).ok()?;
    Some(stored.manifest)
}

fn download(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let mut response = agent
        .get(url)
        .call()
        .map_err(|error| format!("download {url} failed: {error}"))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("download {url} returned HTTP {status}"));
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .read_to_vec()
        .map_err(|error| format!("download {url} failed: {error}"))
        .map(|body| {
            bytes.extend_from_slice(&body);
            bytes
        })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temp = path.with_extension("part");
    fs::write(&temp, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temp, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{AssetCache, asset_files, sha256_hex};
    use twg_contracts::Timestamp;
    use twg_contracts::assets::{
        Asset, AssetCategory, AssetFile, AssetModelKind, AssetMoveMode, AssetSource,
        AssetSourceSlot, AssetVariant, AssetVariantKind,
    };
    use uuid::Uuid;

    fn asset_file(name: &str, url: &str) -> AssetFile {
        AssetFile {
            filename: name.to_owned(),
            url: url.to_owned(),
            mime: "model/gltf-binary".to_owned(),
            size_bytes: 4,
            sha256: sha256_hex(b"data"),
        }
    }

    fn static_model(version: i64) -> Asset {
        let id = Uuid::from_u128(7);
        Asset {
            id,
            name: "model".to_owned(),
            category: AssetCategory::Model,
            model_kind: Some(AssetModelKind::Static),
            move_mode: AssetMoveMode::None,
            group_id: None,
            group_name: None,
            sources: vec![AssetSource {
                slot: AssetSourceSlot::Static,
                file: asset_file("original.glb", &format!("/assets/{id}/static/original.glb")),
                has_draco: false,
                lod: None,
                variants: vec![AssetVariant {
                    kind: AssetVariantKind::DracoLod2,
                    filename: "draco_lod2.glb".to_owned(),
                    url: format!("/assets/{id}/static/draco_lod2.glb"),
                    mime: "model/gltf-binary".to_owned(),
                    size_bytes: 4,
                    sha256: sha256_hex(b"data"),
                    width: None,
                    height: None,
                    reused: false,
                }],
            }],
            version,
            created_at: Timestamp::from(chrono::Utc::now()),
            updated_at: Timestamp::from(chrono::Utc::now()),
        }
    }

    fn manifest(version: i64) -> twg_contracts::assets::AssetManifest {
        twg_contracts::assets::AssetManifest {
            generated_at: Timestamp::from(chrono::Utc::now()),
            assets: vec![static_model(version)],
        }
    }

    fn temp_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("twg-cache-test-{tag}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn plan_lists_missing_source_and_variant_files() {
        let root = temp_root("plan");
        let cache = AssetCache::with_root(&root);
        let remote = manifest(1);
        assert_eq!(cache.plan(&remote).len(), 2);

        for file in asset_files(&remote.assets[0]) {
            let path = cache.local_path(file.asset_id, file.slot, &file.filename);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"data").unwrap();
        }
        assert!(cache.plan(&remote).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_prefers_converted_variant() {
        let root = temp_root("resolve");
        let cache = AssetCache::with_root(&root);
        let asset = static_model(1);
        for file in asset_files(&asset) {
            let path = cache.local_path(file.asset_id, file.slot, &file.filename);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"data").unwrap();
        }
        let (kind, path) = cache
            .resolve(&asset, AssetSourceSlot::Static, true)
            .unwrap();
        assert_eq!(kind, Some(AssetVariantKind::DracoLod2));
        assert!(path.ends_with("draco_lod2.glb"));

        let (kind, path) = cache
            .resolve(&asset, AssetSourceSlot::Static, false)
            .unwrap();
        assert_eq!(kind, None);
        assert!(path.ends_with("original.glb"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn plan_redownloads_files_when_the_asset_version_changes() {
        let root = temp_root("version-plan");
        let mut cache = AssetCache::with_root(&root);
        let remote = manifest(1);
        for file in asset_files(&remote.assets[0]) {
            let path = cache.local_path(file.asset_id, file.slot, &file.filename);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"data").unwrap();
        }
        assert!(cache.plan(&remote).is_empty());

        // Same paths and sizes, but a newer version must re-download so an
        // overwritten conversion cannot stay stale in the cache.
        cache.manifest = Some(remote.clone());
        assert_eq!(cache.plan(&manifest(2)).len(), 2);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn is_current_checks_versions_and_files() {
        let root = temp_root("current");
        let mut cache = AssetCache::with_root(&root);
        let remote = manifest(1);
        assert!(!cache.is_current(&remote));
        for file in asset_files(&remote.assets[0]) {
            let path = cache.local_path(file.asset_id, file.slot, &file.filename);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"data").unwrap();
        }
        // Without a stored manifest the cache cannot be trusted yet.
        assert!(!cache.is_current(&remote));
        cache.manifest = Some(remote.clone());
        assert!(cache.is_current(&remote));
        assert!(!cache.is_current(&manifest(2)));
        let _ = std::fs::remove_dir_all(root);
    }
}
