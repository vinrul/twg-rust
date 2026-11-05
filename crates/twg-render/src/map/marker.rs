use bevy::prelude::*;

#[derive(Component)]
pub struct DemoMarker;

pub fn spawn_demo_marker(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        WorldAssetRoot(asset_server.load("models/motor_patroli_brimob_anim_1.glb#Scene0")),
        Transform::from_xyz(12.0, 0.0, 12.0).with_scale(Vec3::splat(0.02)),
        DemoMarker,
        Name::new("Demo tactical unit"),
    ));
}
