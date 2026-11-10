//! Server-backed preview for the Aset screen.
//!
//! Admin asset previews always come from the server, never from the offline
//! cache: the cache exists for the editor/game, while the library screen must
//! show exactly what the server currently stores. A model can use two slots
//! (idle + animated); the animated slot auto-plays its first glTF animation.
//! Every model slot has its own camera and render target, supports auto-rotate,
//! and can be orbited with the mouse.

use bevy::asset::LoadState;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera, RenderTarget};
use bevy::gltf::{GltfAssetLabel, GltfLoaderSettings};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::transform::TransformSystems;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady};
use uuid::Uuid;

/// Number of simultaneous model preview slots.
pub const PREVIEW_SLOTS: usize = 2;
/// First render layer reserved for previews. Slot `n` uses
/// `PREVIEW_LAYER_BASE + n`.
pub const PREVIEW_LAYER_BASE: usize = 30;
/// Square size of each offscreen render target.
pub const PREVIEW_SIZE: u32 = 512;

const PREVIEW_FOV_Y: f32 = 45.0f32.to_radians();
const PREVIEW_MARGIN: f32 = 1.2;
const PREVIEW_MAX_FIT_ATTEMPTS: u8 = 8;
const PREVIEW_INITIAL_YAW: f32 = 0.42;
const PREVIEW_INITIAL_PITCH: f32 = 0.28;
const PREVIEW_AUTO_ROTATE_SPEED: f32 = 0.55;

/// Identifies one model URL to preview. `version` forces the preview to be
/// rebuilt when a conversion or re-upload replaces the remote file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPreviewKey {
    pub asset_id: Uuid,
    pub version: i64,
    /// Incremented by the UI when the user asks to retry a failed preview.
    pub retry: u64,
    pub label: String,
    pub url: String,
    pub draco: bool,
    /// First animation clip to auto-play, when this slot is the animated one.
    pub animation_url: Option<String>,
}

/// Identifies one texture/icon URL to preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePreviewKey {
    pub asset_id: Uuid,
    pub version: i64,
    /// Incremented by the UI when the user asks to retry a failed preview.
    pub retry: u64,
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewSource {
    Model(ModelPreviewKey),
    Image(ImagePreviewKey),
}

impl PreviewSource {
    pub fn asset_id(&self) -> Uuid {
        match self {
            Self::Model(key) => key.asset_id,
            Self::Image(key) => key.asset_id,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Model(key) => &key.label,
            Self::Image(key) => &key.label,
        }
    }

    pub fn is_model(&self) -> bool {
        matches!(self, Self::Model(_))
    }
}

/// Written by the UI; read by the preview renderer. Slot 0 is the primary
/// preview (image, static model, or animated model's idle pose); slot 1 is the
/// animated model's animation.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct AssetPreviewRequest(pub [Option<PreviewSource>; PREVIEW_SLOTS]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewPhase {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed,
}

/// Mouse/orbit input for one preview slot. Positive `delta_yaw` rotates the
/// camera to the right; positive `delta_pitch` moves it upward.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PreviewOrbitInput {
    pub delta_yaw: f32,
    pub delta_pitch: f32,
    /// True while the user drags the preview image; auto-rotation pauses.
    pub dragging: bool,
}

/// Latest orbit deltas published by the UI. Cleared after the renderer reads
/// them.
#[derive(Resource, Debug, Default)]
pub struct PreviewOrbitInputs(pub [PreviewOrbitInput; PREVIEW_SLOTS]);

/// Status of one server preview slot, plus the image egui can display. For
/// models this is the offscreen render target; for textures/icons it is the
/// image downloaded from the server.
#[derive(Default)]
pub struct PreviewSlot {
    pub asset_id: Option<Uuid>,
    pub phase: PreviewPhase,
    pub error: Option<String>,
    pub image: Option<Handle<Image>>,
    pub dimensions: Option<(u32, u32)>,
    pub label: String,
    source: Option<PreviewSource>,
    pending: Option<PreviewSource>,
    render_target: Option<Handle<Image>>,
    camera: Option<Entity>,
    root: Option<Entity>,
    model_load: Option<Handle<WorldAsset>>,
    image_load: Option<Handle<Image>>,
    animation: Option<PreviewAnimation>,
    animation_played: bool,
    parts_tagged: bool,
    fit_pending: bool,
    fit_attempts: u8,
    orbit_initialized: bool,
    center: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
    auto_rotate: bool,
}

