use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use twg_map::{CameraFlight, MapCameraState};

use super::{BaseLayerResource, CameraFlightState, MapCameraResource, UiPointerState};
use crate::editor::EditorMode;

const KEYBOARD_PAN_SPEED: f64 = 1.1;
/// Logical points panned per wheel line when Shift+wheel is used.
const WHEEL_PAN_POINTS_PER_LINE: f32 = 40.0;
/// Zoom levels per wheel line / 100 scroll pixels.
const WHEEL_ZOOM_STEP: f32 = 0.15;

#[allow(clippy::too_many_arguments)]
pub fn map_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut layers: ResMut<BaseLayerResource>,
    mut flight: ResMut<CameraFlightState>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut pinch_gesture: MessageReader<PinchGesture>,
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
        pinch_gesture.clear();
        return;
    }
    // The public big-screen page is a passive display: ignore every camera
    // gesture so only the driving admin's camera can move it.
    if editor.as_ref().is_some_and(|editor| !editor.camera_input) {
        mouse_motion.clear();
        mouse_wheel.clear();
        pinch_gesture.clear();
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
    //
    // Trackpads report precise pixel deltas with phases, so on native builds
    // two-finger scrolling pans the map. Physical wheels report line deltas
    // and keep zooming. Shift+scroll always pans and Ctrl/Cmd+scroll always
    // zooms, which also covers browser builds where mouse wheels and trackpads
    // both arrive as pixel deltas.
    let mut wheel_lines = 0.0f32;
    let mut wheel_pixels = Vec2::ZERO;
    for wheel in mouse_wheel.read() {
        match wheel.unit {
            MouseScrollUnit::Line => wheel_lines += wheel.y,
            MouseScrollUnit::Pixel => {
                wheel_pixels += Vec2::new(wheel.x, wheel.y);
            }
        }
    }
    let mut pinch = 0.0f32;
    for gesture in pinch_gesture.read() {
        pinch += gesture.0;
    }

    let scale_factor = window.scale_factor();
    // Shift = pan, Ctrl/Cmd = zoom. `Meta` maps to Super on macOS.
    let pan_modifier = keys.any_pressed([
        KeyCode::ShiftLeft,
        KeyCode::ShiftRight,
        KeyCode::AltLeft,
        KeyCode::AltRight,
    ]);
    let zoom_modifier = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);

    let gesture = scroll_gesture(
        wheel_lines,
        wheel_pixels,
        scale_factor,
        pan_modifier,
        zoom_modifier,
        // Browsers cannot tell a wheel from a trackpad; keep scroll-to-zoom
        // there and use Shift+scroll for panning.
        cfg!(not(target_arch = "wasm32")),
    );
    let pan = gesture.pan;
    let mut zoom = gesture.zoom;
    // Pinch zoom is reported by macOS (and iOS) independently of scrolling.
    if pinch.abs() > f32::EPSILON {
        zoom += (pinch * 4.0).clamp(-1.0, 1.0) * WHEEL_ZOOM_STEP;
    }

    if mouse_inside && !pointer.over_ui && (pan != Vec2::ZERO || zoom != 0.0) {
        cancel_flight = true;
        if pan != Vec2::ZERO {
            camera.pan_from_screen_delta(
                pan.x,
                pan.y,
                window.physical_height() as f32,
                60.0f32.to_radians(),
            );
        }
        if zoom != 0.0 {
            camera.zoom_by(zoom);
        }
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

/// Maps raw scroll deltas to camera pan/zoom.
///
/// - `lines`: traditional wheel notches (native backends).
/// - `pixels`: precise scrolling from a trackpad / Magic Mouse.
/// - `precise_pans`: when true, precise pixel deltas pan the map by default
///   (native); when false (browser, where wheels also report pixels) they zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ScrollGesture {
    pan: Vec2,
    zoom: f32,
}

fn scroll_gesture(
    lines: f32,
    pixels: Vec2,
    scale_factor: f32,
    pan_modifier: bool,
    zoom_modifier: bool,
    precise_pans: bool,
) -> ScrollGesture {
    let points = pixels / scale_factor.max(0.01);
    let wheel_zoom = |lines: f32| lines.clamp(-1.0, 1.0) * WHEEL_ZOOM_STEP;

    if pan_modifier {
        return ScrollGesture {
            pan: points + Vec2::new(0.0, lines * WHEEL_PAN_POINTS_PER_LINE),
            zoom: 0.0,
        };
    }
    if zoom_modifier || !precise_pans {
        return ScrollGesture {
            pan: Vec2::ZERO,
            zoom: wheel_zoom(points.y / 100.0 + lines),
        };
    }
    if points != Vec2::ZERO {
        ScrollGesture {
            pan: points,
            zoom: 0.0,
        }
    } else {
        ScrollGesture {
            pan: Vec2::ZERO,
            zoom: wheel_zoom(lines),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WHEEL_PAN_POINTS_PER_LINE, WHEEL_ZOOM_STEP, scroll_gesture};
    use bevy::prelude::Vec2;

    #[test]
    fn wheel_lines_zoom() {
        let gesture = scroll_gesture(1.0, Vec2::ZERO, 1.0, false, false, true);
        assert_eq!(gesture.pan, Vec2::ZERO);
        assert!((gesture.zoom - WHEEL_ZOOM_STEP).abs() < 1e-6);

        let out = scroll_gesture(-99.0, Vec2::ZERO, 1.0, false, false, true);
        assert!((out.zoom + WHEEL_ZOOM_STEP).abs() < 1e-6);
    }

    #[test]
    fn precise_scroll_pans_on_native() {
        let gesture = scroll_gesture(0.0, Vec2::new(20.0, -30.0), 2.0, false, false, true);
        assert_eq!(gesture.pan, Vec2::new(10.0, -15.0));
        assert_eq!(gesture.zoom, 0.0);
    }

    #[test]
    fn precise_scroll_zooms_in_the_browser() {
        let gesture = scroll_gesture(0.0, Vec2::new(0.0, 100.0), 1.0, false, false, false);
        assert_eq!(gesture.pan, Vec2::ZERO);
        assert!((gesture.zoom - WHEEL_ZOOM_STEP).abs() < 1e-6);
    }

    #[test]
    fn shift_scroll_pans_and_ctrl_scroll_zooms() {
        let pan = scroll_gesture(2.0, Vec2::new(10.0, 0.0), 1.0, true, false, true);
        assert_eq!(pan.pan, Vec2::new(10.0, 2.0 * WHEEL_PAN_POINTS_PER_LINE));
        assert_eq!(pan.zoom, 0.0);

        let zoom = scroll_gesture(0.0, Vec2::new(0.0, 100.0), 1.0, false, true, true);
        assert_eq!(zoom.pan, Vec2::ZERO);
        assert!((zoom.zoom - WHEEL_ZOOM_STEP).abs() < 1e-6);
    }
}
