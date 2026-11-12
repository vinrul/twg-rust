mod anchors;
mod camera;
mod cursor;
mod environment;
mod input;
mod marker;
mod overlay;
mod sky;
mod tile_cache;
mod tiles;

use bevy::prelude::*;
use twg_map::{BaseLayer, CameraFlight, MapCameraState, MapCursor};

pub use anchors::GeoAnchor;
pub use camera::{CAMERA_FOV_Y, GIZMO_RENDER_LAYER, MapCamera};
pub use marker::DemoMarker;
pub use sky::SkyMode;
pub use tiles::{Tile, TileWorld};

/// Update systems that move the camera and re-anchor map objects. Editor
/// systems run before this set to spawn anchors and after it to pick with the
/// current camera transform.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MapUpdateSet;

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct MapCameraResource(pub MapCameraState);

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct MapCursorResource(pub MapCursor);

#[derive(Resource, Default)]
pub struct CameraFlightState(pub Option<CameraFlight>);

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BaseLayerResource(pub BaseLayer);

/// Set by the UI crate so map input can ignore clicks over egui panels and
/// keyboard shortcuts while a text field has focus.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiPointerState {
    pub over_ui: bool,
    pub keyboard: bool,
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
            .init_resource::<SkyMode>()
            .add_systems(
                Startup,
                (
                    camera::spawn_camera,
                    environment::spawn_environment,
                    sky::spawn_sky_cubemap,
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
                    camera::sync_gizmo_camera,
                    anchors::update_geo_anchors,
                    cursor::update_cursor,
                    tiles::sync_tiles,
                    tiles::update_error_tiles,
                    tiles::update_tile_transforms,
                    environment::update_ground,
                    sky::apply_sky_mode,
                    hide_demo_content_when_editing,
                )
                    .chain()
                    .in_set(MapUpdateSet),
            );
    }
}

#[allow(clippy::type_complexity)]
fn hide_demo_content_when_editing(
    mut commands: Commands,
    editor: Option<Res<crate::editor::EditorMode>>,
    demos: Query<Entity, Or<(With<DemoMarker>, With<overlay::DemoOverlay>)>>,
) {
    if editor.is_none_or(|editor| !editor.active) {
        return;
    }
    for entity in &demos {
        commands.entity(entity).despawn();
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
