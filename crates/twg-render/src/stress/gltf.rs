use bevy::asset::{Asset, AssetPath};
use bevy::gltf::{GltfAssetLabel, GltfLoaderSettings};
use bevy::prelude::*;

/// Loads a glTF label, disabling glTF JSON validation for Draco files.
///
/// Bevy's validator rejects `extensionsRequired = ["KHR_draco_mesh_compression"]`
/// before the extension handler runs, so compressed models only load with
/// `validate = false`.
pub fn load_gltf<A: Asset>(
    asset_server: &AssetServer,
    label: GltfAssetLabel,
    path: impl Into<AssetPath<'static>>,
    draco: bool,
) -> Handle<A> {
    let asset_path = label.from_asset(path.into());
    if draco {
        asset_server
            .load_builder()
            .with_settings(|settings: &mut GltfLoaderSettings| settings.validate = false)
            .load(asset_path)
    } else {
        asset_server.load(asset_path)
    }
}
