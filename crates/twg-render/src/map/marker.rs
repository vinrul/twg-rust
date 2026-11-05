use bevy::prelude::*;
use twg_map::MapCameraState;

use super::GeoAnchor;

#[derive(Component)]
pub struct DemoMarker;

pub fn spawn_demo_marker(mut commands: Commands, asset_server: Res<AssetServer>) {
    let camera = MapCameraState::default();
    let point = camera.world_to_geo(12.0, 12.0).unwrap_or(camera.center);
    let Some(anchor) = GeoAnchor::new(point) else {
        return;
    };

    commands.spawn((
        WorldAssetRoot(asset_server.load("models/motor_patroli_brimob_anim_1.glb#Scene0")),
        Transform::from_xyz(12.0, 0.0, 12.0).with_scale(Vec3::splat(0.02)),
        anchor,
        DemoMarker,
        Name::new("Demo tactical unit"),
    ));
}
