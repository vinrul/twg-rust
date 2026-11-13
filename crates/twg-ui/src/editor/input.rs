use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::egui;
use twg_map::{CameraFlight, GeoPoint};
use twg_render::{
    BaseLayerResource, CameraFlightState, EditorGizmoAxis, EditorGizmoMode, EditorMode,
    EditorPlayFrame, EditorPlayUnit, EditorScene, EditorShapeKind, MapCamera, MapCameraResource,
    MapCursorResource, SkyMode, UiPointerState,
};

use crate::state::ClientUi;

use super::model::{
    Draft, DragState, EditorKind, EditorState, EditorTool, MIN_MARKER_SCALE, ObjectKey, ObjectType,
    PlaybackState, offset_object,
};

/// Set by the egui pass so editor shortcuts do not fire while a text field has
/// focus.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct EditorInputFocus {
    pub keyboard: bool,
}

pub fn sync_editor_mode(
    client: Res<ClientUi>,
    mut mode: ResMut<EditorMode>,
    mut sky: ResMut<SkyMode>,
) {
    let active = matches!(
        client.state.screen,
        twg_client::session::Screen::MapEditor | twg_client::session::Screen::Preview
    );
    mode.active = active;
    mode.preview = active && client.editor.preview;
    mode.particles_animate = client.editor.particle_preview;
    if *sky != client.editor.sky {
        *sky = client.editor.sky;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn editor_input(
    mut client: ResMut<ClientUi>,
    mut render: ResMut<EditorScene>,
    mut mode: ResMut<EditorMode>,
    pointer: Res<UiPointerState>,
    focus: Res<EditorInputFocus>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera_query: Single<(&Camera, &GlobalTransform), With<MapCamera>>,
    mut map_camera: ResMut<MapCameraResource>,
    mut flight: ResMut<CameraFlightState>,
    mut layers: ResMut<BaseLayerResource>,
    cursor: Res<MapCursorResource>,
    diagnostics: Res<DiagnosticsStore>,
    mut play_frame: ResMut<EditorPlayFrame>,
) {
    let in_editor = matches!(
        client.state.screen,
        twg_client::session::Screen::MapEditor | twg_client::session::Screen::Preview
    );
    if !in_editor {
        if mode.active {
            mode.active = false;
        }
        play_frame.active = false;
        play_frame.units.clear();
        if render.revision != client.editor.render_revision || !render.objects.is_empty() {
            *render = EditorScene {
                revision: client.editor.render_revision,
                ..Default::default()
            };
        }
        return;
    }
    mode.active = true;
    mode.preview = client.editor.preview;
    mode.particles_animate = client.editor.particle_preview;

    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !focus.keyboard && control && keys.just_pressed(KeyCode::KeyS) {
        client.editor_save();
    }

    let (camera, camera_transform) = *camera_query;
    let base_url = client.base_url.clone();
    let editor = &mut client.editor;
    editor.map_state = map_camera.0;
    editor.base_url = base_url;
    editor.cursor_geo = cursor.0.position;
    editor.fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or_default();

    apply_requests(editor, &mut map_camera, &mut flight, &mut layers);

    // Play mode: start/stop the RTS state with the Play toggle and advance the
    // moving units before they are projected for picking and overlays.
    if editor.preview && editor.loaded {
        if !editor.play.active {
            editor.start_play();
        }
        editor.play_tick(time.delta_secs_f64());
    } else if editor.play.active {
        editor.stop_play();
    }

    let viewport = if editor.viewport_rect.width() > 1.0 && editor.viewport_rect.height() > 1.0 {
        editor.viewport_rect
    } else {
        egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(window.width(), window.height()),
        )
    };
    let pointer_pos = window
        .cursor_position()
        .map(|position| egui::pos2(position.x, position.y));
    let pointer_in_viewport = pointer_pos.is_some_and(|position| viewport.contains(position));
    let inside_viewport = pointer_in_viewport && !pointer.over_ui;
    let editing = !editor.preview && editor.loaded;

    // Track the ground point whenever the pointer is over the map, including
    // while a panel drag is in progress (egui owns the pointer then).
    editor.cursor_world = pointer_pos
        .filter(|position| viewport.contains(*position))
        .and_then(|position| ground_point(camera, camera_transform, position, editor));
    if let Some(world) = editor.cursor_world {
        editor.cursor_geo = editor.map_state.world_to_geo(world.x, world.z);
    }

    project_objects(editor, camera, camera_transform);
    editor.draft_screen = editor
        .draft
        .points()
        .iter()
        .filter_map(|point| project_geo(editor, camera, camera_transform, *point))
        .collect();
    editor.measure_screen.clear();
    if let Some(start) = editor.measure.start {
        editor
            .measure_screen
            .extend(project_geo(editor, camera, camera_transform, start));
        if let Some(end) = editor.measure.end.or(editor.cursor_geo) {
            editor
                .measure_screen
                .extend(project_geo(editor, camera, camera_transform, end));
        }
    }
    let hover_axis = if inside_viewport && editing {
        gizmo_axis_at(editor, camera, camera_transform, pointer_pos.unwrap())
    } else {
        None
    };
    if hover_axis != editor.hover_axis {
        editor.hover_axis = hover_axis;
        // The gizmo highlight is part of the render scene.
        editor.touch();
    }
    editor.hover_key = if inside_viewport && editing && hover_axis.is_none() {
        pick_object(editor, pointer_pos.unwrap())
    } else {
        None
    };

    if editing && editor.asset_drag.is_some() {
        // An asset card is being dragged out of the bottom panel. egui owns the
        // pointer, so the map must not treat the same press as a click, and the
        // drop is resolved here where the camera can produce a ground point.
        editor.hover_axis = None;
        editor.hover_key = None;
        if mouse.just_released(MouseButton::Left) {
            if let (Some(draft), Some(position)) = (editor.asset_drag.take(), pointer_pos)
                && viewport.contains(position)
                && let Some(world) = ground_point(camera, camera_transform, position, editor)
                && let Some(geo) = editor.map_state.world_to_geo(world.x, world.z)
            {
                editor.add_asset_marker(&draft, editor.snap_point(geo));
            }
        } else if !mouse.pressed(MouseButton::Left) {
            // egui cancelled the drag (Esc) or the button went up off-window.
            editor.asset_drag = None;
        }
    } else if editing {
        // The context menu is dismissed by its own click-outside handling in the
        // UI pass, so a left press here must not clear it: egui needs both the
        // press and the release to deliver a button click.
        if inside_viewport && mouse.just_pressed(MouseButton::Right) {
            editor.right_press = pointer_pos;
        }
        if inside_viewport
            && mouse.just_released(MouseButton::Right)
            && let (Some(start), Some(position)) = (editor.right_press.take(), pointer_pos)
            && start.distance(position) < 5.0
            && let Some(key) = pick_object(editor, position)
                .or_else(|| pick_object_ignoring_lock(editor, position))
        {
            // A locked route gets its context menu without being selected.
            if !editor.is_locked(key) {
                editor.select_only(key);
            }
            editor.context_menu = Some((position, key));
        }
        keyboard_shortcuts(editor, &keys, &mouse, focus.keyboard);
        if inside_viewport {
            mouse_input(
                editor,
                &keys,
                &mouse,
                pointer_pos.unwrap(),
                camera,
                camera_transform,
                time.elapsed_secs_f64(),
            );
        } else if mouse.just_released(MouseButton::Left) {
            editor.drag = DragState::None;
        }
    } else {
        editor.context_menu = None;
        editor.asset_drag = None;
        if editor.preview && editor.loaded {
            play_input(
                editor,
                &mouse,
                &keys,
                pointer_pos,
                inside_viewport,
                camera,
                camera_transform,
            );
        } else {
            // Only reset the drag when Play is not running: resetting it here
            // every frame would cancel a Play gizmo drag right after it starts.
            editor.drag = DragState::None;
            editor.hover_key = None;
            editor.hover_axis = None;
        }
    }

    play_frame.active = editor.play.active;
    play_frame.units.clear();
    if editor.play.active {
        let animation_running = editor
            .play
            .animations
            .iter()
            .filter(|animation| {
                animation.state == PlaybackState::Playing && !animation.is_finished()
            })
            .filter_map(|animation| animation.marker_key)
            .collect::<std::collections::BTreeSet<_>>();
        play_frame
            .units
            .extend(editor.play.units.iter().map(|(key, unit)| EditorPlayUnit {
                key: *key,
                lon: unit.position.longitude,
                lat: unit.position.latitude,
                altitude: unit.altitude,
                heading_deg: unit.heading_deg,
                moving: unit.target.is_some() || animation_running.contains(key),
                animated: unit.animated,
            }));
        // Limited assets are not orderable units, but their root still has to
        // follow the gizmo while it is dragged.
        play_frame.units.extend(
            editor
                .play
                .limited
                .iter()
                .map(|(key, unit)| EditorPlayUnit {
                    key: *key,
                    lon: unit.position.longitude,
                    lat: unit.position.latitude,
                    altitude: unit.altitude,
                    heading_deg: unit.heading_deg,
                    moving: false,
                    animated: false,
                }),
        );
    }

    if render.revision != editor.render_revision {
        *render = editor.build_scene();
    }
}

fn apply_requests(
    editor: &mut EditorState,
    map_camera: &mut MapCameraResource,
    flight: &mut CameraFlightState,
    layers: &mut BaseLayerResource,
) {
    if let Some(layer) = editor.requested_layer.take() {
        layers.0 = layer;
        editor.base_layer = layer;
    }
    if let Some(target) = editor.requested_flight.take() {
        flight.0 = Some(CameraFlight::new(map_camera.0, target, 0.45));
    }
    if let Some(target) = editor.requested_camera.take() {
        map_camera.0 = target;
        editor.map_state = target;
    }
    if let Some(zoom) = editor.requested_zoom.take() {
        map_camera.0.set_zoom(map_camera.0.zoom + zoom);
    }
    if editor.requested_north {
        editor.requested_north = false;
        let mut target = map_camera.0;
        target.bearing_degrees = 0.0;
        flight.0 = Some(CameraFlight::new(map_camera.0, target, 0.45));
    }
}

/// RTS input used while the editor is in Play mode. `Free` assets and animated
/// units can be selected (click / Shift+click / marquee) and ordered to move
/// with a right click. `Limited` assets are selected with a click and moved by
/// dragging their 3D gizmo. `None` assets are neither pickable nor draggable.
#[allow(clippy::too_many_arguments)]
fn play_input(
    editor: &mut EditorState,
    mouse: &ButtonInput<MouseButton>,
    keys: &ButtonInput<KeyCode>,
    pointer: Option<egui::Pos2>,
    inside_viewport: bool,
    camera: &Camera,
    transform: &GlobalTransform,
) {
    let additive = keys.pressed(KeyCode::ShiftLeft)
        || keys.pressed(KeyCode::ShiftRight)
        || keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight);

    if !inside_viewport {
        editor.play.marquee = None;
        editor.play.right_press = None;
        editor.hover_key = None;
        editor.hover_axis = None;
        if !mouse.pressed(MouseButton::Left) {
            editor.drag = DragState::None;
        }
        return;
    }
    let Some(position) = pointer else {
        return;
    };
    // Play gizmos do not highlight: refreshing the material would rebuild the
    // scene and reset every animated Play instance. The handles still work.
    editor.hover_axis = None;
    editor.hover_key = editor.play_pick(position);

    if mouse.just_pressed(MouseButton::Left) {
        // Grabbing a gizmo handle takes priority over selecting the model.
        if let Some(axis) = gizmo_axis_at(editor, camera, transform, position)
            && matches!(
                axis,
                EditorGizmoAxis::X | EditorGizmoAxis::Y | EditorGizmoAxis::Z
            )
            && let (Some(world), Some((key, original, original_altitude))) =
                (editor.cursor_world, editor.play_gizmo_target())
        {
            editor.drag = DragState::PlayGizmo {
                axis,
                key,
                original,
                original_altitude,
                start_world: world,
                start_screen: position,
            };
            editor.play.marquee = None;
            editor.play.right_press = None;
            return;
        }
        let previous_gizmo = editor.play_gizmo_target().map(|(key, _, _)| key);
        if let Some(key) = editor.play_pick(position) {
            if editor.play.limited.contains_key(&key) {
                editor.play_select_limited(key);
            } else if additive {
                editor.play_toggle_selected(key);
            } else {
                editor.play_select_only(key);
            }
            editor.play.marquee = None;
        } else {
            if !additive {
                editor.play_clear_selection();
            }
            editor.play.marquee = Some((position, position));
        }
        // The gizmo is part of the render scene, so showing, hiding, or moving
        // it to another limited asset needs one rebuild (never per drag frame).
        if previous_gizmo != editor.play_gizmo_target().map(|(key, _, _)| key) {
            editor.touch();
        }
    }
    if mouse.pressed(MouseButton::Left) {
        if matches!(editor.drag, DragState::PlayGizmo { .. }) {
            update_drag(editor, position, camera, transform);
        } else if let Some((_, current)) = editor.play.marquee.as_mut() {
            *current = position;
        }
    }
    if mouse.just_released(MouseButton::Left) {
        if matches!(editor.drag, DragState::PlayGizmo { .. }) {
            editor.drag = DragState::None;
        }
        if let Some((start, current)) = editor.play.marquee.take() {
            let rect = egui::Rect::from_two_pos(start, current);
            if rect.width() > 3.0 || rect.height() > 3.0 {
                editor.play_box_select(rect, additive);
            }
        }
    }

    if mouse.just_pressed(MouseButton::Right) {
        editor.play.right_press = Some(position);
    }
    if mouse.just_released(MouseButton::Right)
        && let Some(start) = editor.play.right_press.take()
        && start.distance(position) <= 4.0
        && !editor.selected.is_empty()
        && let Some(world) = ground_point_raw(camera, transform, position)
        && let Some(target) = editor.map_state.world_to_geo(world.x, world.z)
    {
        editor.play_move_selected(target);
    }

    if keys.just_pressed(KeyCode::Escape) {
        let had_gizmo = editor.play_gizmo_target().is_some();
        editor.play_clear_selection();
        if had_gizmo {
            editor.touch();
        }
    }
}

