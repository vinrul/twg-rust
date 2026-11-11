//! Asset file layout, format probing, and conversions.
//!
//! Layout on disk (one directory per asset id, then one per source slot):
//!
//! ```text
//! <ASSETS_DIR>/<asset-id>/static/original.png
//! <ASSETS_DIR>/<asset-id>/static/resize_512.png
//! <ASSETS_DIR>/<asset-id>/idle/original.glb
//! <ASSETS_DIR>/<asset-id>/idle/draco.glb
//! <ASSETS_DIR>/<asset-id>/animated/original.glb
//! <ASSETS_DIR>/<asset-id>/animated/draco_lod2.glb
//! ```
//!
//! glTF conversions shell out to `gltf-transform` (Draco encoder and mesh
//! simplification); image resizing is done in-process with the `image` crate.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use sha2::{Digest, Sha256};
use twg_contracts::assets::{AssetSourceSlot, AssetVariantKind, ConvertTarget};
use uuid::Uuid;

const GLB_MAGIC: &[u8; 4] = b"glTF";
const CHUNK_JSON: u32 = 0x4E4F_534A; // "JSON"
const CHUNK_BIN: u32 = 0x004E_4942; // "BIN\0"
const GLB_HEADER_LEN: usize = 12;
const CHUNK_HEADER_LEN: usize = 8;
const JSON_CHUNK_PADDING: u8 = b' ';

/// What was detected inside an uploaded glTF/GLB file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GltfProbe {
    /// `KHR_draco_mesh_compression` is present in `extensionsUsed`.
    pub has_draco: bool,
    /// LOD level from `extras.twg.lod` or a `lodN` filename marker.
    pub lod: Option<u16>,
    /// File references buffers/images outside itself (`.gltf` + `.bin`).
    pub external_resources: bool,
    /// Number of entries in the glTF `animations` array.
    pub animation_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResizedImage {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub extension: &'static str,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConvertConfig {
    /// `gltf-transform` executable name or absolute path.
    pub gltf_transform: String,
    /// Target ratio passed to `gltf-transform simplify` for LOD2.
    pub lod2_ratio: f64,
    /// Max error passed to `gltf-transform simplify`.
    pub lod2_error: f64,
}

impl Default for ConvertConfig {
    fn default() -> Self {
        Self {
            gltf_transform: "gltf-transform".to_owned(),
            lod2_ratio: 0.5,
            lod2_error: 0.001,
        }
    }
}

/// Local-disk store for asset files.
#[derive(Debug, Clone)]
pub struct AssetFiles {
    directory: PathBuf,
}

impl AssetFiles {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn asset_dir(&self, asset_id: Uuid) -> PathBuf {
        self.directory.join(asset_id.to_string())
    }

    pub fn path(&self, asset_id: Uuid, slot: AssetSourceSlot, filename: &str) -> PathBuf {
        let safe = Path::new(filename)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("asset");
        self.asset_dir(asset_id).join(slot.to_string()).join(safe)
    }

    pub async fn write(
        &self,
        asset_id: Uuid,
        slot: AssetSourceSlot,
        filename: &str,
        bytes: &[u8],
    ) -> io::Result<()> {
        let path = self.path(asset_id, slot, filename);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, bytes).await
    }

    pub async fn read(
        &self,
        asset_id: Uuid,
        slot: AssetSourceSlot,
        filename: &str,
    ) -> io::Result<Vec<u8>> {
        tokio::fs::read(self.path(asset_id, slot, filename)).await
    }

    pub async fn remove_file(
        &self,
        asset_id: Uuid,
        slot: AssetSourceSlot,
        filename: &str,
    ) -> io::Result<()> {
        match tokio::fs::remove_file(self.path(asset_id, slot, filename)).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub async fn remove_asset_dir(&self, asset_id: Uuid) -> io::Result<()> {
        let dir = self.asset_dir(asset_id);
        match tokio::fs::remove_dir_all(dir).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Saved preview screenshot for the editor tooltip and asset list.
    pub fn preview_path(&self, asset_id: Uuid) -> PathBuf {
        self.asset_dir(asset_id).join("preview.png")
    }

    pub async fn write_preview(&self, asset_id: Uuid, bytes: &[u8]) -> io::Result<()> {
        let path = self.preview_path(asset_id);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, bytes).await
    }

    /// Changes whenever the preview file is replaced, so clients re-download.
    pub async fn preview_version(&self, asset_id: Uuid) -> Option<String> {
        let metadata = tokio::fs::metadata(self.preview_path(asset_id))
            .await
            .ok()?;
        let modified = metadata
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis();
        Some(format!("{}-{modified}", metadata.len()))
    }
}

