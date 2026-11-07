use bevy::asset::LoadState;
use bevy::gltf::{GltfAssetLabel, GltfMaterial};
use bevy::prelude::*;

use super::config::{MarkerModelOverride, MarkerModelSet};
use super::gltf::load_gltf;

#[derive(Resource, Default)]
pub struct MarkerAssets {
    pub static_mesh: Handle<Mesh>,
    pub animated_material_source: Handle<GltfMaterial>,
    pub static_material_source: Handle<GltfMaterial>,
    pub animated_material: Handle<StandardMaterial>,
    pub static_material: Handle<StandardMaterial>,
    pub materials_ready: bool,
    pub static_mesh_ready: bool,
    pub ready: bool,
}

impl MarkerAssets {
    pub fn load(
        asset_server: &AssetServer,
        models: &MarkerModelSet,
        models_override: &MarkerModelOverride,
    ) -> Self {
        let (still_path, still_draco) = models_override.resolve(models, false);
        let (animated_path, animated_draco) = models_override.resolve(models, true);
        let static_mesh = load_gltf(
            asset_server,
            GltfAssetLabel::Primitive {
                mesh: 0,
                primitive: 0,
            },
            still_path.clone(),
            still_draco,
        );
        let animated_material_source = load_gltf(
            asset_server,
            GltfAssetLabel::Material {
                index: 0,
                is_scale_inverted: false,
            },
            animated_path,
            animated_draco,
        );
        let static_material_source = load_gltf(
            asset_server,
            GltfAssetLabel::Material {
                index: 0,
                is_scale_inverted: false,
            },
            still_path,
            still_draco,
        );

        Self {
            static_mesh,
            animated_material_source,
            static_material_source,
            animated_material: Handle::default(),
            static_material: Handle::default(),
            materials_ready: false,
            static_mesh_ready: false,
            ready: false,
        }
    }
}

pub fn load_marker_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    models: Res<MarkerModelSet>,
    models_override: Res<MarkerModelOverride>,
) {
    let (animated_path, _) = models_override.resolve(&models, true);
    let (still_path, _) = models_override.resolve(&models, false);
    info!(
        "stress marker models: {animated_path} (animated), {still_path} (static), lod={}, draco={}, offline_override={}",
        models.lod,
        models.draco,
        models_override.is_set()
    );
    commands.insert_resource(MarkerAssets::load(&asset_server, &models, &models_override));
}

pub fn finalize_marker_assets(
    mut assets: ResMut<MarkerAssets>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    if assets.ready {
        return;
    }

    if !assets.materials_ready {
        let animated = gltf_materials.get(&assets.animated_material_source);
        let static_material = gltf_materials.get(&assets.static_material_source);
        if let (Some(animated), Some(static_material)) = (animated, static_material) {
            assets.animated_material = materials.add(to_standard(animated));
            assets.static_material = materials.add(to_standard(static_material));
            assets.materials_ready = true;
        }
    }

    if !assets.static_mesh_ready {
        assets.static_mesh_ready = matches!(
            asset_server.get_load_state(assets.static_mesh.id()),
            Some(LoadState::Loaded)
        );
    }

    assets.ready = assets.materials_ready && assets.static_mesh_ready;
    if assets.ready {
        info!("marker mesh and material assets ready");
    }
}

fn to_standard(material: &GltfMaterial) -> StandardMaterial {
    StandardMaterial {
        base_color: material.base_color,
        base_color_texture: material.base_color_texture.clone(),
        normal_map_texture: material.normal_map_texture.clone(),
        perceptual_roughness: material.perceptual_roughness,
        metallic: material.metallic,
        reflectance: material.reflectance,
        ..default()
    }
}
