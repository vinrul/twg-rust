use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;

/// Storage abstraction for uploaded media.
///
/// Phase 6 ships a local-disk implementation. Object storage (S3-compatible)
/// can implement the same trait without touching the handlers.
#[async_trait]
pub trait UploadStorage: Send + Sync {
    async fn save(&self, filename: &str, bytes: &[u8]) -> std::io::Result<()>;
}

#[derive(Debug, Clone)]
pub struct LocalUploadStorage {
    directory: PathBuf,
}

impl LocalUploadStorage {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }
}

#[async_trait]
impl UploadStorage for LocalUploadStorage {
    async fn save(&self, filename: &str, bytes: &[u8]) -> std::io::Result<()> {
        fs::create_dir_all(&self.directory).await?;
        let path = self.directory.join(filename);
        fs::write(path, bytes).await
    }
}

/// Returns a safe ASCII filename component for an untrusted upload name.
pub fn sanitize_filename(raw: &str) -> String {
    let base = Path::new(raw)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("upload");
    let mut result = String::with_capacity(base.len().min(120));
    for character in base.chars().take(120) {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
            result.push(character);
        } else {
            result.push('_');
        }
    }
    let result = result.trim_matches('.').to_owned();
    if result.is_empty() {
        "upload".to_owned()
    } else {
        result
    }
}

/// Builds the stored filename. The legacy server prefixed the client filename
/// with a Unix timestamp; we use milliseconds plus the same prefix shape.
pub fn generated_filename(original: &str) -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{millis}-{}", sanitize_filename(original))
}

#[cfg(test)]
mod tests {
    use super::{generated_filename, sanitize_filename};

    #[test]
    fn strips_paths_and_control_characters() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("a b\tc.png"), "a_b_c.png");
        assert_eq!(sanitize_filename(".."), "upload");
    }

    #[test]
    fn generated_name_keeps_extension() {
        let name = generated_filename("photo.png");
        assert!(name.ends_with("-photo.png"), "{name}");
    }
}
