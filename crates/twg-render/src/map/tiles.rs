use std::collections::HashMap;

use bevy::image::{ImageFormatSetting, ImageLoaderSettings};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use twg_map::{
    BaseLayer, MapCameraState, TileId, TileSource, XyzTileSource, cover_tiles,
};

use super::{BaseLayerResource, MapCameraResource};

const MATERIAL_CACHE_LIMIT: usize = 512;

#[derive(Resource)]
pub struct TileWorld {
    pub tiles: HashMap<TileId, Entity>,
    pub materials: HashMap<TileId, Handle<StandardMaterial>>,
    pub mesh: Handle<Mesh>,
    pub layer: BaseLayer,
    pub source: XyzTileSource,
}

impl Default for TileWorld {
    fn default() -> Self {
        let layer = BaseLayer::default();
        Self {
            tiles: HashMap::new(),
            materials: HashMap::new(),
            mesh: Handle::default(),
            layer,
            source: layer.tile_source(),
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Tile(pub TileId);

pub fn spawn_tile_assets(mut meshes: ResMut<Assets<Mesh>>, mut world: ResMut<TileWorld>) {
    world.mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(0.5)));
}

pub fn sync_tiles(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut world: ResMut<TileWorld>,
    state: Res<MapCameraResource>,
    layer: Res<BaseLayerResource>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    if world.layer != layer.0 {
        for entity in world.tiles.values() {
            commands.entity(*entity).despawn();
        }
        world.tiles.clear();
        world.materials.clear();
        world.layer = layer.0;
        world.source = layer.0.tile_source();
    }

    let state = state.0;
    let max_zoom = world.source.max_zoom();
    let viewport_height = window.physical_height() as f32;
    let aspect = if viewport_height > 0.0 {
        window.physical_width() as f32 / viewport_height
    } else {
        1.0
    };
    let radius = state.cover_radius_for(max_zoom, viewport_height, aspect);
    let Ok(cover) = cover_tiles(state.center, state.tile_zoom_for(max_zoom), radius) else {
        return;
    };

    let stale = world
        .tiles
        .iter()
        .filter(|(tile, _)| !cover.tiles.contains(tile))
        .map(|(tile, entity)| (*tile, *entity))
        .collect::<Vec<_>>();
    for (tile, entity) in stale {
        commands.entity(entity).despawn();
        world.tiles.remove(&tile);
    }

    let mesh = world.mesh.clone();
    let source = world.source.clone();
    for tile in cover.tiles {
        if world.tiles.contains_key(&tile) {
            continue;
        }

        let material = if let Some(material) = world.materials.get(&tile) {
            material.clone()
        } else {
            if world.materials.len() >= MATERIAL_CACHE_LIMIT {
                world.materials.clear();
            }
            let image: Handle<Image> = asset_server
                .load_builder()
                .with_settings(|settings: &mut ImageLoaderSettings| {
                    settings.format = ImageFormatSetting::Guess;
                })
                .load(source.url(tile));
            let material = materials.add(StandardMaterial {
                base_color_texture: Some(image),
                unlit: true,
                perceptual_roughness: 1.0,
                ..default()
            });
            world.materials.insert(tile, material.clone());
            material
        };

        let entity = commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                tile_transform(&state, tile, max_zoom),
                Tile(tile),
                Name::new(format!(
                    "{} tile {}/{}/{}",
                    layer.0.label(),
                    tile.zoom,
                    tile.x,
                    tile.y
                )),
            ))
            .id();
        world.tiles.insert(tile, entity);
    }
}

pub fn update_tile_transforms(
    state: Res<MapCameraResource>,
    world: Res<TileWorld>,
    mut tiles: Query<(&Tile, &mut Transform)>,
) {
    let state = state.0;
    let max_zoom = world.source.max_zoom();
    for (tile, mut transform) in &mut tiles {
        *transform = tile_transform(&state, tile.0, max_zoom);
    }
}

fn tile_transform(state: &MapCameraState, tile: TileId, max_zoom: u8) -> Transform {
    let (center_x, center_y) = state.tile_fraction_for(max_zoom);
    let size = state.tile_world_size_for(max_zoom);
    let x = (f64::from(tile.x) - center_x) as f32 * size;
    let z = (f64::from(tile.y) - center_y) as f32 * size;
    Transform::from_xyz(x, 0.0, z).with_scale(Vec3::splat(size))
}

#[cfg(test)]
mod tests {
    use super::tile_transform;
    use twg_map::{GeoPoint, MapCameraState, TileId};

    #[test]
    fn tile_positions_are_metric() {
        let mut state = MapCameraState::new(GeoPoint::new(115.225, -8.648));
        state.set_zoom(19.0);
        let center = state.center.tile(19).unwrap();
        let transform = tile_transform(&state, center, 19);
        assert!(transform.translation.x.abs() < 80.0);
        assert!(transform.translation.z.abs() < 80.0);
        assert!((transform.scale.x - state.tile_world_size_for(19)).abs() < 0.01);
    }
}
