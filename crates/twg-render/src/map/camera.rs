use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::MapCameraResource;

pub const CAMERA_FOV_Y: f32 = 60.0f32.to_radians();

/// Render layer used by the transform gizmo. A second camera draws only this
/// layer after the map camera, so the gizmo is never hidden by a model.
pub const GIZMO_RENDER_LAYER: usize = 15;

#[derive(Component)]
pub struct MapCamera;

/// Overlay camera for the transform gizmo. It shares the map camera's transform
/// and projection but clears no colour and uses its own depth buffer.
#[derive(Component)]
pub struct GizmoCamera;

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
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: CAMERA_FOV_Y,
            near: 0.1,
            far: 100_000.0,
            ..default()
        }),
        Msaa::Off,
        Transform::default(),
        RenderLayers::layer(GIZMO_RENDER_LAYER),
        GizmoCamera,
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
        * Vec3::new(0.0, distance * pitch.cos(), distance * pitch.sin());
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

/// Keeps the overlay gizmo camera aligned with the map camera. It renders after
/// the map camera with a fresh depth buffer, so gizmo handles stay visible even
/// when a model is much larger than the gizmo.
#[allow(clippy::type_complexity)]
pub fn sync_gizmo_camera(
    map: Query<(&Transform, &Projection), With<MapCamera>>,
    mut gizmo: Query<(&mut Transform, &mut Projection), (With<GizmoCamera>, Without<MapCamera>)>,
) {
    let Ok((map_transform, map_projection)) = map.single() else {
        return;
    };
    for (mut transform, mut projection) in &mut gizmo {
        *transform = *map_transform;
        if let (Projection::Perspective(source), Projection::Perspective(target)) =
            (map_projection, &mut *projection)
        {
            target.fov = source.fov;
            target.near = source.near;
            target.far = source.far;
        }
    }
}
