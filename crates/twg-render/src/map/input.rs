use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use twg_map::MapCameraState;

use super::{BaseLayerResource, MapCameraResource};

const KEYBOARD_PAN_SPEED: f64 = 1.1;

pub fn map_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut layers: ResMut<BaseLayerResource>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
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
    let camera = &mut camera.0;

    let mut delta = Vec2::ZERO;
    for motion in mouse_motion.read() {
        delta += motion.delta;
    }

    if mouse_inside
        && (mouse_buttons.pressed(MouseButton::Left) || mouse_buttons.pressed(MouseButton::Middle))
    {
        camera.pan_from_screen_delta(
            delta.x,
            delta.y,
            window.physical_height() as f32,
            60.0f32.to_radians(),
        );
    }

    if mouse_inside && mouse_buttons.pressed(MouseButton::Right) {
        camera.rotate_by(delta.x * 0.25);
        camera.pitch_by(-delta.y * 0.25);
    }

    let mut wheel_delta = 0.0;
    for wheel in mouse_wheel.read() {
        wheel_delta += wheel.y;
    }
    if mouse_inside && wheel_delta.abs() > f32::EPSILON {
        camera.zoom_by(wheel_delta * 0.15);
    }

    let dt = f64::from(time.delta_secs());
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
        let bearing = f64::from(camera.bearing_degrees.to_radians());
        let rotated_east = east * bearing.cos() - north * bearing.sin();
        let rotated_north = east * bearing.sin() + north * bearing.cos();
        camera.pan_by_tiles(
            rotated_east * KEYBOARD_PAN_SPEED * dt,
            rotated_north * KEYBOARD_PAN_SPEED * dt,
        );
    }

    if keys.pressed(KeyCode::KeyQ) {
        camera.rotate_by(-45.0 * dt as f32);
    }
    if keys.pressed(KeyCode::KeyE) {
        camera.rotate_by(45.0 * dt as f32);
    }
    if keys.pressed(KeyCode::KeyR) {
        camera.pitch_by(30.0 * dt as f32);
    }
    if keys.pressed(KeyCode::KeyF) {
        camera.pitch_by(-30.0 * dt as f32);
    }

    if keys.just_pressed(KeyCode::KeyH) {
        *camera = MapCameraState::default();
    }
    if keys.just_pressed(KeyCode::KeyL) {
        layers.0 = layers.0.next();
    }
    if keys.just_pressed(KeyCode::Equal) {
        camera.zoom_by(0.5);
    }
    if keys.just_pressed(KeyCode::Minus) {
        camera.zoom_by(-0.5);
    }
}