pub fn original_filename(extension: &str) -> String {
    format!("original.{extension}")
}

pub fn variant_filename(kind: AssetVariantKind, extension: &str) -> String {
    format!("{}.{extension}", kind.file_stem())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Detects Draco compression and the LOD level of an uploaded model.
pub fn probe_gltf(bytes: &[u8], filename: &str) -> Result<GltfProbe, String> {
    let json = gltf_json(bytes)?;
    let value: serde_json::Value =
        serde_json::from_slice(json).map_err(|error| format!("JSON glTF tidak valid: {error}"))?;
    let has_draco = value
        .get("extensionsUsed")
        .and_then(|value| value.as_array())
        .is_some_and(|extensions| {
            extensions
                .iter()
                .any(|value| value.as_str() == Some("KHR_draco_mesh_compression"))
        })
        || value
            .pointer("/extras/twg/draco")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let lod = value
        .pointer("/extras/twg/lod")
        .and_then(|value| value.as_u64())
        .and_then(|value| u16::try_from(value).ok())
        .or_else(|| lod_from_filename(filename));
    Ok(GltfProbe {
        has_draco,
        lod,
        external_resources: has_external_resources(&value),
        animation_count: value
            .get("animations")
            .and_then(|value| value.as_array())
            .map(Vec::len)
            .unwrap_or(0),
    })
}

/// Adds `extras.twg = { lod, draco }` to a generated GLB so a later re-upload
/// of the same file is detected without guessing.
pub fn stamp_glb_metadata(bytes: &[u8], lod: Option<u16>, draco: bool) -> Result<Vec<u8>, String> {
    if !bytes.starts_with(GLB_MAGIC) {
        return Err("hasil konversi bukan file GLB".to_owned());
    }
    let chunks = parse_glb_chunks(bytes)?;
    let json_index = chunks
        .iter()
        .position(|chunk| chunk.kind == CHUNK_JSON)
        .ok_or_else(|| "GLB tanpa chunk JSON".to_owned())?;
    let mut value: serde_json::Value =
        serde_json::from_slice(trimmed_json(chunks[json_index].data))
            .map_err(|error| format!("JSON glTF tidak valid: {error}"))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "JSON glTF bukan object".to_owned())?;
    let extras = object
        .entry("extras")
        .or_insert_with(|| serde_json::json!({}));
    if !extras.is_object() {
        *extras = serde_json::json!({});
    }
    let extras = extras.as_object_mut().expect("extras object");
    let twg = extras.entry("twg").or_insert_with(|| serde_json::json!({}));
    if !twg.is_object() {
        *twg = serde_json::json!({});
    }
    let twg = twg.as_object_mut().expect("twg object");
    match lod {
        Some(level) => {
            twg.insert("lod".to_owned(), serde_json::json!(level));
        }
        None => {
            twg.remove("lod");
        }
    }
    twg.insert("draco".to_owned(), serde_json::json!(draco));

    let json_bytes =
        serde_json::to_vec(&value).map_err(|error| format!("gagal serialisasi JSON: {error}"))?;
    rebuild_glb(&chunks, json_index, &json_bytes)
}

/// What a repair pass changed in the asset store.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AssetRepairReport {
    pub scanned: usize,
    pub repaired: usize,
    pub failed: usize,
    pub metadata_updated: usize,
    pub files: Vec<AssetFileScan>,
}

/// Metadata of one GLB found while scanning the asset store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetFileScan {
    pub asset_id: Uuid,
    pub slot: AssetSourceSlot,
    pub filename: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub repaired: bool,
}

