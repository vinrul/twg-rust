use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use twg_map::{CameraFlight, MAX_ZOOM};

use crate::map::{
    CameraFlightState, MapCamera, MapCameraResource, MapCursorResource, UiPointerState,
};

use super::config::{MARKER_MODEL_SCALE, StressMarker};

const CLICK_DRAG_LIMIT: f32 = 6.0;
const PICK_RADIUS_PIXELS: f32 = 26.0;
const RING_WORLD_RADIUS: f32 = 3.0;
const MARKER_CENTER_HEIGHT: f32 = 1.0;

#[derive(Resource, Debug, Default)]
pub struct SelectionState {
    pub target: Option<Entity>,
    pub index: Option<usize>,
}

#[derive(Resource)]
pub(super) struct SelectionAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Component)]
pub(super) struct SelectionRing;

#[derive(Default)]
pub(super) struct ClickTracker {
    press: Option<Vec2>,
    max_drag: f32,
    last_click_time: f32,
    last_click_position: Vec2,
    has_last_click: bool,
}

pub fn spawn_selection_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Torus::new(0.85, 1.0));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.72, 0.05),
        unlit: true,
        ..default()
    });
    commands.insert_resource(SelectionAssets { mesh, material });
}

pub fn validate_selection(
    mut state: ResMut<SelectionState>,
    markers: Query<(), With<StressMarker>>,
    rings: Query<Entity, With<SelectionRing>>,
    mut commands: Commands,
) {
    let Some(target) = state.target else {
        return;
    };
    if markers.get(target).is_ok() {
        return;
    }
    state.target = None;
    state.index = None;
    for ring in &rings {
        commands.entity(ring).despawn();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_selection_click(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<MapCamera>>,
    ui_pointer: Res<UiPointerState>,
    time: Res<Time>,
    map_camera: Res<MapCameraResource>,
    cursor_geo: Res<MapCursorResource>,
    mut flight: ResMut<CameraFlightState>,
    markers: Query<(Entity, &StressMarker, &GlobalTransform)>,
    rings: Query<Entity, With<SelectionRing>>,
    assets: Res<SelectionAssets>,
    mut state: ResMut<SelectionState>,
    mut commands: Commands,
    mut tracker: Local<ClickTracker>,
) {
    let Some(cursor) = window.cursor_position() else {
        tracker.press = None;
        return;
    };

    if mouse.just_pressed(MouseButton::Left) {
        tracker.press = Some(cursor);
        tracker.max_drag = 0.0;
    }
    if mouse.pressed(MouseButton::Left)
        && let Some(press) = tracker.press
    {
        tracker.max_drag = tracker.max_drag.max(press.distance(cursor));
    }
    if !mouse.just_released(MouseButton::Left) {
        return;
    }

    let Some(press) = tracker.press.take() else {
        return;
    };
    let max_drag = tracker.max_drag;
    tracker.max_drag = 0.0;
    if ui_pointer.over_ui
        || max_drag > CLICK_DRAG_LIMIT
        || press.distance(cursor) > CLICK_DRAG_LIMIT
    {
        return;
    }

    let now = time.elapsed_secs();
    let is_double_click = tracker.has_last_click
        && now - tracker.last_click_time < 0.35
        && tracker.last_click_position.distance(cursor) < 8.0;
    tracker.last_click_time = now;
    tracker.last_click_position = cursor;
    tracker.has_last_click = true;

    if is_double_click {
        if let Some(point) = cursor_geo.0.position {
            let mut target = map_camera.0;
            target.center = point;
            target.zoom = (target.zoom + 1.0).min(MAX_ZOOM);
            flight.0 = Some(CameraFlight::new(map_camera.0, target, 0.8));
        }
        return;
    }

    let (camera, camera_transform) = *camera;
    let mut best: Option<(f32, Entity, usize)> = None;
    for (entity, marker, transform) in &markers {
        let center = transform.translation() + Vec3::Y * MARKER_CENTER_HEIGHT;
        let Ok(screen) = camera.world_to_viewport(camera_transform, center) else {
            continue;
        };
        let distance = screen.distance(cursor);
        if distance > PICK_RADIUS_PIXELS {
            continue;
        }
        if best.is_none_or(|(best_distance, _, _)| distance < best_distance) {
            best = Some((distance, entity, marker.index));
        }
    }

    for ring in &rings {
        commands.entity(ring).despawn();
    }

    match best {
        Some((_, entity, index)) => {
            state.target = Some(entity);
            state.index = Some(index);
            let mesh = assets.mesh.clone();
            let material = assets.material.clone();
            // The marker root is scaled by 0.02; compensate so the ring has a
            // stable 3 m world radius and sits just above the ground.
            let local_scale = RING_WORLD_RADIUS / MARKER_MODEL_SCALE;
            let local_height = 0.05 / MARKER_MODEL_SCALE;
            commands.entity(entity).with_children(|parent| {
                parent.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, local_height, 0.0)
                        .with_scale(Vec3::splat(local_scale)),
                    SelectionRing,
                ));
            });
        }
        None => {
            state.target = None;
            state.index = None;
        }
    }
}