impl PreviewSlot {
    pub fn is_model(&self) -> bool {
        self.source.as_ref().is_some_and(PreviewSource::is_model)
    }
}

#[derive(Clone)]
struct PreviewAnimation {
    graph: Handle<AnimationGraph>,
    index: AnimationNodeIndex,
    #[allow(dead_code)]
    clip: Handle<AnimationClip>,
}

#[derive(Resource, Default)]
pub struct AssetPreview {
    pub slots: [PreviewSlot; PREVIEW_SLOTS],
}

#[derive(Component)]
struct PreviewRoot;

#[derive(Component)]
struct PreviewPart;

#[derive(Component)]
struct PreviewCamera;

pub struct AssetPreviewPlugin;

impl Plugin for AssetPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetPreviewRequest>()
            .init_resource::<AssetPreview>()
            .init_resource::<PreviewOrbitInputs>()
            .add_systems(Startup, setup_preview)
            .add_systems(
                Update,
                (
                    handle_preview_request,
                    tag_preview_parts,
                    check_preview_loading,
                    play_preview_animations,
                )
                    .chain(),
            )
            // Starts the actual load in the next frame, after the despawn
            // commands from `handle_preview_request` and the asset server's
            // handle-drop processing have run. That way a refreshed version is
            // fetched again instead of reusing the previous asset.
            .add_systems(
                PreUpdate,
                start_pending_preview.after(bevy::asset::AssetTrackingSystems),
            )
            .add_systems(
                PostUpdate,
                (fit_preview_camera, update_preview_orbit)
                    .chain()
                    .after(TransformSystems::Propagate),
            )
            .add_observer(on_preview_instance_ready);
    }
}

fn setup_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut preview: ResMut<AssetPreview>,
) {
    for slot in 0..PREVIEW_SLOTS {
        let image = images.add(Image::new_target_texture(
            PREVIEW_SIZE,
            PREVIEW_SIZE,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        let camera = commands
            .spawn((
                Camera3d::default(),
                Camera {
                    is_active: false,
                    order: -1 - slot as isize,
                    clear_color: Color::srgb(0.055, 0.055, 0.055).into(),
                    ..default()
                },
                RenderTarget::Image(image.clone().into()),
                Projection::Perspective(PerspectiveProjection {
                    fov: PREVIEW_FOV_Y,
                    near: 0.01,
                    far: 10_000.0,
                    ..default()
                }),
                Transform::from_xyz(0.0, 1.6, 4.8).looking_at(Vec3::new(0.0, 0.6, 0.0), Vec3::Y),
                Msaa::Off,
                RenderLayers::layer(PREVIEW_LAYER_BASE + slot),
                PreviewCamera,
            ))
            .id();
        preview.slots[slot].camera = Some(camera);
        preview.slots[slot].render_target = Some(image);
        preview.slots[slot].auto_rotate = true;
    }

    let layers = RenderLayers::layer(PREVIEW_LAYER_BASE).with(PREVIEW_LAYER_BASE + 1);
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.6, -0.8, 0.0)),
        layers.clone(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 3_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, -0.3, 2.4, 0.0)),
        layers,
    ));
}

