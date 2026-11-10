//! User-imported textures for shape materials.
//!
//! Textures picked from the local disk bypass the server asset library (only
//! admins can upload there). The file is copied into the offline asset cache
//! root under `textures/`, so the existing `twg-cache://` source can load it
//! without registering another Bevy asset source.

use std::path::{Path, PathBuf};

use bevy_egui::egui;

/// Prefix used in `buildings.keterangan` to mark a locally imported texture.
pub const LOCAL_TEXTURE_PREFIX: &str = "local:";

const LOCAL_TEXTURE_DIR: &str = "textures";
/// Subfolder for preview copies of textures uploaded to the server.
const UPLOADED_DIR: &str = "uploads";
#[cfg(not(target_arch = "wasm32"))]
const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
#[cfg(not(target_arch = "wasm32"))]
const PREVIEW_MAX_SIDE: u32 = 256;

/// Folder that holds user-imported textures.
pub fn local_texture_root() -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(twg_client::asset_cache::default_root().join(LOCAL_TEXTURE_DIR))
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

/// Finished preview downloads shared with the UI thread.
pub type PreviewDownloads = std::sync::Arc<std::sync::Mutex<Vec<(String, Option<PathBuf>)>>>;

/// True for references returned by `POST /private/upload-image`.
pub fn is_uploaded_reference(reference: &str) -> bool {
    uploaded_file_name(reference).is_some()
}

fn uploaded_file_name(reference: &str) -> Option<&str> {
    let name = reference.strip_prefix("/uploads/")?;
    (!name.is_empty() && !name.contains(['/', '\\', ':'])).then_some(name)
}

/// Local preview copy of a texture uploaded to the server, when this machine
/// has one (the uploader). Other machines still render the texture from the
/// server URL.
pub fn uploaded_texture_file(reference: &str) -> Option<PathBuf> {
    let name = uploaded_file_name(reference)?;
    let path = local_texture_root()?.join(UPLOADED_DIR).join(name);
    path.is_file().then_some(path)
}

/// Stores a copy of the uploaded image for the Inspector preview.
#[cfg(not(target_arch = "wasm32"))]
pub fn cache_uploaded_texture(reference: &str, source: &Path) -> Option<PathBuf> {
    let name = uploaded_file_name(reference)?;
    let root = local_texture_root()?.join(UPLOADED_DIR);
    std::fs::create_dir_all(&root).ok()?;
    let path = root.join(name);
    std::fs::copy(source, &path).ok()?;
    Some(path)
}

/// Downloads an uploaded texture in the background so the Inspector can show a
/// preview on machines that did not upload the file. Results are pushed to
/// `results` as `(reference, cached file)` pairs.
#[cfg(not(target_arch = "wasm32"))]
pub fn download_uploaded_texture(
    reference: &str,
    base_url: &str,
    results: &PreviewDownloads,
) -> bool {
    let Some(name) = uploaded_file_name(reference).map(str::to_owned) else {
        return false;
    };
    let Some(root) = local_texture_root().map(|root| root.join(UPLOADED_DIR)) else {
        return false;
    };
    let reference = reference.to_owned();
    let url = format!("{}{reference}", base_url.trim_end_matches('/'));
    let results = results.clone();
    std::thread::spawn(move || {
        let path = download_to(&root.join(name), &url);
        if let Ok(mut entries) = results.lock() {
            entries.push((reference, path));
        }
    });
    true
}

#[cfg(not(target_arch = "wasm32"))]
fn download_to(target: &Path, url: &str) -> Option<PathBuf> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(5)))
        .build()
        .into();
    let mut response = agent.get(url).call().ok()?;
    if !(200..300).contains(&response.status().as_u16()) {
        return None;
    }
    let bytes = response.body_mut().read_to_vec().ok()?;
    if bytes.is_empty() {
        return None;
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(target, bytes).ok()?;
    Some(target.to_path_buf())
}