/// Some exporters embed PNG/WebP/KTX2 bytes in a GLB but leave the glTF
/// `mimeType` as `image/jpeg`. Bevy's glTF loader trusts that field and refuses
/// the file, so normalize it from the actual image magic bytes.
///
/// Returns `None` when the file is not a GLB or already has correct mime types.
pub fn normalize_glb_image_mime_types(bytes: &[u8]) -> Result<Option<Vec<u8>>, String> {
    if !bytes.starts_with(GLB_MAGIC) {
        return Ok(None);
    }
    let chunks = parse_glb_chunks(bytes)?;
    let json_index = chunks
        .iter()
        .position(|chunk| chunk.kind == CHUNK_JSON)
        .ok_or_else(|| "GLB tanpa chunk JSON".to_owned())?;
    let bin = chunks
        .iter()
        .find(|chunk| chunk.kind == CHUNK_BIN)
        .map(|chunk| chunk.data);
    let mut value: serde_json::Value =
        serde_json::from_slice(trimmed_json(chunks[json_index].data))
            .map_err(|error| format!("JSON glTF tidak valid: {error}"))?;

    let mut fixes = Vec::new();
    {
        let Some(images) = value.get("images").and_then(serde_json::Value::as_array) else {
            return Ok(None);
        };
        let Some(buffer_views) = value
            .get("bufferViews")
            .and_then(serde_json::Value::as_array)
        else {
            return Ok(None);
        };
        let Some(buffers) = value.get("buffers").and_then(serde_json::Value::as_array) else {
            return Ok(None);
        };
        let Some(bin) = bin else {
            return Ok(None);
        };
        // GLB embeds its binary payload in buffer 0 without a URI.
        let Some(buffer) = buffers.first() else {
            return Ok(None);
        };
        if buffer.get("uri").is_some() {
            return Ok(None);
        }

        for (index, image) in images.iter().enumerate() {
            let Some(view_index) = image
                .get("bufferView")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
            else {
                continue;
            };
            let Some(view) = buffer_views.get(view_index) else {
                continue;
            };
            let view_buffer = view
                .get("buffer")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            if view_buffer != 0 {
                continue;
            }
            let offset = view
                .get("byteOffset")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0);
            let length = view
                .get("byteLength")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0);
            let Some(data) = offset
                .checked_add(length)
                .and_then(|end| bin.get(offset..end))
            else {
                continue;
            };
            let Some(mime) = sniff_image_mime(data) else {
                continue;
            };
            if image.get("mimeType").and_then(serde_json::Value::as_str) != Some(mime) {
                fixes.push((index, mime));
            }
        }
    }

    if fixes.is_empty() {
        return Ok(None);
    }
    let Some(images) = value
        .get_mut("images")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(None);
    };
    for (index, mime) in fixes {
        if let Some(image) = images
            .get_mut(index)
            .and_then(serde_json::Value::as_object_mut)
        {
            image.insert(
                "mimeType".to_owned(),
                serde_json::Value::String(mime.to_owned()),
            );
        }
    }

    let json_bytes =
        serde_json::to_vec(&value).map_err(|error| format!("gagal serialisasi JSON: {error}"))?;
    rebuild_glb(&chunks, json_index, &json_bytes).map(Some)
}

fn sniff_image_mime(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else if data.starts_with(&[
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ]) {
        Some("image/ktx2")
    } else {
        None
    }
}

/// Repairs embedded image mime types for every GLB in the asset store. Called
/// at server startup so assets uploaded by older versions work without a
/// re-upload.
pub fn repair_asset_store(directory: &Path) -> AssetRepairReport {
    let mut report = AssetRepairReport::default();
    let mut stack = vec![directory.to_path_buf()];
    while let Some(folder) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let is_glb = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("glb"));
            if !is_glb {
                continue;
            }
            report.scanned += 1;
            let Ok(bytes) = std::fs::read(&path) else {
                report.failed += 1;
                tracing::warn!(path = %path.display(), "failed to read asset");
                continue;
            };
            let (bytes, repaired) = match normalize_glb_image_mime_types(&bytes) {
                Ok(Some(fixed)) => {
                    if let Err(error) = std::fs::write(&path, &fixed) {
                        report.failed += 1;
                        tracing::warn!(path = %path.display(), %error, "failed to repair asset");
                        continue;
                    }
                    report.repaired += 1;
                    (fixed, true)
                }
                Ok(None) => (bytes, false),
                Err(error) => {
                    report.failed += 1;
                    tracing::warn!(path = %path.display(), %error, "failed to inspect asset");
                    continue;
                }
            };
            if let Some((asset_id, slot, filename)) = asset_file_identity(directory, &path) {
                report.files.push(AssetFileScan {
                    asset_id,
                    slot,
                    filename,
                    size_bytes: bytes.len() as i64,
                    sha256: sha256_hex(&bytes),
                    repaired,
                });
            }
        }
    }
    report
}

