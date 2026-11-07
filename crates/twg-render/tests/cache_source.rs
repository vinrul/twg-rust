//! Verifies that an **absolute** offline cache directory can be used as a Bevy
//! asset source root (the desktop registers it as `twg-cache://`).

use std::path::Path;

use bevy::asset::AsyncReadExt;
use bevy::asset::io::{AssetReader, file::FileAssetReader};
use bevy::tasks::block_on;

#[test]
fn file_asset_reader_serves_an_absolute_cache_root() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("twg-cache-source-{stamp}"));
    let nested = root.join("asset-id").join("animated");
    std::fs::create_dir_all(&nested).unwrap();
    let contents = b"glTF\x02\x00\x00\x00";
    std::fs::write(nested.join("draco_lod2.glb"), contents).unwrap();

    let reader = FileAssetReader::new(&root);
    let bytes = block_on(async {
        let mut file = reader
            .read(Path::new("asset-id/animated/draco_lod2.glb"))
            .await
            .expect("cache file is readable through the asset reader");
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await.unwrap();
        buffer
    });
    assert_eq!(bytes, contents);

    let missing = block_on(reader.read(Path::new("asset-id/missing.glb"))).is_err();
    assert!(missing);
    let _ = std::fs::remove_dir_all(root);
}