fn handle_preview_request(
    mut commands: Commands,
    request: Res<AssetPreviewRequest>,
    mut preview: ResMut<AssetPreview>,
    mut cameras: Query<&mut Camera, With<PreviewCamera>>,
) {
    for slot_index in 0..PREVIEW_SLOTS {
        let wanted = request.0[slot_index].clone();
        if preview.slots[slot_index].source == wanted {
            continue;
        }

        if let Some(root) = preview.slots[slot_index].root.take() {
            commands.entity(root).despawn();
        }
        let slot = &mut preview.slots[slot_index];
        slot.model_load = None;
        slot.image_load = None;
        slot.animation = None;
        slot.animation_played = false;
        slot.parts_tagged = false;
        slot.fit_pending = false;
        slot.fit_attempts = 0;
        slot.orbit_initialized = false;
        slot.error = None;
        slot.asset_id = wanted.as_ref().map(PreviewSource::asset_id);
        slot.label = wanted
            .as_ref()
            .map(|source| source.label().to_owned())
            .unwrap_or_default();
        slot.source = wanted.clone();
        slot.pending = wanted.clone();
        slot.image = None;
        slot.dimensions = None;
        slot.phase = if wanted.is_some() {
            PreviewPhase::Loading
        } else {
            PreviewPhase::Idle
        };
        slot.auto_rotate = true;

        let model_active = wanted.as_ref().is_some_and(PreviewSource::is_model);
        if let Some(camera) = slot.camera
            && let Ok(mut camera) = cameras.get_mut(camera)
        {
            camera.is_active = model_active;
        }
    }
}

fn start_pending_preview(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut preview: ResMut<AssetPreview>,
) {
    for slot_index in 0..PREVIEW_SLOTS {
        let Some(source) = preview.slots[slot_index].pending.take() else {
            continue;
        };
        match source {
            PreviewSource::Image(key) => {
                let handle: Handle<Image> = asset_server.load(key.url);
                preview.slots[slot_index].image = Some(handle.clone());
                preview.slots[slot_index].image_load = Some(handle);
            }
            PreviewSource::Model(key) => {
                let handle = load_preview_scene(&asset_server, &key.url, key.draco);
                let layer = RenderLayers::layer(PREVIEW_LAYER_BASE + slot_index);
                let root = commands
                    .spawn((
                        WorldAssetRoot(handle.clone()),
                        Transform::default(),
                        Visibility::default(),
                        layer,
                        PreviewRoot,
                        Name::new(format!("asset-preview-{slot_index}")),
                    ))
                    .id();
                let animation = key.animation_url.as_deref().map(|url| {
                    let clip = load_preview_animation(&asset_server, url, key.draco);
                    let (graph, index) = AnimationGraph::from_clip(clip.clone());
                    PreviewAnimation {
                        graph: graphs.add(graph),
                        index,
                        clip,
                    }
                });
                let slot = &mut preview.slots[slot_index];
                slot.root = Some(root);
                slot.model_load = Some(handle);
                slot.animation = animation;
                slot.image = slot.render_target.clone();
            }
        }
    }
}

fn load_preview_scene(asset_server: &AssetServer, url: &str, draco: bool) -> Handle<WorldAsset> {
    let asset_path = GltfAssetLabel::Scene(0).from_asset(url.to_owned());
    if draco {
        asset_server
            .load_builder()
            .with_settings(|settings: &mut GltfLoaderSettings| settings.validate = false)
            .load(asset_path)
    } else {
        asset_server.load(asset_path)
    }
}

fn load_preview_animation(
    asset_server: &AssetServer,
    url: &str,
    draco: bool,
) -> Handle<AnimationClip> {
    let asset_path = GltfAssetLabel::Animation(0).from_asset(url.to_owned());
    if draco {
        asset_server
            .load_builder()
            .with_settings(|settings: &mut GltfLoaderSettings| settings.validate = false)
            .load(asset_path)
    } else {
        asset_server.load(asset_path)
    }
}