/// Ground point under the cursor without the editor's snap grid.
fn ground_point_raw(
    camera: &Camera,
    transform: &GlobalTransform,
    position: egui::Pos2,
) -> Option<Vec3> {
    let ray = camera
        .viewport_to_world(transform, egui_vec2(position))
        .ok()?;
    ray.plane_intersection_point(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))
}

fn ground_point(
    camera: &Camera,
    transform: &GlobalTransform,
    position: egui::Pos2,
    editor: &EditorState,
) -> Option<Vec3> {
    let ray = camera
        .viewport_to_world(transform, egui_vec2(position))
        .ok()?;
    let point = ray.plane_intersection_point(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
    Some(match editor.snap_meters {
        0.0 => point,
        snap => Vec3::new(
            (point.x / snap).round() * snap,
            point.y,
            (point.z / snap).round() * snap,
        ),
    })
}

fn egui_vec2(position: egui::Pos2) -> Vec2 {
    Vec2::new(position.x, position.y)
}

fn project_objects(editor: &mut EditorState, camera: &Camera, transform: &GlobalTransform) {
    editor.projected.clear();
    editor.labels.clear();
    editor.text_overlays.clear();
    for object in &editor.objects {
        if !object.visible || !editor.visible.allows(object.object_type()) {
            continue;
        }
        let points = match editor.play.units.get(&object.key) {
            Some(unit) => vec![unit.position],
            None => match editor.play.limited.get(&object.key) {
                Some(unit) => vec![unit.position],
                None => object.kind.points(),
            },
        };
        let altitudes = match &object.kind {
            super::model::EditorKind::Line(line) => line
                .altitudes
                .iter()
                .map(|value| *value as f32)
                .collect::<Vec<_>>(),
            super::model::EditorKind::Text(text) => vec![text.altitude as f32],
            super::model::EditorKind::Building(building) => {
                vec![building.altitude as f32; points.len()]
            }
            _ => Vec::new(),
        };
        let mut screen_points = Vec::with_capacity(points.len());
        for (index, point) in points.iter().enumerate() {
            let Some(world) =
                world_of_at(editor, *point, altitudes.get(index).copied().unwrap_or(0.0))
            else {
                continue;
            };
            let Ok(screen) = camera.world_to_viewport(transform, world) else {
                continue;
            };
            screen_points.push(egui::pos2(screen.x, screen.y));
        }
        if screen_points.is_empty() {
            continue;
        }
        let label_position = screen_points[0];
        editor.projected.push((
            object.key,
            object.object_type(),
            screen_points,
            editor.selected.contains(&object.key),
        ));
        if let super::model::EditorKind::Text(text) = &object.kind {
            // Text is content, not a debug label: it stays visible in Play mode
            // and its size follows the Font field.
            editor.text_overlays.push((
                label_position,
                text.text.clone(),
                super::model::parse_hex_color(&object.kind.color()),
                text.font_size.max(6.0) as f32,
            ));
        }
        if editor.labels_visible && !editor.preview {
            let label = match &object.kind {
                super::model::EditorKind::Marker(_) if object.visible => Some(object.name.clone()),
                _ => None,
            };
            if let Some(label) = label {
                editor.labels.push((
                    label_position,
                    label,
                    super::model::parse_hex_color(&object.kind.color()),
                ));
            }
        }
    }
    if editor.labels.len() > 400 {
        editor.labels.truncate(400);
    }
    if editor.text_overlays.len() > 1_000 {
        editor.text_overlays.truncate(1_000);
    }

    // Ground rings for the RTS selection overlay: sample a circle in meters
    // around every unit and project it, so the ring lies on the map plane
    // instead of facing the camera.
    let mut rings = Vec::new();
    if editor.preview && editor.play.active {
        const SEGMENTS: usize = 32;
        for (key, unit) in &editor.play.units {
            let mut ring = Vec::with_capacity(SEGMENTS);
            for index in 0..SEGMENTS {
                let angle = index as f64 / SEGMENTS as f64 * std::f64::consts::TAU;
                let Ok(point) = GeoPoint::from_local_offset(
                    unit.position,
                    unit.radius * angle.cos(),
                    unit.radius * angle.sin(),
                ) else {
                    continue;
                };
                if let Some(screen) = project_geo(editor, camera, transform, point) {
                    ring.push(screen);
                }
            }
            if ring.len() >= 3 {
                rings.push((*key, ring));
            }
        }
        for (key, unit) in &editor.play.limited {
            let mut ring = Vec::with_capacity(SEGMENTS);
            for index in 0..SEGMENTS {
                let angle = index as f64 / SEGMENTS as f64 * std::f64::consts::TAU;
                let Ok(point) = GeoPoint::from_local_offset(
                    unit.position,
                    unit.radius * angle.cos(),
                    unit.radius * angle.sin(),
                ) else {
                    continue;
                };
                if let Some(screen) = project_geo(editor, camera, transform, point) {
                    ring.push(screen);
                }
            }
            if ring.len() >= 3 {
                rings.push((*key, ring));
            }
        }
    }
    editor.play_rings = rings;
}

fn world_of(editor: &EditorState, point: GeoPoint) -> Option<Vec3> {
    world_of_at(editor, point, 0.0)
}

fn world_of_at(editor: &EditorState, point: GeoPoint, altitude: f32) -> Option<Vec3> {
    let (x, z) = editor.map_state.geo_to_world(point)?;
    Some(Vec3::new(x, altitude, z))
}

fn project_geo(
    editor: &EditorState,
    camera: &Camera,
    transform: &GlobalTransform,
    point: GeoPoint,
) -> Option<egui::Pos2> {
    let world = world_of(editor, point)?;
    let screen = camera.world_to_viewport(transform, world).ok()?;
    Some(egui::pos2(screen.x, screen.y))
}

fn pick_object(editor: &EditorState, pointer: egui::Pos2) -> Option<ObjectKey> {
    pick_object_inner(editor, pointer, false)
}

/// Picks including locked objects. The context menu uses this so a locked
/// route can still be unlocked from the map.
fn pick_object_ignoring_lock(editor: &EditorState, pointer: egui::Pos2) -> Option<ObjectKey> {
    pick_object_inner(editor, pointer, true)
}

fn pick_object_inner(
    editor: &EditorState,
    pointer: egui::Pos2,
    include_locked: bool,
) -> Option<ObjectKey> {
    let mut best: Option<(f32, ObjectKey)> = None;
    for (key, kind, points, _) in &editor.projected {
        // Locked routes stay visible but ignore map clicks.
        if !include_locked && editor.is_locked(*key) {
            continue;
        }
        let score = match kind {
            ObjectType::Marker | ObjectType::Text => {
                let distance = points
                    .first()
                    .map(|point| distance(point, pointer))
                    .unwrap_or(f32::MAX);
                (distance <= 18.0).then_some(distance * 0.2)
            }
            // A particle effect rises above its anchor, so give it a wider
            // click target than a flat label marker.
            ObjectType::Particle => {
                let distance = points
                    .first()
                    .map(|point| distance(point, pointer))
                    .unwrap_or(f32::MAX);
                (distance <= 40.0).then_some(distance * 0.15)
            }
            ObjectType::Line => polyline_distance(points, pointer)
                .filter(|distance| *distance <= 10.0)
                .map(|distance| distance + 8.0),
            ObjectType::Building => {
                if point_in_polygon(points, pointer) {
                    Some(7.0)
                } else {
                    polyline_distance(points, pointer)
                        .filter(|distance| *distance <= 10.0)
                        .map(|distance| distance + 9.0)
                }
            }
        };
        if let Some(score) = score
            && best.is_none_or(|(current, _)| score < current)
        {
            best = Some((score, *key));
        }
    }
    best.map(|(_, key)| key)
}

fn polyline_distance(points: &[egui::Pos2], pointer: egui::Pos2) -> Option<f32> {
    if points.len() < 2 {
        return points.first().map(|point| distance(point, pointer));
    }
    points
        .windows(2)
        .map(|pair| segment_distance(pair[0], pair[1], pointer))
        .min_by(|left, right| left.total_cmp(right))
}

fn segment_distance(a: egui::Pos2, b: egui::Pos2, point: egui::Pos2) -> f32 {
    let ab = b - a;
    let length_squared = ab.length_sq();
    if length_squared <= f32::EPSILON {
        return distance(&a, point);
    }
    let t = ((point - a).dot(ab) / length_squared).clamp(0.0, 1.0);
    let closest = a + ab * t;
    distance(&closest, point)
}

fn distance(a: &egui::Pos2, b: egui::Pos2) -> f32 {
    (*a - b).length()
}

fn point_in_polygon(points: &[egui::Pos2], point: egui::Pos2) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let pi = points[i];
        let pj = points[j];
        if (pi.y > point.y) != (pj.y > point.y)
            && point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn gizmo_axis_at(
    editor: &EditorState,
    camera: &Camera,
    transform: &GlobalTransform,
    pointer: egui::Pos2,
) -> Option<EditorGizmoAxis> {
    let size = (editor.map_state.meters_per_pixel() as f32 * 90.0).max(1.0);
    // Play shows a full move gizmo (X/Y/Z) for a limited asset.
    let (anchor, allow_y, mode) = if let Some((_, point, altitude)) = editor.play_gizmo_target() {
        let (x, z) = editor.map_state.geo_to_world(point)?;
        (Vec3::new(x, altitude, z), true, EditorGizmoMode::Move)
    } else {
        if !editor.tool.changes_gizmo() {
            return None;
        }
        let (_, target) = editor.gizmo_target()?;
        let (x, z) = editor
            .map_state
            .geo_to_world(GeoPoint::new(target.lon, target.lat))?;
        let mode = match editor.tool {
            EditorTool::Rotate => EditorGizmoMode::Rotate,
            EditorTool::Scale => EditorGizmoMode::Scale,
            _ => EditorGizmoMode::Move,
        };
        let allow_y = editor.point_edit.is_none() || editor.point_edit_allows_altitude();
        (Vec3::new(x, target.altitude as f32, z), allow_y, mode)
    };
    let anchor_screen = camera
        .world_to_viewport(transform, anchor)
        .ok()
        .map(|point| egui::pos2(point.x, point.y))?;

    match mode {
        EditorGizmoMode::Rotate => ring_axis_at(camera, transform, anchor, size, pointer),
        EditorGizmoMode::Scale => {
            // The center ring/handle scales every axis at once.
            if anchor_screen.distance(pointer) <= 30.0 {
                return Some(EditorGizmoAxis::Uniform);
            }
            handle_axis_at(
                camera,
                transform,
                anchor,
                anchor_screen,
                size,
                pointer,
                true,
            )
        }
        EditorGizmoMode::Move => handle_axis_at(
            camera,
            transform,
            anchor,
            anchor_screen,
            size,
            pointer,
            allow_y,
        ),
    }
}

/// Picks the straight axis handle under the pointer (move and scale gizmos).
fn handle_axis_at(
    camera: &Camera,
    transform: &GlobalTransform,
    anchor: Vec3,
    anchor_screen: egui::Pos2,
    size: f32,
    pointer: egui::Pos2,
    allow_y: bool,
) -> Option<EditorGizmoAxis> {
    let axes = [
        (EditorGizmoAxis::X, Vec3::X),
        (EditorGizmoAxis::Y, Vec3::Y),
        (EditorGizmoAxis::Z, Vec3::Z),
    ];
    let mut best: Option<(f32, EditorGizmoAxis)> = None;
    for (axis, direction) in axes {
        if axis == EditorGizmoAxis::Y && !allow_y {
            continue;
        }
        let tip = anchor + direction * size;
        let Ok(tip_screen) = camera.world_to_viewport(transform, tip) else {
            continue;
        };
        let tip_screen = egui::pos2(tip_screen.x, tip_screen.y);
        let value = segment_distance(anchor_screen, tip_screen, pointer);
        if value <= 10.0 && best.is_none_or(|(current, _)| value < current) {
            best = Some((value, axis));
        }
    }
    best.map(|(_, axis)| axis)
}

/// Picks the rotation ring under the pointer. Each ring lives in the plane
/// perpendicular to its axis.
fn ring_axis_at(
    camera: &Camera,
    transform: &GlobalTransform,
    anchor: Vec3,
    size: f32,
    pointer: egui::Pos2,
) -> Option<EditorGizmoAxis> {
    let rings = [
        (EditorGizmoAxis::RotateX, Vec3::Y, Vec3::Z),
        (EditorGizmoAxis::RotateY, Vec3::X, Vec3::Z),
        (EditorGizmoAxis::RotateZ, Vec3::X, Vec3::Y),
    ];
    let mut best: Option<(f32, EditorGizmoAxis)> = None;
    for (axis, u, v) in rings {
        let mut previous: Option<egui::Pos2> = None;
        let mut distance_to_ring = f32::MAX;
        for step in 0..=64 {
            let angle = step as f32 / 64.0 * std::f32::consts::TAU;
            let point = anchor + (u * angle.cos() + v * angle.sin()) * size;
            let Ok(screen) = camera.world_to_viewport(transform, point) else {
                continue;
            };
            let screen = egui::pos2(screen.x, screen.y);
            if let Some(previous) = previous {
                distance_to_ring =
                    distance_to_ring.min(segment_distance(previous, screen, pointer));
            }
            previous = Some(screen);
        }
        if distance_to_ring <= 10.0 && best.is_none_or(|(current, _)| distance_to_ring < current) {
            best = Some((distance_to_ring, axis));
        }
    }
    best.map(|(_, axis)| axis)
}

/// Screen position of the gizmo origin (the primary transformable object).
fn gizmo_center_screen(
    editor: &EditorState,
    camera: &Camera,
    transform: &GlobalTransform,
) -> Option<egui::Pos2> {
    let (point, altitude) = if let Some((_, point, altitude)) = editor.play_gizmo_target() {
        (point, altitude)
    } else {
        let (_, target) = editor.gizmo_target()?;
        (
            GeoPoint::new(target.lon, target.lat),
            target.altitude as f32,
        )
    };
    let (x, z) = editor.map_state.geo_to_world(point)?;
    let anchor = Vec3::new(x, altitude, z);
    camera
        .world_to_viewport(transform, anchor)
        .ok()
        .map(|point| egui::pos2(point.x, point.y))
}

fn line_screen_points(editor: &EditorState, key: ObjectKey) -> Option<&[egui::Pos2]> {
    editor
        .projected
        .iter()
        .find(|(projected_key, _, _, _)| *projected_key == key)
        .map(|(_, _, points, _)| points.as_slice())
}

/// Index of the line handle under the pointer, if any.
fn line_handle_at(editor: &EditorState, pointer: egui::Pos2) -> Option<usize> {
    let edit = editor.point_edit?;
    let points = line_screen_points(editor, edit.key)?;
    points
        .iter()
        .enumerate()
        .map(|(index, position)| (index, position.distance(pointer)))
        .filter(|(_, distance)| *distance <= 10.0)
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(index, _)| index)
}

