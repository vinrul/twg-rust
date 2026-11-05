use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::MapCameraResource;

pub const CAMERA_FOV_Y: f32 = 60.0f32.to_radians();

#[derive(Component)]
pub struct MapCamera;

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: CAMERA_FOV_Y,
            near: 0.1,
            far: 100_000.0,
            ..default()
        }),
        Msaa::Off,
        Transform::default(),
        MapCamera,
    ));
}

pub fn update_camera_transform(
    state: Res<MapCameraResource>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<MapCamera>>,
) {
    let state = state.0;
    let viewport_height = window.physical_height() as f32;
    let distance = state.distance_for_viewport(viewport_height, CAMERA_FOV_Y);
    let pitch = state.pitch_degrees.to_radians();
    let bearing = -state.bearing_degrees.to_radians();
    let offset = Quat::from_rotation_y(bearing)
        * Vec3::new(
            0.0,
            distance * pitch.cos(),
            distance * pitch.sin(),
        );
    let up = Quat::from_rotation_y(bearing) * Vec3::new(0.0, pitch.sin(), -pitch.cos());

    for (mut transform, mut projection) in &mut cameras {
        *transform = Transform::from_translation(offset).looking_at(Vec3::ZERO, up);
        if let Projection::Perspective(perspective) = &mut *projection {
            perspective.fov = CAMERA_FOV_Y;
            perspective.near = (distance / 2_000.0).max(0.05);
            perspective.far = (distance * 100.0).max(5_000.0);
        }
    }
}
