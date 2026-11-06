use std::collections::HashMap;

use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::image::{Image, ImageFormatSetting, ImageLoaderSettings};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;
use twg_map::{BaseLayer, MapCameraState, TileId, TileSource, XyzTileSource, cover_tiles};

use super::tile_cache::LruCache;
use super::{BaseLayerResource, MapCameraResource, TileProxyConfig};

const MATERIAL_CACHE_LIMIT: usize = 512;
const IMAGE_CACHE_LIMIT: usize = 512;

type TileKey = (BaseLayer, TileId);

#[derive(Resource)]
pub struct TileWorld {
    pub tiles: HashMap<TileId, Entity>,
    materials: LruCache<TileKey, Handle<StandardMaterial>>,
    images: LruCache<TileKey, Handle<Image>>,
    pub mesh: Handle<Mesh>,
    pub placeholder: Handle<StandardMaterial>,
    pub layer: BaseLayer,
    pub proxy: Option<String>,
    pub source: XyzTileSource,
}

impl Default for TileWorld {
    fn default() -> Self {
        let layer = BaseLayer::default();
        Self {
            tiles: HashMap::new(),
            materials: LruCache::new(MATERIAL_CACHE_LIMIT),
            images: LruCache::new(IMAGE_CACHE_LIMIT),
            mesh: Handle::default(),
            placeholder: Handle::default(),
            layer,
            proxy: None,
            source: layer.tile_source(),
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Tile(pub TileId);

#[derive(Component, Clone)]
pub struct TileImage {
    pub image: Handle<Image>,
    pub material: Handle<StandardMaterial>,
}

pub fn spawn_tile_assets(
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut world: ResMut<TileWorld>,
) {
    world.mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(0.5)));

    let size = 64u32;
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let light = ((x / 8) + (y / 8)).is_multiple_of(2);
            let value = if light { 72 } else { 48 };
            let index = ((y * size + x) * 4) as usize;
            data[index] = value;
            data[index + 1] = value;
            data[index + 2] = value + 10;
            data[index + 3] = 255;
        }
    }
    let image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let image = images.add(image);
    world.placeholder = materials.add(StandardMaterial {
        base_color_texture: Some(image),
        unlit: true,
        perceptual_roughness: 1.0,
        ..default()
    });
}

#[allow(clippy::too_many_arguments)]
pub fn sync_tiles(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut world: ResMut<TileWorld>,
    state: Res<MapCameraResource>,
    layer: Res<BaseLayerResource>,
    proxy: Res<TileProxyConfig>,
    editor: Option<Res<crate::editor::EditorScene>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    if editor.as_ref().is_some_and(|editor| editor.indoor) {
        for entity in world.tiles.values() {
            commands.entity(*entity).despawn();
        }
        world.tiles.clear();
        return;
    }

    if world.layer != layer.0 || world.proxy != proxy.0 {
        for entity in world.tiles.values() {
            commands.entity(*entity).despawn();
        }
        world.tiles.clear();
        world.layer = layer.0;
        world.proxy = proxy.0.clone();
        world.source = layer.0.tile_source_with_proxy(proxy.0.as_deref());
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
    let placeholder = world.placeholder.clone();
    let source = world.source.clone();
    let current_layer = world.layer;
    for tile in cover.tiles {
        if world.tiles.contains_key(&tile) {
            continue;
        }

        let key = (current_layer, tile);
        let (material, image) = if let Some(material) = world.materials.get(&key).cloned() {
            let image = materials
                .get(&material)
                .and_then(|material| material.base_color_texture.clone())
                .unwrap_or_default();
            (material, image)
        } else {
            let image = if let Some(image) = world.images.get(&key).cloned() {
                image
            } else {
                let image: Handle<Image> = asset_server
                    .load_builder()
                    .with_settings(|settings: &mut ImageLoaderSettings| {
                        settings.format = ImageFormatSetting::Guess;
                    })
                    .load(source.url(tile));
                world.images.insert(key, image.clone());
                image
            };
            let material = materials.add(StandardMaterial {
                base_color_texture: Some(image.clone()),
                unlit: true,
                perceptual_roughness: 1.0,
                ..default()
            });
            world.materials.insert(key, material.clone());
            (material, image)
        };

        // Show a checker until the image resolves. `update_error_tiles` swaps
        // in the real material on success and leaves the checker on failure.
        let entity = commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(placeholder.clone()),
                tile_transform(&state, tile, max_zoom),
                Tile(tile),
                TileImage {
                    image,
                    material: material.clone(),
                },
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

pub fn update_error_tiles(
    asset_server: Res<AssetServer>,
    world: Res<TileWorld>,
    mut tiles: Query<(&TileImage, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (tile_image, mut material) in &mut tiles {
        match asset_server.get_load_state(tile_image.image.id()) {
            Some(LoadState::Failed(_)) if material.0 != world.placeholder => {
                material.0 = world.placeholder.clone();
            }
            Some(LoadState::Loaded) if material.0 != tile_image.material => {
                material.0 = tile_image.material.clone();
            }
            _ => {}
        }
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
    use twg_map::{GeoPoint, MapCameraState};

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
