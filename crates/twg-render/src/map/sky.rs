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
/// power-of-two cubemap; 64 px is plenty for a smooth gradient.
const SKY_FACE_SIZE: u32 = 64;
const SKY_BRIGHTNESS: f32 = 1.1;
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
    let zenith = Vec3::new(0.05, 0.16, 0.45);
    let horizon = Vec3::new(0.52, 0.70, 0.94);
    let ground_near = Vec3::new(0.24, 0.27, 0.31);
    let ground_far = Vec3::new(0.035, 0.045, 0.055);

    let mut color = if direction.y >= 0.0 {
        horizon.lerp(zenith, direction.y.powf(0.55))
    } else {
        ground_near.lerp(ground_far, (-direction.y).powf(0.65))
    };
    let glow = direction.dot(sun).max(0.0);
    let sun_amount = glow.powf(96.0) * 0.9 + glow.powf(8.0) * 0.06;
    color += Vec3::new(1.0, 0.92, 0.78) * sun_amount;
    color
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
        let zenith = sky_color(Vec3::Y, sun);
        let horizon = sky_color(Vec3::X, sun);
        assert!(zenith.z / zenith.x > horizon.z / horizon.x);
    }
}