fn asset_file_identity(root: &Path, path: &Path) -> Option<(Uuid, AssetSourceSlot, String)> {
    let relative = path.strip_prefix(root).ok()?;
    let mut components = relative.components();
    let asset_id = Uuid::parse_str(components.next()?.as_os_str().to_str()?).ok()?;
    let slot = AssetSourceSlot::from_str(components.next()?.as_os_str().to_str()?).ok()?;
    let filename = components.next()?.as_os_str().to_str()?.to_owned();
    if components.next().is_some() {
        return None;
    }
    Some((asset_id, slot, filename))
}

/// Repairs the GLB files and synchronizes their size/sha256 metadata in the
/// asset database. Existing client caches then see a new asset version and
/// re-download the fixed files.
pub async fn repair_asset_store_with_metadata(
    directory: &Path,
    store: &twg_storage::AssetStore,
) -> AssetRepairReport {
    let mut report = repair_asset_store(directory);
    for file in &report.files {
        match store
            .sync_file_metadata(
                file.asset_id,
                file.slot,
                &file.filename,
                file.size_bytes,
                &file.sha256,
            )
            .await
        {
            Ok(true) => report.metadata_updated += 1,
            Ok(false) => {}
            Err(error) => {
                report.failed += 1;
                tracing::warn!(
                    asset_id = %file.asset_id,
                    slot = %file.slot,
                    filename = %file.filename,
                    %error,
                    "failed to update asset metadata"
                );
            }
        }
    }
    report
}

/// Runs the requested glTF conversion. Blocking; call from `spawn_blocking`.
pub fn run_convert(
    config: &ConvertConfig,
    input: &Path,
    output: &Path,
    target: ConvertTarget,
) -> Result<(), String> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("gagal membuat direktori keluaran: {error}"))?;
    }
    match target {
        ConvertTarget::Draco => run_draco(config, input, output),
        ConvertTarget::Lod2 => run_simplify(config, input, output),
        ConvertTarget::DracoLod2 => {
            let temporary = output.with_file_name(format!(
                "{}.lodstep.glb",
                output
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("variant")
            ));
            let result = run_simplify(config, input, &temporary)
                .and_then(|()| run_draco(config, &temporary, output));
            let _ = std::fs::remove_file(&temporary);
            result
        }
        ConvertTarget::Resize => Err("resize bukan konversi glTF".to_owned()),
    }
}

fn run_draco(config: &ConvertConfig, input: &Path, output: &Path) -> Result<(), String> {
    run_tool(config, &["draco", path_arg(input)?, path_arg(output)?])
}

fn run_simplify(config: &ConvertConfig, input: &Path, output: &Path) -> Result<(), String> {
    let ratio = format!("{:.4}", config.lod2_ratio.clamp(0.01, 1.0));
    let error = format!("{:.5}", config.lod2_error.clamp(0.00001, 100.0));
    run_tool(
        config,
        &[
            "simplify",
            path_arg(input)?,
            path_arg(output)?,
            "--ratio",
            &ratio,
            "--error",
            &error,
        ],
    )
}

fn path_arg(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "path asset bukan UTF-8".to_owned())
}

