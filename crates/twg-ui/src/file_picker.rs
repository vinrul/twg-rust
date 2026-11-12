//! Native file pickers shared by the asset upload surfaces.

#[cfg(not(target_arch = "wasm32"))]
use twg_contracts::assets::AssetCategory;

/// Opens a native dialog restricted to the formats `category` accepts.
#[cfg(not(target_arch = "wasm32"))]
pub fn pick_asset_file(category: AssetCategory) -> Option<std::path::PathBuf> {
    let dialog = rfd::FileDialog::new();
    match category {
        AssetCategory::Model => dialog
            .add_filter("Model glTF/GLB (*.glb, *.gltf)", &["glb", "gltf"])
            .set_title("Pilih model .glb/.gltf")
            .pick_file(),
        AssetCategory::Texture | AssetCategory::Icon => dialog
            .add_filter(
                "Gambar (*.png, *.jpg, *.jpeg, *.webp)",
                &["png", "jpg", "jpeg", "webp"],
            )
            .set_title("Pilih gambar")
            .pick_file(),
    }
}