/// Segment under the pointer plus the fraction along it, used by double-click
/// insertion.
fn line_segment_at(editor: &EditorState, pointer: egui::Pos2) -> Option<(usize, f64)> {
    let edit = editor.point_edit?;
    let points = line_screen_points(editor, edit.key)?;
    let mut best: Option<(usize, f64, f32)> = None;
    for index in 0..points.len().saturating_sub(1) {
        let start = points[index];
        let end = points[index + 1];
        let segment = end - start;
        let length_sq = segment.length_sq();
        if length_sq <= 1.0 {
            continue;
        }
        let t = ((pointer - start).dot(segment) / length_sq).clamp(0.0, 1.0);
        let closest = start + segment * t;
        let distance = closest.distance(pointer);
        if distance <= 8.0 && best.is_none_or(|(_, _, best)| distance < best) {
            best = Some((index, t as f64, distance));
        }
    }
    best.map(|(index, t, _)| (index, t))
}

fn keyboard_shortcuts(
    editor: &mut EditorState,
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    text_focus: bool,
) {
    if !text_focus {
        // The delete confirmation modal owns the keyboard until it closes.
        if editor.confirm_delete {
            if keys.just_pressed(KeyCode::Escape) {
                editor.confirm_delete = false;
            }
            return;
        }
        let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        if control && keys.just_pressed(KeyCode::KeyZ) {
            editor.undo();
        }
        if (control && keys.just_pressed(KeyCode::KeyY))
            || (control && keys.pressed(KeyCode::ShiftLeft) && keys.just_pressed(KeyCode::KeyZ))
        {
            editor.redo();
        }
        if control && keys.just_pressed(KeyCode::KeyD) {
            editor.duplicate_selected();
        }
        if keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace) {
            editor.request_delete_selected();
        }
        if keys.just_pressed(KeyCode::Digit1) {
            editor.set_tool(EditorTool::Shape);
        }
        if keys.just_pressed(KeyCode::Digit2) {
            editor.set_tool(EditorTool::Line);
        }
        if keys.just_pressed(KeyCode::Digit3) {
            editor.set_tool(EditorTool::Text);
        }
        if keys.just_pressed(KeyCode::Digit4) {
            editor.set_tool(EditorTool::Measure);
        }
        if keys.just_pressed(KeyCode::Digit5) {
            editor.set_tool(EditorTool::Particle);
        }
        if keys.just_pressed(KeyCode::KeyQ) {
            editor.set_tool(EditorTool::Select);
        }
        if keys.just_pressed(KeyCode::KeyW) {
            editor.set_tool(EditorTool::Move);
        }
        if keys.just_pressed(KeyCode::KeyE) {
            editor.set_tool(EditorTool::Rotate);
        }
        if keys.just_pressed(KeyCode::KeyR) {
            editor.set_tool(EditorTool::Scale);
        }
        if keys.just_pressed(KeyCode::Enter) {
            finish_draft(editor);
        }
        if keys.just_pressed(KeyCode::Escape) {
            if editor.pick_route_marker.is_some() {
                editor.pick_route_marker = None;
            } else if editor.context_menu.is_some() {
                editor.context_menu = None;
            } else if editor.point_edit.is_some() {
                editor.end_point_edit();
            } else if editor.asset_drag.is_some() {
                editor.asset_drag = None;
            } else if editor.draft.is_active() {
                editor.draft.clear();
            } else if editor.measure.start.is_some() {
                editor.measure = Default::default();
            } else {
                editor.clear_selection();
            }
        }
        if keys.just_pressed(KeyCode::KeyG) {
            editor.snap_meters = if editor.snap_meters > 0.0 { 0.0 } else { 1.0 };
        }
    }
    if mouse.just_pressed(MouseButton::Right) && editor.draft.is_active() {
        editor.draft.clear();
    }
}