fn run_tool(config: &ConvertConfig, args: &[&str]) -> Result<(), String> {
    let output = Command::new(&config.gltf_transform)
        .args(args)
        .output()
        .map_err(|error| {
            format!(
                "tidak bisa menjalankan `{}` ({error}); pasang dengan `npm i -g @gltf-transform/cli`",
                config.gltf_transform
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = stderr
            .lines()
            .chain(stdout.lines())
            .map(str::trim)
            .rev()
            .find(|line| !line.is_empty())
            .unwrap_or("tanpa pesan");
        return Err(format!(
            "`{} {}` gagal: {detail}",
            config.gltf_transform, args[0]
        ));
    }
    Ok(())
}

/// Resizes a texture/icon to a square canvas, preserving aspect ratio by
/// center-cropping. PNG keeps transparency; JPEG stays JPEG.
pub fn resize_image(bytes: &[u8], side: u32, mime: &str) -> Result<ResizedImage, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| format!("gambar tidak bisa dibaca: {error}"))?;
    let resized = image.resize_to_fill(side, side, image::imageops::FilterType::Lanczos3);
    let keep_alpha = resized.color().has_alpha();
    let (encoded, mime, extension) = if mime.eq_ignore_ascii_case("image/jpeg") && !keep_alpha {
        let mut buffer = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, 85);
        encoder
            .encode_image(&resized)
            .map_err(|error| format!("gagal encode JPEG: {error}"))?;
        (buffer, "image/jpeg".to_owned(), "jpg")
    } else {
        let mut buffer = std::io::Cursor::new(Vec::new());
        resized
            .write_to(&mut buffer, image::ImageFormat::Png)
            .map_err(|error| format!("gagal encode PNG: {error}"))?;
        (buffer.into_inner(), "image/png".to_owned(), "png")
    };
    Ok(ResizedImage {
        bytes: encoded,
        mime,
        extension,
        width: side,
        height: side,
    })
}

pub fn lod_from_filename(filename: &str) -> Option<u16> {
    let lower = filename.to_ascii_lowercase();
    let mut cursor = 0;
    while let Some(found) = lower[cursor..].find("lod") {
        let after_marker = cursor + found + 3;
        let digits: String = lower[after_marker..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if !digits.is_empty() {
            if let Ok(level) = digits.parse::<u16>()
                && level <= 9
            {
                return Some(level);
            }
        } else {
            // A bare `lod` marker (`idle_lod.glb`, `model-lod.glb`) means LOD1.
            let boundary = lower[after_marker..].chars().next();
            if boundary.is_none_or(|character| !character.is_alphanumeric()) {
                return Some(1);
            }
        }
        cursor = after_marker;
        if cursor >= lower.len() {
            break;
        }
    }
    None
}

fn gltf_json(bytes: &[u8]) -> Result<&[u8], String> {
    if bytes.starts_with(GLB_MAGIC) {
        let chunks = parse_glb_chunks(bytes)?;
        return chunks
            .iter()
            .find(|chunk| chunk.kind == CHUNK_JSON)
            .map(|chunk| trimmed_json(chunk.data))
            .ok_or_else(|| "GLB tanpa chunk JSON".to_owned());
    }
    let trimmed = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .map(|start| &bytes[start..])
        .unwrap_or(bytes);
    if trimmed.first() == Some(&b'{') {
        return Ok(trimmed_json(bytes));
    }
    Err("file bukan glTF/GLB".to_owned())
}

/// GLB chunks are padded to 4 bytes; the spec uses spaces for JSON, but other
/// writers use NUL, so both are accepted here.
fn trimmed_json(data: &[u8]) -> &[u8] {
    let end = data
        .iter()
        .rposition(|byte| !matches!(byte, 0 | b' ' | b'\n' | b'\r' | b'\t'))
        .map(|index| index + 1)
        .unwrap_or(0);
    &data[..end]
}

struct GlbChunk<'a> {
    kind: u32,
    data: &'a [u8],
}

fn parse_glb_chunks(bytes: &[u8]) -> Result<Vec<GlbChunk<'_>>, String> {
    if bytes.len() < GLB_HEADER_LEN {
        return Err("GLB terlalu pendek".to_owned());
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().expect("4 bytes"));
    if version != 2 {
        return Err(format!("versi GLB {version} tidak didukung"));
    }
    let mut chunks = Vec::new();
    let mut offset = GLB_HEADER_LEN;
    while offset + CHUNK_HEADER_LEN <= bytes.len() {
        let length =
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("4 bytes")) as usize;
        let kind = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().expect("4 bytes"));
        let start = offset + CHUNK_HEADER_LEN;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| "chunk GLB terpotong".to_owned())?;
        chunks.push(GlbChunk {
            kind,
            data: &bytes[start..end],
        });
        offset = end;
    }
    Ok(chunks)
}

