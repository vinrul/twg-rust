use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::{MapCamera, MapCameraResource, MapCursorResource};

pub fn update_cursor(
    camera: Single<(&Camera, &GlobalTransform), With<MapCamera>>,
    window: Single<&Window, With<PrimaryWindow>>,
    state: Res<MapCameraResource>,
    mut cursor: ResMut<MapCursorResource>,
) {
    let Some(position) = window.cursor_position() else {
        cursor.0.position = None;
        return;
    };
    let (camera, transform) = *camera;
    let Some(point) = camera
        .viewport_to_world(transform, position)
        .ok()
        .and_then(|ray| ray.plane_intersection_point(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y)))
    else {
        cursor.0.position = None;
        return;
    };
    cursor.0.position = state.0.world_to_geo(point.x, point.z);
}