#[allow(clippy::too_many_arguments)]
fn mouse_input(
    editor: &mut EditorState,
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    pointer: egui::Pos2,
    camera: &Camera,
    transform: &GlobalTransform,
    now: f64,
) {
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if mouse.just_pressed(MouseButton::Left) {
        let Some(world) = editor.cursor_world else {
            return;
        };
        let geo = editor
            .map_state
            .world_to_geo(world.x, world.z)
            .map(|point| editor.snap_point(point));

        // Picking a route marker from the map takes precedence over every
        // other click action while the Inspector asks for it.
        if let Some(route_key) = editor.pick_route_marker {
            if let Some(key) = pick_object(editor, pointer) {
                let marker_id = editor.object(key).and_then(|object| {
                    matches!(&object.kind, EditorKind::Marker(_))
                        .then_some(object.server_id)
                        .flatten()
                });
                match marker_id {
                    Some(marker_id) => {
                        let animatable = editor
                            .object(key)
                            .and_then(|object| match &object.kind {
                                EditorKind::Marker(marker) => {
                                    Some(editor.marker_is_animatable(marker))
                                }
                                _ => None,
                            })
                            .unwrap_or(false);
                        editor.assign_route_marker(route_key, marker_id);
                        if !animatable {
                            editor.log(
                                "Peringatan: marker ini belum bisa bergerak. Atur mode pindah \
                                 asetnya menjadi Free/Limited atau pakai model animasi.",
                            );
                        }
                    }
                    None => editor.log(
                        "Itu bukan marker, atau marker baru belum tersimpan. Simpan dulu (Ctrl+S).",
                    ),
                }
            } else {
                editor.log("Klik tepat pada marker di peta.");
            }
            return;
        }

        if let Some(axis) = gizmo_axis_at(editor, camera, transform, pointer) {
            let center = gizmo_center_screen(editor, camera, transform);
            begin_gizmo_drag(editor, axis, world, pointer, center);
            return;
        }

        // Per-point line editing: click a handle to select a vertex, or
        // double-click a segment to insert a new vertex there.
        if let Some(edit) = editor.point_edit {
            if let Some(index) = line_handle_at(editor, pointer) {
                editor.set_point_edit_index(index);
                return;
            }
            if let Some((segment, t)) = line_segment_at(editor, pointer) {
                let double_click = editor.last_click.is_some_and(|(last, position)| {
                    now - last < 0.35 && position.distance(pointer) < 6.0
                });
                if double_click {
                    editor.insert_edit_point(edit.key, segment, t);
                } else {
                    editor.last_click = Some((now, pointer));
                }
                return;
            }
            // Keep edit mode active on other viewport clicks; exit with the
            // Inspector button or Escape so a missed handle does not drop out.
            editor.last_click = None;
            return;
        }

        match editor.tool {
            EditorTool::Shape => {
                if let Some(geo) = geo {
                    if editor.active_shape == EditorShapeKind::Custom {
                        if !matches!(editor.draft, Draft::Polygon(_)) {
                            editor.draft = Draft::Polygon(Vec::new());
                        }
                        let double_click = editor.last_click.is_some_and(|(last, position)| {
                            now - last < 0.35 && position.distance(pointer) < 6.0
                        });
                        if double_click {
                            // Do not push the second click of a double-click: it
                            // would add a duplicate vertex and taper the shape.
                            finish_draft(editor);
                        } else {
                            editor.draft.push(geo);
                        }
                        editor.last_click = Some((now, pointer));
                    } else {
                        editor.add_shape(editor.active_shape, geo);
                        // One click creates one shape; go back to Select so the
                        // next click does not place another shape by accident.
                        editor.set_tool(EditorTool::Select);
                    }
                }
            }
            EditorTool::Text => {
                if let Some(geo) = geo {
                    editor.add_text(geo);
                    editor.set_tool(EditorTool::Select);
                }
            }
            EditorTool::Particle => {
                if let Some(geo) = geo {
                    editor.add_particle(geo);
                    // One click places one particle; go back to Select so the
                    // next click does not add another one by accident.
                    editor.set_tool(EditorTool::Select);
                }
            }
            EditorTool::Line => {
                if !matches!(editor.draft, Draft::Line(_) | Draft::Route(_)) {
                    editor.draft = Draft::Line(Vec::new());
                }
                let double_click = editor.last_click.is_some_and(|(last, position)| {
                    now - last < 0.35 && position.distance(pointer) < 6.0
                });
                if double_click {
                    // The first click already added the final vertex.
                    finish_draft(editor);
                } else if let Some(geo) = geo {
                    editor.draft.push(geo);
                }
                editor.last_click = Some((now, pointer));
            }
            EditorTool::Measure => {
                if let Some(geo) = geo {
                    if editor.measure.start.is_none() || editor.measure.end.is_some() {
                        editor.measure = super::model::MeasureState {
                            start: Some(geo),
                            end: None,
                        };
                    } else {
                        editor.measure.end = Some(geo);
                    }
                }
            }
            EditorTool::Select | EditorTool::Move | EditorTool::Rotate | EditorTool::Scale => {
                let picked = pick_object(editor, pointer);
                if let Some(key) = picked {
                    if control {
                        editor.toggle_selected(key);
                    } else if !editor.selected.contains(&key) {
                        editor.select_only(key);
                    }
                    let selection_transformable = editor.primary_transform().is_some();
                    if selection_transformable
                        && matches!(editor.tool, EditorTool::Select | EditorTool::Move)
                    {
                        let originals = selected_points(editor);
                        if let Some(start) = geo
                            && !originals.is_empty()
                        {
                            editor.push_undo();
                            editor.drag = DragState::MoveObjects { start, originals };
                        }
                    }
                } else {
                    if !control {
                        editor.clear_selection();
                    }
                    editor.drag = DragState::Marquee {
                        start: pointer,
                        current: pointer,
                    };
                }
            }
        }
    }

    if mouse.pressed(MouseButton::Left) {
        update_drag(editor, pointer, camera, transform);
    }

    if mouse.just_released(MouseButton::Left) {
        if let DragState::Marquee { start, current } = editor.drag {
            let rect = egui::Rect::from_two_pos(start, current);
            if rect.width() > 4.0 || rect.height() > 4.0 {
                editor.selected.clear();
                for (key, _, points, _) in &editor.projected {
                    if editor.is_locked(*key) {
                        continue;
                    }
                    if points
                        .first()
                        .is_some_and(|position| rect.contains(*position))
                    {
                        editor.selected.insert(*key);
                    }
                }
                editor.touch();
            }
        }
        editor.drag = DragState::None;
    }
}