/// Builds the multipart body expected by `POST /private/upload-image`.
#[cfg(not(target_arch = "wasm32"))]
pub fn multipart_image_body(source: &Path) -> Result<(String, Vec<u8>), String> {
    let filename = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("tekstur");
    let bytes = std::fs::read(source).map_err(|error| format!("Gagal membaca tekstur: {error}"))?;
    if bytes.is_empty() {
        return Err("File tekstur kosong.".to_owned());
    }
    let boundary = format!("----twg{}", uuid::Uuid::new_v4().simple());
    let mut body = Vec::with_capacity(bytes.len() + 256);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"{filename}\"\r\nContent-Type: {}\r\n\r\n",
            image_mime(source)
        )
        .as_bytes(),
    );
    body.extend_from_slice(&bytes);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Ok((format!("multipart/form-data; boundary={boundary}"), body))
}

#[cfg(not(target_arch = "wasm32"))]
fn image_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

/// Asset path used by the renderer, or `None` when the file is missing (web
/// build or a scenario copied from another machine).
pub fn local_texture_asset_path(reference: &str) -> Option<String> {
    let name = local_texture_file_name(reference)?;
    let path = local_texture_root()?.join(name);
    path.is_file().then(|| {
        format!(
            "{}://{LOCAL_TEXTURE_DIR}/{name}",
            twg_render::CACHE_ASSET_SOURCE
        )
    })
}

/// Absolute file path for local references, used by the egui preview.
pub fn local_texture_file(reference: &str) -> Option<PathBuf> {
    let name = local_texture_file_name(reference)?;
    let path = local_texture_root()?.join(name);
    path.is_file().then_some(path)
}

fn local_texture_file_name(reference: &str) -> Option<&str> {
    let name = reference.strip_prefix(LOCAL_TEXTURE_PREFIX)?;
    (!name.is_empty() && !name.contains(['/', '\\', ':'])).then_some(name)
}

/// Lists imported textures as `(reference, display name)` pairs.
pub fn scan_local_textures() -> Vec<(String, String)> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(root) = local_texture_root() else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(&root) else {
            return Vec::new();
        };
        let mut rows = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && is_image_file(path))
            .filter_map(|path| {
                let name = path.file_name()?.to_string_lossy().to_string();
                let label = local_texture_label(&name);
                Some((format!("{LOCAL_TEXTURE_PREFIX}{name}"), label))
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|(_, label)| label.to_lowercase());
        rows
    }
    #[cfg(target_arch = "wasm32")]
    {
        Vec::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn local_texture_label(file_name: &str) -> String {
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name);
    let label = stem.split_once("__").map_or(stem, |(_, label)| label);
    label.replace('_', " ")
}

/// Decodes an image file into an egui texture for the Inspector preview.
pub fn load_preview(ctx: &egui::Context, path: &Path) -> Option<egui::TextureHandle> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let image = image::open(path)
            .ok()?
            .thumbnail(PREVIEW_MAX_SIDE, PREVIEW_MAX_SIDE)
            .to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let pixels = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        Some(ctx.load_texture(
            format!("texture-preview:{}", path.display()),
            pixels,
            egui::TextureOptions::LINEAR,
        ))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (ctx, path);
        None
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{local_texture_file_name, local_texture_label};

    #[test]
    fn local_texture_labels_drop_the_uuid_prefix_and_extension() {
        assert_eq!(
            local_texture_label("0f8fad5b-d9cb-469f-a165-70867728950e__bukit_hijau.png"),
            "bukit hijau"
        );
        assert_eq!(local_texture_label("tanpa_prefix.jpg"), "tanpa prefix");
    }

    #[test]
    fn local_references_reject_traversal_and_foreign_ids() {
        assert!(local_texture_file_name("local:../evil.png").is_none());
        assert!(local_texture_file_name("local:sub/dir.png").is_none());
        assert!(local_texture_file_name("asset-id").is_none());
        assert_eq!(
            local_texture_file_name("local:bukit.png"),
            Some("bukit.png")
        );
    }

    #[test]
    fn uploaded_references_only_accept_server_file_names() {
        assert!(super::is_uploaded_reference("/uploads/123-bukit.png"));
        assert!(!super::is_uploaded_reference("/uploads/../evil.png"));
        assert!(!super::is_uploaded_reference("local:bukit.png"));
        assert!(!super::is_uploaded_reference("asset-id"));
    }
}