fn rebuild_glb(chunks: &[GlbChunk<'_>], json_index: usize, json: &[u8]) -> Result<Vec<u8>, String> {
    let padding = (4 - json.len() % 4) % 4;
    let json_length = json.len() + padding;
    let mut body_length = 0usize;
    for (index, chunk) in chunks.iter().enumerate() {
        let length = if index == json_index {
            json_length
        } else {
            chunk.data.len()
        };
        body_length += CHUNK_HEADER_LEN + length;
    }
    let total = GLB_HEADER_LEN + body_length;
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(GLB_MAGIC);
    output.extend_from_slice(&2u32.to_le_bytes());
    output.extend_from_slice(&(total as u32).to_le_bytes());
    for (index, chunk) in chunks.iter().enumerate() {
        if index == json_index {
            output.extend_from_slice(&(json_length as u32).to_le_bytes());
            output.extend_from_slice(&CHUNK_JSON.to_le_bytes());
            output.extend_from_slice(json);
            output.extend(std::iter::repeat_n(JSON_CHUNK_PADDING, padding));
        } else {
            output.extend_from_slice(&(chunk.data.len() as u32).to_le_bytes());
            output.extend_from_slice(&chunk.kind.to_le_bytes());
            output.extend_from_slice(chunk.data);
        }
    }
    Ok(output)
}

fn has_external_resources(value: &serde_json::Value) -> bool {
    ["buffers", "images"].iter().any(|key| {
        value
            .get(*key)
            .and_then(|value| value.as_array())
            .is_some_and(|entries| {
                entries.iter().any(|entry| {
                    entry
                        .get("uri")
                        .and_then(|uri| uri.as_str())
                        .is_some_and(|uri| !uri.starts_with("data:"))
                })
            })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        GltfProbe, lod_from_filename, normalize_glb_image_mime_types, probe_gltf, rebuild_glb,
        resize_image, sha256_hex, stamp_glb_metadata,
    };
    use serde_json::json;

    fn build_glb(json: &serde_json::Value, bin: &[u8]) -> Vec<u8> {
        let json_bytes = serde_json::to_vec(json).unwrap();
        let mut chunks = Vec::new();
        let mut offset = 12;
        let mut chunk = |kind: u32, data: &[u8]| {
            let mut padding = (4 - data.len() % 4) % 4;
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&((data.len() + padding) as u32).to_le_bytes());
            bytes.extend_from_slice(&kind.to_le_bytes());
            bytes.extend_from_slice(data);
            while padding > 0 {
                bytes.push(if kind == 0x4E4F_534A { b' ' } else { 0 });
                padding -= 1;
            }
            offset += bytes.len();
            chunks.push(bytes);
        };
        chunk(0x4E4F_534A, &json_bytes);
        if !bin.is_empty() {
            chunk(0x004E_4942, bin);
        }
        let mut output = Vec::new();
        output.extend_from_slice(b"glTF");
        output.extend_from_slice(&2u32.to_le_bytes());
        output.extend_from_slice(&(offset as u32).to_le_bytes());
        for chunk in chunks {
            output.extend_from_slice(&chunk);
        }
        output
    }

    #[test]
    fn detects_draco_and_lod_from_glb_metadata() {
        let glb = build_glb(
            &json!({
                "asset": {"version": "2.0"},
                "extensionsUsed": ["KHR_draco_mesh_compression"],
                "extras": {"twg": {"lod": 2}},
                "buffers": [{"byteLength": 4}]
            }),
            &[1, 2, 3, 4],
        );
        let probe = probe_gltf(&glb, "model.glb").unwrap();
        assert_eq!(
            probe,
            GltfProbe {
                has_draco: true,
                lod: Some(2),
                external_resources: false,
                animation_count: 0,
            }
        );
    }

    #[test]
    fn falls_back_to_filename_lod_marker() {
        let glb = build_glb(
            &json!({"asset": {"version": "2.0"}, "buffers": [{"byteLength": 4}]}),
            &[1, 2, 3, 4],
        );
        let probe = probe_gltf(&glb, "polwan_airud_walk_lod2_draco.glb").unwrap();
        assert!(!probe.has_draco);
        assert_eq!(probe.lod, Some(2));
        assert_eq!(lod_from_filename("idle_lod.glb"), Some(1));
        assert_eq!(lod_from_filename("idle_lod2.glb"), Some(2));
        assert_eq!(lod_from_filename("idle.glb"), None);
    }

    #[test]
    fn flags_external_gltf_resources() {
        let gltf = br#"{"asset":{"version":"2.0"},"buffers":[{"uri":"model.bin","byteLength":4}]}"#;
        let probe = probe_gltf(gltf, "model.gltf").unwrap();
        assert!(probe.external_resources);

        let embedded = br#"{"asset":{"version":"2.0"},"buffers":[{"uri":"data:application/octet-stream;base64,AA==","byteLength":1}]}"#;
        assert!(
            !probe_gltf(embedded, "model.gltf")
                .unwrap()
                .external_resources
        );
    }

    #[test]
    fn counts_animations() {
        let glb = build_glb(
            &json!({
                "asset": {"version": "2.0"},
                "buffers": [{"byteLength": 4}],
                "animations": [{"channels": [], "samplers": []}]
            }),
            &[1, 2, 3, 4],
        );
        assert_eq!(probe_gltf(&glb, "walk.glb").unwrap().animation_count, 1);
    }

    #[test]
    fn rejects_non_gltf_payloads() {
        assert!(probe_gltf(b"\x89PNG", "texture.png").is_err());
    }

    #[test]
    fn stamping_adds_detectable_metadata() {
        let glb = build_glb(
            &json!({"asset": {"version": "2.0"}, "buffers": [{"byteLength": 4}]}),
            &[9, 9, 9, 9],
        );
        let stamped = stamp_glb_metadata(&glb, Some(2), true).unwrap();
        let probe = probe_gltf(&stamped, "whatever.glb").unwrap();
        assert!(probe.has_draco);
        assert_eq!(probe.lod, Some(2));
        assert_eq!(stamped.len() % 4, 0);
        // The binary chunk must survive untouched.
        assert!(stamped.ends_with(&[9, 9, 9, 9]));
        assert!(!probe_gltf(&stamped, "x.glb").unwrap().external_resources);
        let _ = rebuild_glb(&[], 0, &[]);
    }

    #[test]
    fn repairs_embedded_image_mime_types_from_magic_bytes() {
        let png: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
        let glb = build_glb(
            &json!({
                "asset": {"version": "2.0"},
                "buffers": [{"byteLength": png.len()}],
                "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": png.len()}],
                "images": [{"name": "tex.jpg", "bufferView": 0, "mimeType": "image/jpeg"}]
            }),
            &png,
        );
        let normalized = normalize_glb_image_mime_types(&glb)
            .expect("normalize should parse")
            .expect("mime type should change");
        assert_ne!(normalized, glb);

        let chunks = super::parse_glb_chunks(&normalized).unwrap();
        let json_chunk = chunks
            .iter()
            .find(|chunk| chunk.kind == super::CHUNK_JSON)
            .unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(super::trimmed_json(json_chunk.data)).unwrap();
        assert_eq!(
            value
                .pointer("/images/0/mimeType")
                .and_then(serde_json::Value::as_str),
            Some("image/png")
        );
        assert!(normalized.ends_with(&png));
        assert!(
            normalize_glb_image_mime_types(&normalized)
                .expect("second pass should parse")
                .is_none()
        );
    }

    #[test]
    fn image_resize_produces_square_png() {
        let source = image::RgbaImage::from_pixel(640, 320, image::Rgba([10, 20, 30, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(source)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let resized = resize_image(bytes.get_ref(), 128, "image/png").unwrap();
        assert_eq!((resized.width, resized.height), (128, 128));
        assert_eq!(resized.mime, "image/png");
        let decoded = image::load_from_memory(&resized.bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (128, 128));
    }

    #[test]
    fn sha256_is_stable() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