fn selected_points(editor: &EditorState) -> Vec<(ObjectKey, GeoPoint)> {
    editor
        .selected
        .iter()
        .filter_map(|key| {
            let object = editor.object(*key)?;
            Some((*key, object.point()?))
        })
        .collect()
}

fn begin_gizmo_drag(
    editor: &mut EditorState,
    axis: EditorGizmoAxis,
    world: Vec3,
    pointer: egui::Pos2,
    center_screen: Option<egui::Pos2>,
) {
    // Point editing uses the same gizmo; only the selected vertex moves.
    if let Some(edit) = editor.point_edit
        && let Some(index) = edit.point
    {
        let original = editor
            .object(edit.key)
            .and_then(|object| match &object.kind {
                EditorKind::Line(line) => {
                    let point = line.points.get(index)?;
                    Some([point[0], point[1], line.altitude(index)])
                }
                EditorKind::Building(building) if building.shape == EditorShapeKind::Custom => {
                    let point = building.points.get(index)?;
                    Some([point[0], point[1], building.altitude])
                }
                _ => None,
            });
        if let Some(original) = original
            && matches!(
                axis,
                EditorGizmoAxis::X | EditorGizmoAxis::Y | EditorGizmoAxis::Z
            )
        {
            editor.push_undo();
            editor.drag = DragState::PointDrag {
                axis,
                key: edit.key,
                index,
                original,
                start_world: world,
                start_screen: pointer,
            };
        }
        return;
    }

    let originals = editor
        .selected
        .iter()
        .filter_map(|key| {
            let object = editor.object(*key)?;
            let transform = object.kind.transform()?;
            Some((
                *key,
                (
                    [transform.lon, transform.lat, transform.altitude],
                    transform.rot,
                    transform.scale,
                ),
            ))
        })
        .collect::<Vec<_>>();
    if originals.is_empty() {
        return;
    }
    match axis {
        EditorGizmoAxis::RotateX | EditorGizmoAxis::RotateY | EditorGizmoAxis::RotateZ => {
            let Some(center) = center_screen else {
                return;
            };
            let start_angle = (pointer.y - center.y).atan2(pointer.x - center.x);
            editor.push_undo();
            editor.drag = DragState::Rotate {
                axis,
                originals: originals
                    .iter()
                    .map(|(key, (_, rotation, _))| (*key, *rotation))
                    .collect(),
                center_screen: center,
                start_angle,
            };
        }
        EditorGizmoAxis::Uniform => {
            editor.push_undo();
            editor.drag = DragState::Scale {
                axis,
                originals: originals
                    .iter()
                    .map(|(key, (_, _, scale))| (*key, *scale))
                    .collect(),
                start_world: world,
                start_screen: pointer,
            };
        }
        _ if editor.tool == EditorTool::Scale => {
            editor.push_undo();
            editor.drag = DragState::Scale {
                axis,
                originals: originals
                    .iter()
                    .map(|(key, (_, _, scale))| (*key, *scale))
                    .collect(),
                start_world: world,
                start_screen: pointer,
            };
        }
        _ => {
            editor.push_undo();
            editor.drag = DragState::Gizmo {
                axis,
                original: originals
                    .iter()
                    .map(|(key, (position, _, _))| (*key, *position))
                    .collect(),
                start_world: world,
                start_screen: pointer,
            };
        }
    }
}

