//! End-to-end sync test for the offline cache against a tiny local HTTP server
//! (no network access required). Desktop-only module, so host builds cover it.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use twg_client::asset_cache::{AssetCache, SyncProgress};
use twg_contracts::SingleResponse;
use twg_contracts::Timestamp;
use twg_contracts::assets::{
    Asset, AssetCategory, AssetFile, AssetManifest, AssetModelKind, AssetMoveMode, AssetSource,
    AssetSourceSlot, AssetVariant, AssetVariantKind,
};
use uuid::Uuid;

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn sample_asset() -> (Asset, Vec<u8>, Vec<u8>) {
    let id = Uuid::from_u128(42);
    let original = b"original-glb-bytes".to_vec();
    let variant = b"draco-lod2-bytes".to_vec();
    let asset = Asset {
        id,
        name: "walk".to_owned(),
        category: AssetCategory::Model,
        model_kind: Some(AssetModelKind::Animated),
        move_mode: AssetMoveMode::Free,
        group_id: None,
        group_name: None,
        sources: vec![
            AssetSource {
                slot: AssetSourceSlot::Idle,
                file: AssetFile {
                    filename: "original.glb".to_owned(),
                    url: format!("/assets/{id}/idle/original.glb"),
                    mime: "model/gltf-binary".to_owned(),
                    size_bytes: original.len() as i64,
                    sha256: sha256_hex(&original),
                },
                has_draco: false,
                lod: None,
                variants: vec![AssetVariant {
                    kind: AssetVariantKind::DracoLod2,
                    filename: "draco_lod2.glb".to_owned(),
                    url: format!("/assets/{id}/idle/draco_lod2.glb"),
                    mime: "model/gltf-binary".to_owned(),
                    size_bytes: variant.len() as i64,
                    sha256: sha256_hex(&variant),
                    width: None,
                    height: None,
                    reused: false,
                }],
            },
            AssetSource {
                slot: AssetSourceSlot::Animated,
                file: AssetFile {
                    filename: "original.glb".to_owned(),
                    url: format!("/assets/{id}/animated/original.glb"),
                    mime: "model/gltf-binary".to_owned(),
                    size_bytes: original.len() as i64,
                    sha256: sha256_hex(&original),
                },
                has_draco: false,
                lod: None,
                variants: Vec::new(),
            },
        ],
        version: 3,
        created_at: Timestamp::from(chrono::Utc::now()),
        updated_at: Timestamp::from(chrono::Utc::now()),
    };
    (asset, original, variant)
}

