use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::MapCameraResource;
use super::camera::CAMERA_FOV_Y;

#[derive(Component)]
pub struct GroundPlane;

pub fn spawn_environment(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 900.0,
        affects_lightmapped_meshes: true,
    });

    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, -0.35, 0.0)),
    ));

    let ground = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(0.5)));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.012, 0.035, 0.055),
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(ground),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, -0.05, 0.0),
        GroundPlane,
        Name::new("Map fallback ground"),
    ));
}

pub fn update_ground(
    state: Res<MapCameraResource>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut grounds: Query<&mut Transform, With<GroundPlane>>,
) {
    let distance = state
        .0
        .distance_for_viewport(window.physical_height() as f32, CAMERA_FOV_Y);
    let scale = (distance * 20.0).max(1_000.0);
    for mut transform in &mut grounds {
        *transform = Transform::from_xyz(0.0, -0.05, 0.0).with_scale(Vec3::splat(scale));
    }
}