fn update_drag(
    editor: &mut EditorState,
    pointer: egui::Pos2,
    camera: &Camera,
    transform: &GlobalTransform,
) {
    let drag = editor.drag.clone();
    match drag {
        DragState::MoveObjects { start, originals } => {
            let Some(world) = ground_point(camera, transform, pointer, editor) else {
                return;
            };
            let Some(current) = editor.map_state.world_to_geo(world.x, world.z) else {
                return;
            };
            let Ok(offset) = current.local_offset(start) else {
                return;
            };
            for (key, original) in originals {
                let Some(object) = editor.object_mut(key) else {
                    continue;
                };
                let Ok(target) =
                    GeoPoint::from_local_offset(original, offset.east_meters, offset.north_meters)
                else {
                    continue;
                };
                object.kind.set_position(target);
                object.dirty = true;
            }
            editor.dirty = true;
            editor.touch();
        }
        DragState::Gizmo {
            axis,
            original,
            start_world,
            start_screen,
        } => {
            let direction = axis_vector(axis);
            let delta = if axis == EditorGizmoAxis::Y {
                (start_screen.y - pointer.y) * editor.map_state.meters_per_pixel() as f32
            } else {
                let Some(world) = ground_point(camera, transform, pointer, editor) else {
                    return;
                };
                (world - start_world).dot(direction)
            };
            // Convert the ground displacement from the drag origin into a geo
            // offset once, then place every object from its original anchor.
            // Applying `offset_object` here would add the cumulative delta on
            // top of the current position on every frame and make objects
            // accelerate away from the pointer.
            let offset = if axis == EditorGizmoAxis::Y {
                None
            } else {
                let target_world = start_world + direction * delta;
                let Some(start_geo) = editor.map_state.world_to_geo(start_world.x, start_world.z)
                else {
                    return;
                };
                let Some(target_geo) = editor
                    .map_state
                    .world_to_geo(target_world.x, target_world.z)
                else {
                    return;
                };
                match target_geo.local_offset(start_geo) {
                    Ok(offset) => Some(offset),
                    Err(_) => return,
                }
            };
            for (key, data) in original {
                let Some(object) = editor.object_mut(key) else {
                    continue;
                };
                match axis {
                    EditorGizmoAxis::X | EditorGizmoAxis::Z => {
                        if let Some(offset) = offset
                            && let Ok(target) = GeoPoint::from_local_offset(
                                GeoPoint::new(data[0], data[1]),
                                offset.east_meters,
                                offset.north_meters,
                            )
                        {
                            object.kind.set_position(target);
                        }
                    }
                    EditorGizmoAxis::Y => {
                        object.kind.set_altitude((data[2] + delta as f64).max(0.0));
                    }
                    EditorGizmoAxis::RotateX
                    | EditorGizmoAxis::RotateY
                    | EditorGizmoAxis::RotateZ
                    | EditorGizmoAxis::Uniform => {}
                }
                object.dirty = true;
            }
            editor.dirty = true;
            editor.touch();
        }
        DragState::PlayGizmo {
            axis,
            key,
            original,
            original_altitude,
            start_world,
            start_screen,
        } => {
            if axis == EditorGizmoAxis::Y {
                let delta =
                    (start_screen.y - pointer.y) * editor.map_state.meters_per_pixel() as f32;
                if let Some(unit) = editor.play.limited.get_mut(&key) {
                    unit.altitude = (original_altitude + delta).max(0.0);
                }
                editor.play_sync_pending = true;
                // No `touch()`: the marker root and the gizmo both follow the
                // runtime altitude through `EditorPlayFrame`.
                return;
            }
            let Some(world) = ground_point(camera, transform, pointer, editor) else {
                return;
            };
            let Some(start_geo) = editor.map_state.world_to_geo(start_world.x, start_world.z)
            else {
                return;
            };
            let Some(target_geo) = editor.map_state.world_to_geo(world.x, world.z) else {
                return;
            };
            let Ok(offset) = target_geo.local_offset(start_geo) else {
                return;
            };
            let Ok(target) =
                GeoPoint::from_local_offset(original, offset.east_meters, offset.north_meters)
            else {
                return;
            };
            if let Some(unit) = editor.play.limited.get_mut(&key) {
                unit.position = target;
            }
            if let Some(obstacle) = editor
                .play
                .obstacles
                .iter_mut()
                .find(|obstacle| obstacle.key == Some(key))
            {
                obstacle.center = target;
            }
            editor.play_sync_pending = true;
            // No `touch()`: the marker root and the gizmo both follow the
            // runtime position through `EditorPlayFrame`.
        }
        DragState::PointDrag {
            axis,
            key,
            index,
            original,
            start_world,
            start_screen,
        } => {
            let direction = axis_vector(axis);
            let delta = if axis == EditorGizmoAxis::Y {
                (start_screen.y - pointer.y) * editor.map_state.meters_per_pixel() as f32
            } else {
                let Some(world) = ground_point(camera, transform, pointer, editor) else {
                    return;
                };
                (world - start_world).dot(direction)
            };
            let target = if axis == EditorGizmoAxis::Y {
                None
            } else {
                let target_world = start_world + direction * delta;
                let Some(start_geo) = editor.map_state.world_to_geo(start_world.x, start_world.z)
                else {
                    return;
                };
                let Some(target_geo) = editor
                    .map_state
                    .world_to_geo(target_world.x, target_world.z)
                else {
                    return;
                };
                let Ok(offset) = target_geo.local_offset(start_geo) else {
                    return;
                };
                GeoPoint::from_local_offset(
                    GeoPoint::new(original[0], original[1]),
                    offset.east_meters,
                    offset.north_meters,
                )
                .ok()
            };
            let Some(object) = editor.object_mut(key) else {
                return;
            };
            match &mut object.kind {
                EditorKind::Line(line) => match axis {
                    EditorGizmoAxis::Y => {
                        let Some(altitude) = line.altitudes.get_mut(index) else {
                            return;
                        };
                        *altitude = (original[2] + delta as f64).max(0.0);
                    }
                    EditorGizmoAxis::X | EditorGizmoAxis::Z => {
                        let (Some(point), Some(target)) = (line.points.get_mut(index), target)
                        else {
                            return;
                        };
                        *point = [target.longitude, target.latitude];
                    }
                    _ => return,
                },
                EditorKind::Building(building) if building.shape == EditorShapeKind::Custom => {
                    if !matches!(axis, EditorGizmoAxis::X | EditorGizmoAxis::Z) {
                        return;
                    }
                    let (Some(point), Some(target)) = (building.points.get_mut(index), target)
                    else {
                        return;
                    };
                    *point = [target.longitude, target.latitude];
                }
                _ => return,
            }
            object.dirty = true;
            editor.dirty = true;
            editor.touch();
        }
        DragState::Rotate {
            axis,
            originals,
            center_screen,
            start_angle,
        } => {
            let angle = (pointer.y - center_screen.y).atan2(pointer.x - center_screen.x);
            // Screen angles grow clockwise; the stored rotations follow the
            // opposite convention.
            let mut delta = -(angle - start_angle).to_degrees() as f64;
            if delta > 180.0 {
                delta -= 360.0;
            } else if delta < -180.0 {
                delta += 360.0;
            }
            for (key, original) in originals {
                let Some(object) = editor.object_mut(key) else {
                    continue;
                };
                let axis_index = match axis {
                    EditorGizmoAxis::RotateX => 0,
                    EditorGizmoAxis::RotateY => 1,
                    EditorGizmoAxis::RotateZ => 2,
                    _ => continue,
                };
                let value = (original[axis_index] + delta).rem_euclid(360.0);
                object.kind.set_rotation_axis(axis_index, value);
                object.dirty = true;
            }
            editor.dirty = true;
            editor.touch();
        }
        DragState::Scale {
            axis,
            originals,
            start_world,
            start_screen,
        } => {
            let factor = if axis == EditorGizmoAxis::Uniform {
                // Uniform scaling follows the pointer drag from the click point
                // (dragging right/up grows, left/down shrinks).
                let delta = (pointer.x - start_screen.x) + (start_screen.y - pointer.y);
                (1.0 + delta / 140.0).clamp(0.1, 10.0) as f64
            } else {
                let direction = axis_vector(axis);
                let delta = if axis == EditorGizmoAxis::Y {
                    (start_screen.y - pointer.y) * editor.map_state.meters_per_pixel() as f32
                } else {
                    let Some(world) = editor.cursor_world else {
                        return;
                    };
                    (world - start_world).dot(direction)
                };
                (1.0_f32 + delta / 60.0).clamp(0.1, 10.0) as f64
            };
            for (key, original) in originals {
                let Some(object) = editor.object_mut(key) else {
                    continue;
                };
                let mut scale = original;
                match axis {
                    EditorGizmoAxis::X => scale[0] = (original[0] * factor).max(MIN_MARKER_SCALE),
                    EditorGizmoAxis::Y => scale[1] = (original[1] * factor).max(MIN_MARKER_SCALE),
                    EditorGizmoAxis::Z => scale[2] = (original[2] * factor).max(MIN_MARKER_SCALE),
                    EditorGizmoAxis::Uniform => {
                        scale = [
                            (original[0] * factor).max(MIN_MARKER_SCALE),
                            (original[1] * factor).max(MIN_MARKER_SCALE),
                            (original[2] * factor).max(MIN_MARKER_SCALE),
                        ];
                    }
                    _ => continue,
                }
                object.kind.set_scale(scale);
                object.dirty = true;
            }
            editor.dirty = true;
            editor.touch();
        }
        DragState::Marquee { start, .. } => {
            editor.drag = DragState::Marquee {
                start,
                current: pointer,
            };
        }
        DragState::None => {}
    }
}

