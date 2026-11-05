mod camera;
mod cursor;
mod environment;
mod input;
mod marker;
mod tiles;

use bevy::prelude::*;
use twg_map::{BaseLayer, MapCameraState, MapCursor};

pub use camera::{CAMERA_FOV_Y, MapCamera};
pub use marker::DemoMarker;
pub use tiles::{Tile, TileWorld};

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct MapCameraResource(pub MapCameraState);

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct MapCursorResource(pub MapCursor);

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseLayerResource(pub BaseLayer);

impl Default for BaseLayerResource {
    fn default() -> Self {
        Self(BaseLayer::default())
    }
}

pub struct MapRenderPlugin;

impl Plugin for MapRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MapCameraResource(MapCameraState::default()))
            .insert_resource(MapCursorResource(MapCursor::default()))
            .insert_resource(BaseLayerResource::default())
            .init_resource::<TileWorld>()
            .add_systems(
                Startup,
                (
                    camera::spawn_camera,
                    environment::spawn_environment,
                    tiles::spawn_tile_assets,
                    marker::spawn_demo_marker,
                ),
            )
            .add_systems(
                Update,
                (
                    input::map_input,
                    camera::update_camera_transform,
                    cursor::update_cursor,
                    tiles::sync_tiles,
                    tiles::update_tile_transforms,
                    environment::update_ground,
                )
                    .chain(),
            );
    }
}
