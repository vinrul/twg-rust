use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use twg_map::{CameraFlight, MapCameraState};

use super::{BaseLayerResource, CameraFlightState, MapCameraResource, UiPointerState};
use crate::editor::EditorMode;

const KEYBOARD_PAN_SPEED: f64 = 1.1;

#[allow(clippy::too_many_arguments)]
pub fn map_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut layers: ResMut<BaseLayerResource>,
    mut flight: ResMut<CameraFlightState>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    editor: Option<Res<EditorMode>>,
    pointer: Res<UiPointerState>,
    mut camera: ResMut<MapCameraResource>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    if !window.focused {
        mouse_motion.clear();
        mouse_wheel.clear();
        return;
    }
    let mouse_inside = window.cursor_position().is_some();
    let editor_active = editor.is_some_and(|editor| editor.active);
    let camera = &mut camera.0;

    let mut cancel_flight = false;
    let mut delta = Vec2::ZERO;
    for motion in mouse_motion.read() {
        delta += motion.delta;
    }

    if mouse_inside
        && !pointer.over_ui
        && ((!editor_active && mouse_buttons.pressed(MouseButton::Left))
            || mouse_buttons.pressed(MouseButton::Middle))
    {
        if delta != Vec2::ZERO {
            cancel_flight = true;
        }
        camera.pan_from_screen_delta(
            delta.x,
            delta.y,
            window.physical_height() as f32,
            60.0f32.to_radians(),
        );
    }

    if mouse_inside && !pointer.over_ui && mouse_buttons.pressed(MouseButton::Right) {
        if delta != Vec2::ZERO {
            cancel_flight = true;
        }
        camera.rotate_by(delta.x * 0.25);
        camera.pitch_by(-delta.y * 0.25);
    }

    // Browsers report pixel deltas (often ~100 per notch) while native
    // backends report lines. Normalizing both to "lines" and clamping the
    // per-frame total prevents a single wheel notch from jumping straight to
    // MAX_ZOOM or MIN_ZOOM on the web.
    let mut wheel_lines = 0.0;
    for wheel in mouse_wheel.read() {
        let lines = match wheel.unit {
            MouseScrollUnit::Line => wheel.y,
            MouseScrollUnit::Pixel => wheel.y / 100.0,
        };
        wheel_lines += lines;
    }
    if mouse_inside && !pointer.over_ui && wheel_lines.abs() > f32::EPSILON {
        cancel_flight = true;
        camera.zoom_by(wheel_lines.clamp(-1.0, 1.0) * 0.15);
    }

    let dt = f64::from(time.delta_secs());
    // Typing in a text field must not pan, rotate, zoom, or reset the map.
    if !editor_active && !pointer.keyboard {
        let mut east = 0.0;
        let mut north = 0.0;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            north += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            north -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            east += 1.0;
        }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            east -= 1.0;
        }
        if east != 0.0 || north != 0.0 {
            cancel_flight = true;
            camera.pan_by_screen_tiles(
                east * KEYBOARD_PAN_SPEED * dt,
                north * KEYBOARD_PAN_SPEED * dt,
            );
        }

        if keys.pressed(KeyCode::KeyQ) {
            cancel_flight = true;
            camera.rotate_by(-45.0 * dt as f32);
        }
        if keys.pressed(KeyCode::KeyE) {
            cancel_flight = true;
            camera.rotate_by(45.0 * dt as f32);
        }
        if keys.pressed(KeyCode::KeyR) {
            cancel_flight = true;
            camera.pitch_by(30.0 * dt as f32);
        }
        if keys.pressed(KeyCode::KeyF) {
            cancel_flight = true;
            camera.pitch_by(-30.0 * dt as f32);
        }
    }

    if !pointer.keyboard {
        if keys.just_pressed(KeyCode::KeyH) {
            cancel_flight = true;
            *camera = MapCameraState::default();
        }
        if keys.just_pressed(KeyCode::KeyL) {
            layers.0 = layers.0.next();
        }
        if keys.just_pressed(KeyCode::Equal) {
            cancel_flight = true;
            camera.zoom_by(0.5);
        }
        if keys.just_pressed(KeyCode::Minus) {
            cancel_flight = true;
            camera.zoom_by(-0.5);
        }

        if cancel_flight {
            flight.0 = None;
        }

        if keys.just_pressed(KeyCode::KeyN) {
            let mut target = *camera;
            target.bearing_degrees = 0.0;
            flight.0 = Some(CameraFlight::new(*camera, target, 0.5));
        }
    } else if cancel_flight {
        flight.0 = None;
    }
}