fn axis_vector(axis: EditorGizmoAxis) -> Vec3 {
    match axis {
        EditorGizmoAxis::X => Vec3::X,
        EditorGizmoAxis::Y => Vec3::Y,
        EditorGizmoAxis::Z => Vec3::Z,
        EditorGizmoAxis::RotateX | EditorGizmoAxis::RotateY | EditorGizmoAxis::RotateZ => Vec3::Y,
        EditorGizmoAxis::Uniform => Vec3::ONE,
    }
}

fn finish_draft(editor: &mut EditorState) {
    let draft = std::mem::take(&mut editor.draft);
    match draft {
        Draft::Line(points) if points.len() >= 2 => {
            editor.add_line(points);
            editor.set_tool(EditorTool::Select);
        }
        Draft::Route(points) if points.len() >= 2 => {
            editor.add_route(points);
            editor.set_tool(EditorTool::Select);
        }
        Draft::Polygon(points) if points.len() >= 3 => {
            editor.add_polygon(points);
            editor.set_tool(EditorTool::Select);
        }
        _ => {}
    }
}

/// Applies a drag delta to markers in a single selection. Used by the
/// inspector's numeric fields to keep undo history compact.
pub fn nudge_selected(editor: &mut EditorState, east: f64, north: f64) {
    editor.push_undo();
    let selected = editor.selected.clone();
    for key in selected {
        if let Some(object) = editor.object_mut(key) {
            offset_object(&mut object.kind, east, north);
            object.dirty = true;
        }
    }
    editor.dirty = true;
    editor.touch();
}

