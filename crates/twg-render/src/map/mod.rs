mod anchors;
mod camera;
mod cursor;
mod environment;
mod input;
mod marker;
mod overlay;
mod tile_cache;
mod tiles;

use bevy::prelude::*;
use twg_map::{BaseLayer, CameraFlight, MapCameraState, MapCursor};

pub use anchors::GeoAnchor;
pub use camera::{CAMERA_FOV_Y, MapCamera};
pub use marker::DemoMarker;
pub use tiles::{Tile, TileWorld};

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct MapCameraResource(pub MapCameraState);

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct MapCursorResource(pub MapCursor);

#[derive(Resource, Default)]
pub struct CameraFlightState(pub Option<CameraFlight>);

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BaseLayerResource(pub BaseLayer);

/// Set by the UI crate so map input can ignore clicks over egui panels.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiPointerState {
    pub over_ui: bool,
}

#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct TileProxyConfig(pub Option<String>);

pub struct MapRenderPlugin;

impl Plugin for MapRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MapCameraResource(MapCameraState::default()))
            .insert_resource(MapCursorResource(MapCursor::default()))
            .init_resource::<CameraFlightState>()
            .insert_resource(BaseLayerResource::default())
            .init_resource::<UiPointerState>()
            .init_resource::<TileProxyConfig>()
            .init_resource::<TileWorld>()
            .add_systems(
                Startup,
                (
                    camera::spawn_camera,
                    environment::spawn_environment,
                    tiles::spawn_tile_assets,
                    overlay::spawn_overlays,
                    marker::spawn_demo_marker,
                ),
            )
            .add_systems(
                Update,
                (
                    input::map_input,
                    advance_camera_flight,
                    camera::update_camera_transform,
                    anchors::update_geo_anchors,
                    cursor::update_cursor,
                    tiles::sync_tiles,
                    tiles::update_error_tiles,
                    tiles::update_tile_transforms,
                    environment::update_ground,
                )
                    .chain(),
            );
    }
}

fn advance_camera_flight(
    time: Res<Time>,
    mut flight: ResMut<CameraFlightState>,
    mut camera: ResMut<MapCameraResource>,
) {
    let Some(active) = flight.0.as_mut() else {
        return;
    };
    let (state, done) = active.tick(time.delta_secs());
    camera.0 = state;
    if done {
        flight.0 = None;
    }
}
