use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::egui;
use twg_map::{CameraFlight, GeoPoint};
use twg_render::{
    BaseLayerResource, CameraFlightState, EditorGizmoAxis, EditorMode, EditorScene, MapCamera,
    MapCameraResource, MapCursorResource, UiPointerState,
};

use crate::state::ClientUi;

use super::model::{
    Draft, DragState, EditorState, EditorTool, ObjectKey, ObjectType, offset_object,
};

/// Set by the egui pass so editor shortcuts do not fire while a text field has
/// focus.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct EditorInputFocus {
    pub keyboard: bool,
}

pub fn sync_editor_mode(client: Res<ClientUi>, mut mode: ResMut<EditorMode>) {
    let active = client.state.screen == twg_client::session::Screen::MapEditor;
    mode.active = active;
    mode.preview = active && client.editor.preview;
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
) {
    let in_editor = client.state.screen == twg_client::session::Screen::MapEditor;
    if !in_editor {
        if mode.active {
            mode.active = false;
        }
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

    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !focus.keyboard && control && keys.just_pressed(KeyCode::KeyS) {
        client.editor_save();
    }

    let (camera, camera_transform) = *camera_query;
    let editor = &mut client.editor;
    editor.map_state = map_camera.0;
    editor.cursor_geo = cursor.0.position;
    editor.fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or_default();

    apply_requests(editor, &mut map_camera, &mut flight, &mut layers);

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
    let inside_viewport =
        pointer_pos.is_some_and(|position| viewport.contains(position)) && !pointer.over_ui;
    let editing = !editor.preview && editor.loaded;

    editor.cursor_world = pointer_pos
        .filter(|_| inside_viewport)
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
    if inside_viewport && editing {
        editor.hover_axis = gizmo_axis_at(editor, camera, camera_transform, pointer_pos.unwrap());
        editor.hover_key = if editor.hover_axis.is_none() {
            pick_object(editor, pointer_pos.unwrap())
        } else {
            None
        };
    } else {
        editor.hover_axis = None;
        editor.hover_key = None;
    }

    if editing {
        if mouse.just_pressed(MouseButton::Left) {
            editor.context_menu = None;
        }
        if inside_viewport && mouse.just_pressed(MouseButton::Right) {
            editor.right_press = pointer_pos;
        }
        if inside_viewport
            && mouse.just_released(MouseButton::Right)
            && let (Some(start), Some(position)) = (editor.right_press.take(), pointer_pos)
            && start.distance(position) < 5.0
            && let Some(key) = pick_object(editor, position)
        {
            editor.select_only(key);
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
        editor.drag = DragState::None;
        editor.context_menu = None;
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
    for object in &editor.objects {
        if !object.visible || !editor.visible.allows(object.object_type()) {
            continue;
        }
        let points = object.kind.points();
        let mut screen_points = Vec::with_capacity(points.len());
        for point in &points {
            let Some(world) = world_of(editor, *point) else {
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
        if editor.labels_visible {
            let label = match &object.kind {
                super::model::EditorKind::Text(text) => Some(text.text.clone()),
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
}

fn world_of(editor: &EditorState, point: GeoPoint) -> Option<Vec3> {
    let (x, z) = editor.map_state.geo_to_world(point)?;
    Some(Vec3::new(x, 0.0, z))
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
    let mut best: Option<(f32, ObjectKey)> = None;
    for (key, kind, points, _) in &editor.projected {
        let score = match kind {
            ObjectType::Marker | ObjectType::Text | ObjectType::Particle => {
                let distance = points
                    .first()
                    .map(|point| distance(point, pointer))
                    .unwrap_or(f32::MAX);
                (distance <= 18.0).then_some(distance * 0.2)
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
    if !editor.tool.changes_gizmo() {
        return None;
    }
    let marker = editor.primary_marker()?;
    let size = (editor.map_state.meters_per_pixel() as f32 * 90.0).max(1.0);
    let (x, z) = editor.map_state.geo_to_world(marker.point())?;
    let anchor = Vec3::new(x, marker.altitude as f32, z);
    let anchor_screen = camera
        .world_to_viewport(transform, anchor)
        .ok()
        .map(|point| egui::pos2(point.x, point.y))?;

    if editor.tool == EditorTool::Rotate {
        let mut best = f32::MAX;
        let mut previous: Option<egui::Pos2> = None;
        for step in 0..=48 {
            let angle = step as f32 / 48.0 * std::f32::consts::TAU;
            let point = anchor + Vec3::new(angle.cos() * size, 0.0, angle.sin() * size);
            let Ok(screen) = camera.world_to_viewport(transform, point) else {
                continue;
            };
            let screen = egui::pos2(screen.x, screen.y);
            if let Some(previous) = previous {
                best = best.min(segment_distance(previous, screen, pointer));
            }
            previous = Some(screen);
        }
        if best <= 10.0 {
            return Some(EditorGizmoAxis::RotateY);
        }
    }

    let axes = [
        (EditorGizmoAxis::X, Vec3::X),
        (EditorGizmoAxis::Y, Vec3::Y),
        (EditorGizmoAxis::Z, Vec3::Z),
    ];
    let mut best: Option<(f32, EditorGizmoAxis)> = None;
    for (axis, direction) in axes {
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

fn keyboard_shortcuts(
    editor: &mut EditorState,
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    text_focus: bool,
) {
    if !text_focus {
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
            editor.delete_selected();
        }
        if keys.just_pressed(KeyCode::Digit1) {
            editor.tool = EditorTool::Marker;
        }
        if keys.just_pressed(KeyCode::Digit2) {
            editor.tool = EditorTool::Line;
        }
        if keys.just_pressed(KeyCode::Digit3) {
            editor.tool = EditorTool::Polygon;
        }
        if keys.just_pressed(KeyCode::Digit4) {
            editor.tool = EditorTool::Text;
        }
        if keys.just_pressed(KeyCode::Digit5) {
            editor.tool = EditorTool::Measure;
        }
        if keys.just_pressed(KeyCode::KeyQ) {
            editor.tool = EditorTool::Select;
        }
        if keys.just_pressed(KeyCode::KeyW) {
            editor.tool = EditorTool::Move;
        }
        if keys.just_pressed(KeyCode::KeyE) {
            editor.tool = EditorTool::Rotate;
        }
        if keys.just_pressed(KeyCode::KeyR) {
            editor.tool = EditorTool::Scale;
        }
        if keys.just_pressed(KeyCode::Enter) {
            finish_draft(editor);
        }
        if keys.just_pressed(KeyCode::Escape) {
            if editor.draft.is_active() {
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

        if let Some(axis) = gizmo_axis_at(editor, camera, transform, pointer) {
            begin_gizmo_drag(editor, axis, world, pointer);
            return;
        }

        match editor.tool {
            EditorTool::Marker => {
                if let Some(geo) = geo {
                    editor.add_marker(geo, editor.active_unit);
                }
            }
            EditorTool::Text => {
                if let Some(geo) = geo {
                    editor.add_text(geo);
                }
            }
            EditorTool::Line | EditorTool::Polygon => {
                let polygon = editor.tool == EditorTool::Polygon;
                let matches_kind = matches!(
                    (&editor.draft, polygon),
                    (Draft::Line(_), false) | (Draft::Polygon(_), true)
                );
                if !matches_kind {
                    editor.draft = if polygon {
                        Draft::Polygon(Vec::new())
                    } else {
                        Draft::Line(Vec::new())
                    };
                }
                if let Some(geo) = geo {
                    editor.draft.push(geo);
                }
                let double_click = editor.last_click.is_some_and(|(last, position)| {
                    now - last < 0.35 && position.distance(pointer) < 6.0
                });
                if double_click {
                    finish_draft(editor);
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
                    let selection_has_marker = editor.primary_marker().is_some();
                    if selection_has_marker
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
) {
    let originals = editor
        .selected
        .iter()
        .filter_map(|key| {
            let object = editor.object(*key)?;
            match &object.kind {
                super::model::EditorKind::Marker(marker) => Some((
                    *key,
                    (marker.lon, marker.lat, marker.altitude, marker.rot_y),
                )),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    if originals.is_empty() {
        return;
    }
    editor.push_undo();
    if axis == EditorGizmoAxis::RotateY {
        let Some(marker) = editor.primary_marker() else {
            return;
        };
        let (x, z) = editor
            .map_state
            .geo_to_world(marker.point())
            .unwrap_or((0.0, 0.0));
        let start_angle = (world.z - z).atan2(world.x - x);
        editor.drag = DragState::Rotate {
            originals: originals.iter().map(|(key, data)| (*key, data.3)).collect(),
            start_angle,
        };
    } else if editor.tool == EditorTool::Scale {
        editor.drag = DragState::Scale {
            axis,
            originals: originals
                .iter()
                .map(|(key, data)| (*key, [data.0, data.1, data.2]))
                .collect(),
            start_world: world,
            start_screen: pointer,
        };
    } else {
        editor.drag = DragState::Gizmo {
            axis,
            original: originals
                .iter()
                .map(|(key, data)| (*key, [data.0, data.1, data.2]))
                .collect(),
            start_world: world,
            start_screen: pointer,
        };
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
            for (key, _) in originals {
                let Some(object) = editor.object_mut(key) else {
                    continue;
                };
                offset_object(&mut object.kind, offset.east_meters, offset.north_meters);
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
            for (key, data) in original {
                let Some(object) = editor.object_mut(key) else {
                    continue;
                };
                let super::model::EditorKind::Marker(marker) = &mut object.kind else {
                    continue;
                };
                match axis {
                    EditorGizmoAxis::X => {
                        if let Ok(point) = GeoPoint::from_local_offset(
                            GeoPoint::new(data[0], data[1]),
                            delta as f64,
                            0.0,
                        ) {
                            marker.set_point(point);
                        }
                    }
                    EditorGizmoAxis::Y => marker.altitude = (data[2] + delta as f64).max(0.0),
                    EditorGizmoAxis::Z => {
                        if let Ok(point) = GeoPoint::from_local_offset(
                            GeoPoint::new(data[0], data[1]),
                            0.0,
                            -(delta as f64),
                        ) {
                            marker.set_point(point);
                        }
                    }
                    EditorGizmoAxis::RotateY => {}
                }
                object.dirty = true;
            }
            editor.dirty = true;
            editor.touch();
        }
        DragState::Rotate {
            originals,
            start_angle,
        } => {
            let Some(world) = editor.cursor_world else {
                return;
            };
            let Some(marker) = editor.primary_marker() else {
                return;
            };
            let (x, z) = editor
                .map_state
                .geo_to_world(marker.point())
                .unwrap_or((0.0, 0.0));
            let angle = (world.z - z).atan2(world.x - x);
            let delta = (angle - start_angle).to_degrees() as f64;
            for (key, original) in originals {
                let Some(object) = editor.object_mut(key) else {
                    continue;
                };
                if let super::model::EditorKind::Marker(marker) = &mut object.kind {
                    marker.rot_y = (original + delta).rem_euclid(360.0);
                    object.dirty = true;
                }
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
            let direction = axis_vector(axis);
            let delta = if axis == EditorGizmoAxis::Y {
                (start_screen.y - pointer.y) * editor.map_state.meters_per_pixel() as f32
            } else {
                let Some(world) = editor.cursor_world else {
                    return;
                };
                (world - start_world).dot(direction)
            };
            let factor = (1.0_f32 + delta / 60.0).clamp(0.1, 10.0) as f64;
            for (key, original) in originals {
                let Some(object) = editor.object_mut(key) else {
                    continue;
                };
                if let super::model::EditorKind::Marker(marker) = &mut object.kind {
                    match axis {
                        EditorGizmoAxis::X => marker.scale_x = (original[0] * factor).max(0.05),
                        EditorGizmoAxis::Y => marker.scale_y = (original[1] * factor).max(0.05),
                        EditorGizmoAxis::Z => marker.scale_z = (original[2] * factor).max(0.05),
                        EditorGizmoAxis::RotateY => {}
                    }
                    object.dirty = true;
                }
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
        EditorGizmoAxis::RotateY => Vec3::Y,
    }
}

fn finish_draft(editor: &mut EditorState) {
    let draft = std::mem::take(&mut editor.draft);
    match draft {
        Draft::Line(points) if points.len() >= 2 => editor.add_line(points),
        Draft::Polygon(points) if points.len() >= 3 => editor.add_polygon(points),
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