/// Returns the distance in meters between two points, used by the measure
/// tool and the status bar.
pub fn distance_meters(a: GeoPoint, b: GeoPoint) -> f64 {
    let Ok(offset) = b.local_offset(a) else {
        return 0.0;
    };
    (offset.east_meters * offset.east_meters + offset.north_meters * offset.north_meters).sqrt()
}

pub fn select_all(editor: &mut EditorState) {
    editor.selected = editor.objects.iter().map(|object| object.key).collect();
    editor.touch();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finishing_a_line_returns_to_select() {
        let mut editor = EditorState::new();
        editor.loaded = true;
        editor.set_tool(EditorTool::Line);
        editor.draft = Draft::Line(vec![
            GeoPoint::new(115.225, -8.648),
            GeoPoint::new(115.226, -8.649),
        ]);

        finish_draft(&mut editor);

        assert_eq!(editor.tool, EditorTool::Select);
        assert_eq!(editor.objects.len(), 1);
    }

    #[test]
    fn locked_route_ignores_map_picking() {
        let mut editor = EditorState::new();
        editor.loaded = true;
        let base = GeoPoint::new(115.225, -8.648);
        let end = GeoPoint::from_local_offset(base, 10.0, 0.0).expect("east");
        editor.add_route(vec![base, end]);
        let key = *editor.selected.iter().next().expect("route selected");
        editor.projected.push((
            key,
            ObjectType::Line,
            vec![egui::pos2(100.0, 100.0), egui::pos2(200.0, 100.0)],
            true,
        ));
        let pointer = egui::pos2(150.0, 100.0);
        assert_eq!(pick_object(&editor, pointer), Some(key));

        editor.locked.insert(key);
        assert_eq!(pick_object(&editor, pointer), None);
        // The context menu still finds the locked route so it can be unlocked.
        assert_eq!(pick_object_ignoring_lock(&editor, pointer), Some(key));
    }

    #[test]
    fn finishing_a_custom_polygon_returns_to_select() {
        let mut editor = EditorState::new();
        editor.loaded = true;
        editor.set_tool(EditorTool::Shape);
        editor.draft = Draft::Polygon(vec![
            GeoPoint::new(115.225, -8.648),
            GeoPoint::new(115.226, -8.648),
            GeoPoint::new(115.226, -8.649),
        ]);

        finish_draft(&mut editor);

        assert_eq!(editor.tool, EditorTool::Select);
        assert_eq!(editor.objects.len(), 1);
    }

    #[test]
    fn dragging_a_custom_mesh_point_starts_a_point_drag() {
        let mut editor = EditorState::new();
        editor.loaded = true;
        let base = GeoPoint::new(115.225, -8.648);
        editor.add_polygon(vec![
            base,
            GeoPoint::from_local_offset(base, 5.0, 0.0).expect("east"),
            GeoPoint::from_local_offset(base, 5.0, 5.0).expect("corner"),
            GeoPoint::from_local_offset(base, 0.0, 5.0).expect("north"),
        ]);
        let key = *editor.selected.iter().next().expect("shape selected");
        editor.begin_point_edit(key);
        editor.set_point_edit_index(0);

        begin_gizmo_drag(
            &mut editor,
            EditorGizmoAxis::X,
            Vec3::ZERO,
            egui::pos2(10.0, 10.0),
            None,
        );

        assert!(matches!(
            editor.drag,
            DragState::PointDrag {
                axis: EditorGizmoAxis::X,
                index: 0,
                ..
            }
        ));
    }
}