/// Serves the given routes for `expected` requests, then stops.
fn serve(routes: Vec<(String, Vec<u8>)>, expected: usize) -> (String, Arc<Mutex<usize>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("local addr");
    listener.set_nonblocking(true).expect("nonblocking");
    let counter = Arc::new(Mutex::new(0usize));
    let served = counter.clone();
    std::thread::spawn(move || {
        let mut handled = 0usize;
        while handled < expected {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
                    let mut buffer = [0u8; 4096];
                    let Ok(read) = stream.read(&mut buffer) else {
                        continue;
                    };
                    let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("/")
                        .to_owned();
                    let response = match routes.iter().find(|(route, _)| route == &path) {
                        Some((_, body)) => {
                            let mut response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            )
                            .into_bytes();
                            response.extend_from_slice(body);
                            response
                        }
                        None => {
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                                .to_vec()
                        }
                    };
                    let _ = stream.write_all(&response);
                    let _ = stream.flush();
                    handled += 1;
                    *served.lock().expect("counter") += 1;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    (format!("http://{address}"), counter)
}

#[test]
fn sync_mirrors_files_and_manifest_for_animated_models() {
    let (asset, original, variant) = sample_asset();
    let manifest = AssetManifest {
        generated_at: Timestamp::from(chrono::Utc::now()),
        assets: vec![asset.clone()],
    };
    let routes = vec![
        (
            format!("/assets/{}/idle/original.glb", asset.id),
            original.clone(),
        ),
        (
            format!("/assets/{}/idle/draco_lod2.glb", asset.id),
            variant.clone(),
        ),
        (
            format!("/assets/{}/animated/original.glb", asset.id),
            original.clone(),
        ),
    ];
    let (base_url, served) = serve(routes, 3);

    let root = std::env::temp_dir().join(format!("twg-sync-test-{}", Uuid::new_v4()));
    let mut cache = AssetCache::with_root(&root);
    let progress = Mutex::new(SyncProgress::default());
    let report = cache
        .sync(&manifest, &base_url, &progress)
        .expect("sync should succeed");
    assert_eq!(report.downloaded, 3);
    assert_eq!(*served.lock().unwrap(), 3);

    let idle = cache.local_path(asset.id, AssetSourceSlot::Idle, "original.glb");
    assert_eq!(std::fs::read(&idle).unwrap(), original);
    let animated = cache.local_path(asset.id, AssetSourceSlot::Animated, "original.glb");
    assert_eq!(std::fs::read(&animated).unwrap(), original);
    assert!(cache.is_current(&manifest));
    assert!(
        cache
            .resolve(&asset, AssetSourceSlot::Idle, true)
            .unwrap()
            .1
            .ends_with("draco_lod2.glb")
    );

    // Reloading from disk restores the manifest written by the sync.
    let reloaded = AssetCache::with_root(&root);
    assert!(reloaded.is_current(&manifest));
    assert!(reloaded.plan(&manifest).is_empty());

    let _ = std::fs::remove_dir_all(root);
}

/// Opt-in check against a live local server:
///
/// ```sh
/// TWG_TEST_MANIFEST=target/manifest.json TWG_TEST_BASE=http://127.0.0.1:8099 \
///   cargo test -p twg-client --test asset_cache_sync sync_against_live_manifest
/// ```
#[test]
fn sync_against_live_manifest() {
    let (Ok(manifest_path), Ok(base_url)) = (
        std::env::var("TWG_TEST_MANIFEST"),
        std::env::var("TWG_TEST_BASE"),
    ) else {
        return;
    };
    let bytes = std::fs::read(&manifest_path).expect("read manifest fixture");
    let envelope: SingleResponse<AssetManifest> =
        serde_json::from_slice(&bytes).expect("manifest envelope");
    let manifest = envelope.data;
    assert!(!manifest.assets.is_empty(), "manifest is empty");

    let root = std::env::var("TWG_TEST_CACHE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join(format!("twg-live-sync-{}", Uuid::new_v4())));
    let mut cache = AssetCache::with_root(&root);
    let progress = Mutex::new(SyncProgress::default());
    let report = cache
        .sync(&manifest, &base_url, &progress)
        .expect("live sync should succeed");
    assert!(report.downloaded > 0);
    for asset in &manifest.assets {
        for source in &asset.sources {
            let path = cache.local_path(asset.id, source.slot, &source.file.filename);
            assert!(path.is_file(), "missing {}", path.display());
            for variant in &source.variants {
                let path = cache.local_path(asset.id, source.slot, &variant.filename);
                assert!(path.is_file(), "missing variant {}", path.display());
            }
        }
    }
    // A second pass must be a no-op.
    let again = cache
        .sync(&manifest, &base_url, &progress)
        .expect("second sync");
    assert_eq!(again.downloaded, 0);
    if std::env::var("TWG_TEST_CACHE").is_err() {
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn sync_rejects_checksum_mismatch() {
    let (mut asset, original, _variant) = sample_asset();
    asset.sources.truncate(1);
    asset.sources[0].variants.clear();
    asset.sources[0].file.sha256 = "deadbeef".to_owned();
    let manifest = AssetManifest {
        generated_at: Timestamp::from(chrono::Utc::now()),
        assets: vec![asset.clone()],
    };
    let routes = vec![(format!("/assets/{}/idle/original.glb", asset.id), original)];
    let (base_url, _served) = serve(routes, 1);
    let root = std::env::temp_dir().join(format!("twg-sync-test-bad-{}", Uuid::new_v4()));
    let mut cache = AssetCache::with_root(&root);
    let progress = Mutex::new(SyncProgress::default());
    let error = cache
        .sync(&manifest, &base_url, &progress)
        .expect_err("checksum mismatch must fail");
    assert!(error.contains("checksum"), "{error}");
    assert!(cache.manifest().is_none());
    let _ = std::fs::remove_dir_all(root);
}