fn check_preview_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut preview: ResMut<AssetPreview>,
) {
    for slot in &mut preview.slots {
        if slot.phase != PreviewPhase::Loading {
            continue;
        }
        match slot.source.clone() {
            Some(PreviewSource::Model(_)) => {
                let Some(handle) = &slot.model_load else {
                    continue;
                };
                if let Some(LoadState::Failed(error)) = asset_server.get_load_state(handle.id()) {
                    if let Some(root) = slot.root.take() {
                        commands.entity(root).despawn();
                    }
                    slot.model_load = None;
                    slot.phase = PreviewPhase::Failed;
                    slot.error = Some(format!("gagal memuat model: {error}"));
                }
            }
            Some(PreviewSource::Image(_)) => {
                let Some(handle) = &slot.image_load else {
                    continue;
                };
                match asset_server.get_load_state(handle.id()) {
                    Some(LoadState::Loaded) => {
                        let Some(image) = images.get(handle) else {
                            continue;
                        };
                        slot.dimensions = Some((image.width(), image.height()));
                        slot.phase = PreviewPhase::Ready;
                    }
                    Some(LoadState::Failed(error)) => {
                        slot.phase = PreviewPhase::Failed;
                        slot.error = Some(format!("gagal memuat gambar: {error}"));
                    }
                    _ => {}
                }
            }
            None => {}
        }
    }
}

/// Scene instances spawn their children asynchronously. Mark them with the
/// preview layer as soon as they exist, otherwise the map camera would see the
/// model too (or the preview camera would see nothing).
fn tag_preview_parts(
    mut commands: Commands,
    mut preview: ResMut<AssetPreview>,
    children: Query<&Children>,
    tagged: Query<(), With<PreviewPart>>,
) {
    for slot_index in 0..PREVIEW_SLOTS {
        if preview.slots[slot_index].parts_tagged {
            continue;
        }
        let Some(root) = preview.slots[slot_index].root else {
            continue;
        };

        let layer = RenderLayers::layer(PREVIEW_LAYER_BASE + slot_index);
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            let Ok(list) = children.get(entity) else {
                continue;
            };
            for child in list.iter() {
                if tagged.get(child).is_err() {
                    commands.entity(child).insert((PreviewPart, layer.clone()));
                }
                stack.push(child);
            }
        }
        preview.slots[slot_index].parts_tagged = true;
    }
}

fn on_preview_instance_ready(
    ready: On<WorldInstanceReady>,
    mut preview: ResMut<AssetPreview>,
    parents: Query<&ChildOf>,
) {
    for slot in &mut preview.slots {
        let Some(root) = slot.root else {
            continue;
        };
        if !is_descendant_or_self(ready.entity, root, &parents) {
            continue;
        }
        slot.parts_tagged = false;
        slot.fit_pending = true;
    }
}

fn play_preview_animations(
    mut commands: Commands,
    mut preview: ResMut<AssetPreview>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for slot in &mut preview.slots {
        if slot.animation_played {
            continue;
        }
        let Some(root) = slot.root else {
            continue;
        };
        let Some(animation) = slot.animation.as_ref() else {
            continue;
        };
        let mut candidates = vec![root];
        candidates.extend(children.iter_descendants(root));
        for entity in candidates {
            if let Ok(mut player) = players.get_mut(entity) {
                player.play(animation.index).repeat();
                commands
                    .entity(entity)
                    .insert(AnimationGraphHandle(animation.graph.clone()));
                slot.animation_played = true;
                break;
            }
        }
    }
}

fn is_descendant_or_self(entity: Entity, root: Entity, parents: &Query<&ChildOf>) -> bool {
    let mut current = entity;
    loop {
        if current == root {
            return true;
        }
        let Ok(parent) = parents.get(current) else {
            return false;
        };
        current = parent.parent();
    }
}

