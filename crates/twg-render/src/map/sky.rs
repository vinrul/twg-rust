//! Procedural sky for the map camera.
//!
//! The same generated cubemap drives both the visible skybox and the optional
//! image-based lighting, so the two modes cannot drift apart. No external HDR
//! file is required: a small gradient cubemap is built at startup.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::light::Skybox;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};

use super::camera::MapCamera;
use super::environment::GroundPlane;

/// Cubemap face resolution. `GeneratedEnvironmentMapLight` requires a square
/// power-of-two cubemap; 128 px keeps the clouds readable without a big asset.
const SKY_FACE_SIZE: u32 = 128;
/// Bevy multiplies skybox brightness by the camera exposure, which is about
/// 0.0011 with the default `Exposure` (EV100 9.7). A small value would render
/// almost black, so the sky is authored in the same range as Bevy's examples.
const SKY_BRIGHTNESS: f32 = 900.0;
const IBL_INTENSITY: f32 = 1_200.0;
const AMBIENT_BRIGHTNESS: f32 = 900.0;
const IBL_AMBIENT_BRIGHTNESS: f32 = 150.0;

/// Background style for the map view (editor and Play share it).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkyMode {
    /// Previous behaviour: the dark clear colour is visible when pitching.
    #[default]
    Black,
    /// Gradient sky dome without changing the scene lighting.
    Skybox,
    /// Gradient sky dome plus image-based lighting from the same cubemap.
    Environment,
}

impl SkyMode {
    pub const ALL: [Self; 3] = [Self::Black, Self::Skybox, Self::Environment];
}

#[derive(Resource)]
pub(super) struct SkyCubemap(Handle<Image>);

pub fn spawn_sky_cubemap(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(SkyCubemap(images.add(sky_cubemap(SKY_FACE_SIZE))));
}

pub fn apply_sky_mode(
    mode: Res<SkyMode>,
    cubemap: Res<SkyCubemap>,
    mut applied: Local<Option<SkyMode>>,
    ambient: Option<ResMut<GlobalAmbientLight>>,
    cameras: Query<Entity, With<MapCamera>>,
    mut grounds: Query<&mut Visibility, With<GroundPlane>>,
    mut commands: Commands,
) {
    if *applied == Some(*mode) {
        return;
    }
    *applied = Some(*mode);

    // IBL replaces the flat ambient term; otherwise the scene would be lit
    // twice when the sky map is enabled.
    if let Some(mut ambient) = ambient {
        ambient.brightness = match *mode {
            SkyMode::Environment => IBL_AMBIENT_BRIGHTNESS,
            SkyMode::Black | SkyMode::Skybox => AMBIENT_BRIGHTNESS,
        };
    }

    // The fallback ground is almost black and would hide the sky at the
    // horizon; the sky cubemap already has a ground gradient below it.
    let ground_visibility = if *mode == SkyMode::Black {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut grounds {
        *visibility = ground_visibility;
    }

    for camera in &cameras {
        commands
            .entity(camera)
            .remove::<(Skybox, GeneratedEnvironmentMapLight, EnvironmentMapLight)>();
        match *mode {
            SkyMode::Black => {}
            SkyMode::Skybox => {
                commands.entity(camera).insert(Skybox {
                    image: Some(cubemap.0.clone()),
                    brightness: SKY_BRIGHTNESS,
                    ..default()
                });
            }
            SkyMode::Environment => {
                commands
                    .entity(camera)
                    .insert(Skybox {
                        image: Some(cubemap.0.clone()),
                        brightness: SKY_BRIGHTNESS,
                        ..default()
                    })
                    .insert(GeneratedEnvironmentMapLight {
                        environment_map: cubemap.0.clone(),
                        intensity: IBL_INTENSITY,
                        ..default()
                    });
            }
        }
    }
}

fn sky_cubemap(size: u32) -> Image {
    let sun = Vec3::new(0.35, 0.62, -0.70).normalize();
    let mut data = Vec::with_capacity((size * size * 6 * 4) as usize);
    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let direction = face_direction(face, u, v).normalize_or_zero();
                let color = sky_color(direction, sun);
                data.extend_from_slice(&[
                    to_byte(color.x),
                    to_byte(color.y),
                    to_byte(color.z),
                    255,
                ]);
            }
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image
}

/// Standard cubemap face order: +X, -X, +Y, -Y, +Z, -Z.
fn face_direction(face: usize, u: f32, v: f32) -> Vec3 {
    match face {
        0 => Vec3::new(1.0, -v, -u),
        1 => Vec3::new(-1.0, -v, u),
        2 => Vec3::new(u, 1.0, v),
        3 => Vec3::new(u, -1.0, -v),
        4 => Vec3::new(u, -v, 1.0),
        _ => Vec3::new(-u, -v, -1.0),
    }
}

fn sky_color(direction: Vec3, sun: Vec3) -> Vec3 {
    let mut color = sky_gradient(direction, sun);

    // Project the upper hemisphere onto a virtual cloud plane. The noise is
    // evaluated from world-space directions, so it stays continuous across
    // cubemap faces, and it fades out at the horizon.
    let height = smoothstep(0.03, 0.22, direction.y);
    if height > 0.0 {
        let uv = Vec2::new(direction.x, direction.z) / (direction.y + 0.35);
        let coverage = fbm(uv.x * 1.7 + 3.1, uv.y * 1.7 - 1.7);
        let clouds = smoothstep(0.48, 0.70, coverage) * height;
        let cloud_color = Vec3::new(0.86, 0.89, 0.93);
        color = color.lerp(cloud_color, clouds * 0.85);
    }
    color
}

