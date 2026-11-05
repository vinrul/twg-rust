use bevy::asset::LoadState;
use bevy::gltf::{GltfAssetLabel, GltfMaterial};
use bevy::prelude::*;

use super::config::{ANIMATED_MODEL, STATIC_MODEL};

#[derive(Resource)]
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

pub fn load_marker_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let static_mesh = asset_server
        .load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(STATIC_MODEL));
    let animated_material_source = asset_server.load(
        GltfAssetLabel::Material {
            index: 0,
            is_scale_inverted: false,
        }
        .from_asset(ANIMATED_MODEL),
    );
    let static_material_source = asset_server.load(
        GltfAssetLabel::Material {
            index: 0,
            is_scale_inverted: false,
        }
        .from_asset(STATIC_MODEL),
    );

    commands.insert_resource(MarkerAssets {
        static_mesh,
        animated_material_source,
        static_material_source,
        animated_material: Handle::default(),
        static_material: Handle::default(),
        materials_ready: false,
        static_mesh_ready: false,
        ready: false,
    });
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