fn fit_preview_camera(
    mut preview: ResMut<AssetPreview>,
    asset_server: Res<AssetServer>,
    meshes: Res<Assets<Mesh>>,
    children: Query<&Children>,
    parts: Query<(&Mesh3d, &GlobalTransform)>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<PreviewCamera>>,
) {
    for slot in &mut preview.slots {
        if !slot.fit_pending {
            continue;
        }
        let Some(root) = slot.root else {
            slot.fit_pending = false;
            continue;
        };
        let loaded = slot.model_load.as_ref().is_some_and(|handle| {
            matches!(
                asset_server.get_load_state(handle.id()),
                Some(LoadState::Loaded)
            )
        });
        if !loaded {
            continue;
        }

        let Some((min, max)) = preview_bounds(root, &children, &parts, &meshes) else {
            slot.fit_attempts += 1;
            if slot.fit_attempts >= PREVIEW_MAX_FIT_ATTEMPTS {
                slot.fit_pending = false;
                slot.phase = PreviewPhase::Failed;
                slot.error = Some("model tidak memiliki mesh yang bisa dipreview".to_owned());
            }
            continue;
        };

        let center = (min + max) * 0.5;
        let radius = ((max - min).length() * 0.5).max(0.001);
        slot.center = center;
        slot.distance = radius / (PREVIEW_FOV_Y * 0.5).tan() * PREVIEW_MARGIN;
        if !slot.orbit_initialized {
            slot.yaw = PREVIEW_INITIAL_YAW;
            slot.pitch = PREVIEW_INITIAL_PITCH;
            slot.orbit_initialized = true;
        }
        slot.fit_pending = false;
        slot.fit_attempts = 0;
        slot.dimensions = Some((PREVIEW_SIZE, PREVIEW_SIZE));
        slot.phase = PreviewPhase::Ready;
        slot.error = None;
        apply_preview_camera(slot, &mut cameras);
    }
}

fn update_preview_orbit(
    time: Res<Time>,
    mut preview: ResMut<AssetPreview>,
    mut orbit: ResMut<PreviewOrbitInputs>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<PreviewCamera>>,
) {
    for (index, slot) in preview.slots.iter_mut().enumerate() {
        if slot.phase != PreviewPhase::Ready || !slot.is_model() {
            continue;
        }
        let input = orbit.0[index];
        slot.yaw += input.delta_yaw;
        slot.pitch = (slot.pitch + input.delta_pitch).clamp(-1.35, 1.35);
        if !input.dragging && slot.auto_rotate {
            slot.yaw += time.delta_secs() * PREVIEW_AUTO_ROTATE_SPEED;
        }
        apply_preview_camera(slot, &mut cameras);
    }
    orbit.0 = [PreviewOrbitInput::default(); PREVIEW_SLOTS];
}

fn apply_preview_camera(
    slot: &PreviewSlot,
    cameras: &mut Query<(&mut Transform, &mut Projection), With<PreviewCamera>>,
) {
    let Some(entity) = slot.camera else {
        return;
    };
    let Ok((mut transform, mut projection)) = cameras.get_mut(entity) else {
        return;
    };
    let direction = Vec3::new(
        slot.pitch.cos() * slot.yaw.sin(),
        slot.pitch.sin(),
        slot.pitch.cos() * slot.yaw.cos(),
    );
    let eye = slot.center + direction * slot.distance;
    *transform = Transform::from_translation(eye).looking_at(slot.center, Vec3::Y);
    if let Projection::Perspective(perspective) = &mut *projection {
        perspective.fov = PREVIEW_FOV_Y;
        perspective.near = (slot.distance * 0.001).max(0.001);
        perspective.far = (slot.distance * 20.0).max(100.0);
    }
}

fn preview_bounds(
    root: Entity,
    children: &Query<&Children>,
    parts: &Query<(&Mesh3d, &GlobalTransform)>,
    meshes: &Assets<Mesh>,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3A::splat(f32::INFINITY);
    let mut max = Vec3A::splat(f32::NEG_INFINITY);
    let mut found = false;

    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok((mesh3d, transform)) = parts.get(entity)
            && let Some(mesh) = meshes.get(&mesh3d.0)
            && let Some(positions) = mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .and_then(|attribute| attribute.as_float3())
            && let Some(local) =
                Aabb::enclosing(positions.iter().map(|position| Vec3::from_array(*position)))
        {
            let affine = transform.affine();
            let center = affine.transform_point3a(local.center);
            let half = affine.matrix3.abs() * local.half_extents;
            min = min.min(center - half);
            max = max.max(center + half);
            found = true;
        }

        if let Ok(list) = children.get(entity) {
            stack.extend(list.iter());
        }
    }

    found.then_some((min.into(), max.into()))
}