fn sky_gradient(direction: Vec3, sun: Vec3) -> Vec3 {
    let zenith = Vec3::new(0.05, 0.16, 0.45);
    let horizon = Vec3::new(0.52, 0.70, 0.94);
    let deep_ground = Vec3::new(0.08, 0.10, 0.14);

    let mut color = if direction.y >= 0.0 {
        horizon.lerp(zenith, direction.y.powf(0.55))
    } else {
        // Start exactly at the horizon colour and fade into a dark, slightly
        // blue ground. Smoothstep keeps the derivative zero at the horizon, so
        // the band beyond a finite map plane reads as distant haze instead of
        // a grey stripe with a hard edge.
        horizon.lerp(deep_ground, smoothstep(0.0, 0.45, -direction.y))
    };
    let glow = direction.dot(sun).max(0.0);
    let sun_amount = glow.powf(96.0) * 0.9 + glow.powf(8.0) * 0.06;
    color += Vec3::new(1.0, 0.92, 0.78) * sun_amount;
    color
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fbm(x: f32, y: f32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 0.5;
    let mut frequency = 1.0;
    for _ in 0..4 {
        value += value_noise(x * frequency, y * frequency) * amplitude;
        frequency *= 2.03;
        amplitude *= 0.5;
    }
    value
}

fn value_noise(x: f32, y: f32) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let x0 = x0 as i32;
    let y0 = y0 as i32;
    let a = hash_2d(x0, y0);
    let b = hash_2d(x0 + 1, y0);
    let c = hash_2d(x0, y0 + 1);
    let d = hash_2d(x0 + 1, y0 + 1);
    let top = a + (b - a) * ux;
    let bottom = c + (d - c) * ux;
    top + (bottom - top) * uy
}

fn hash_2d(x: i32, y: i32) -> f32 {
    let mut hash = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    hash = (hash ^ (hash >> 13)).wrapping_mul(1_274_126_177);
    ((hash ^ (hash >> 16)) & 0xffff) as f32 / 65_535.0
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubemap_has_six_faces_with_a_cube_view() {
        let size = 8;
        let image = sky_cubemap(size);
        assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 6);
        assert_eq!(
            image.data.as_ref().map(Vec::len),
            Some((size * size * 6 * 4) as usize)
        );
        assert_eq!(
            image
                .texture_view_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.dimension),
            Some(TextureViewDimension::Cube)
        );
    }

    #[test]
    fn zenith_is_bluer_than_the_horizon() {
        let sun = Vec3::new(0.35, 0.62, -0.70).normalize();
        let zenith = sky_gradient(Vec3::Y, sun);
        let horizon = sky_gradient(Vec3::X, sun);
        assert!(zenith.z / zenith.x > horizon.z / horizon.x);
    }

    #[test]
    fn horizon_colour_is_continuous() {
        let sun = Vec3::new(0.35, 0.62, -0.70).normalize();
        let above = sky_gradient(Vec3::new(0.0, 0.0001, -1.0).normalize(), sun);
        let below = sky_gradient(Vec3::new(0.0, -0.0001, -1.0).normalize(), sun);
        assert!((above - below).length() < 0.01);
    }

    #[test]
    fn below_horizon_starts_as_bluish_haze() {
        let sun = Vec3::new(0.35, 0.62, -0.70).normalize();
        let band = sky_gradient(Vec3::new(0.0, -0.09, -1.0).normalize(), sun);
        assert!(band.z > band.x);
    }

    #[test]
    fn cubemap_contains_visible_colors() {
        let image = sky_cubemap(16);
        let data = image.data.expect("sky cubemap data");
        let max = data
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel| &pixel[..3])
            .copied()
            .max()
            .unwrap_or(0);
        assert!(max > 128, "cubemap looks too dark (max {max})");
    }

    #[test]
    fn skybox_brightness_compensates_the_camera_exposure() {
        let exposure = bevy::camera::Exposure::default().exposure();
        let effective = SKY_BRIGHTNESS * exposure;
        assert!(
            (0.5..=2.0).contains(&effective),
            "sky would be too dark or blown out (effective {effective})"
        );
    }

    #[test]
    fn sky_mode_attaches_and_removes_the_camera_skybox() {
        let mut world = World::new();
        world.init_resource::<SkyMode>();
        world.insert_resource(SkyCubemap(Handle::default()));
        let camera = world.spawn(MapCamera).id();
        world.spawn((GroundPlane, Visibility::Inherited));

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_sky_mode);

        *world.resource_mut::<SkyMode>() = SkyMode::Skybox;
        schedule.run(&mut world);
        assert!(world.entity(camera).contains::<Skybox>());
        let mut grounds = world.query_filtered::<&Visibility, With<GroundPlane>>();
        assert!(
            grounds
                .iter(&world)
                .all(|visibility| *visibility == Visibility::Hidden)
        );

        *world.resource_mut::<SkyMode>() = SkyMode::Black;
        schedule.run(&mut world);
        assert!(!world.entity(camera).contains::<Skybox>());
        let mut grounds = world.query_filtered::<&Visibility, With<GroundPlane>>();
        assert!(
            grounds
                .iter(&world)
                .all(|visibility| *visibility == Visibility::Inherited)
        );
    }
}
