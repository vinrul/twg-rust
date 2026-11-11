use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::gltf::{GltfAssetLabel, GltfLoaderSettings};
use bevy::image::Image;
use bevy::mesh::VertexAttributeValues;
use bevy::pbr::wireframe::Wireframe;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use twg_map::GeoPoint;

use super::mesh;
use crate::map::{GeoAnchor, MapCamera, MapCameraResource};

/// Per-frame RTS play state pushed by the UI layer. Unlike [`EditorScene`]
/// this resource is updated every frame and never rebuilds the entity graph;
/// it only moves and animates the existing marker roots.
#[derive(Resource, Debug, Clone, Default)]
pub struct EditorPlayFrame {
    pub active: bool,
    pub units: Vec<EditorPlayUnit>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorPlayUnit {
    pub key: u64,
    pub lon: f64,
    pub lat: f64,
    /// Height in meters above the map plane.
    pub altitude: f32,
    /// Bearing in degrees clockwise from north.
    pub heading_deg: f32,
    pub moving: bool,
    /// Animated units play their walk clip. Static movable units only turn
    /// toward their movement direction.
    pub animated: bool,
}

/// Enables editing behaviour and tells map input to stop using left-drag for
/// panning. `preview` keeps the scene visible while disabling editor input.
#[derive(Resource, Debug, Clone, Copy)]
pub struct EditorMode {
    pub active: bool,
    pub preview: bool,
    /// Editor-only particle animation toggle. Play/simulation always animates.
    pub particles_animate: bool,
}

impl Default for EditorMode {
    fn default() -> Self {
        Self {
            active: false,
            preview: false,
            particles_animate: true,
        }
    }
}

/// Render snapshot produced by the UI layer every frame the editor scene
/// changes. Keeping this a plain data resource decouples editor state from the
/// Bevy entity graph.
#[derive(Resource, Debug, Clone, Default)]
pub struct EditorScene {
    pub revision: u64,
    pub indoor: bool,
    pub objects: Vec<EditorSceneObject>,
    pub grid: Option<EditorGrid>,
    pub gizmo: Option<EditorGizmo>,
    /// Shading used for imported glTF models in the editor viewport.
    pub model_shading: EditorModelShading,
}

/// Viewport shading for imported models, mirroring Blender's modes. Only the
/// editor uses it; gameplay/simulation always renders textures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EditorModelShading {
    #[default]
    Textured,
    Solid,
    Wireframe,
}

impl EditorModelShading {
    pub const ALL: [Self; 3] = [Self::Textured, Self::Solid, Self::Wireframe];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Textured => "Texture",
            Self::Solid => "Solid",
            Self::Wireframe => "Wireframe",
        }
    }
}

/// Primitive shape used by editor "Bentuk" objects. Custom uses the authored
/// footprint polygon (the old building/polygon behaviour).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum EditorShapeKind {
    #[default]
    Box,
    Cylinder,
    Sphere,
    Cone,
    Ground,
    Custom,
}

impl EditorShapeKind {
    pub const ALL: [Self; 6] = [
        Self::Box,
        Self::Cylinder,
        Self::Sphere,
        Self::Cone,
        Self::Ground,
        Self::Custom,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Box => "Kubus / Kotak",
            Self::Cylinder => "Silinder",
            Self::Sphere => "Bola",
            Self::Cone => "Kerucut",
            Self::Ground => "Plane / Ground",
            Self::Custom => "Custom mesh",
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Self::Box => "box",
            Self::Cylinder => "cylinder",
            Self::Sphere => "sphere",
            Self::Cone => "cone",
            Self::Ground => "ground",
            Self::Custom => "custom",
        }
    }

    pub fn from_key(key: &str) -> Self {
        match key.trim().to_ascii_lowercase().as_str() {
            "box" | "cube" | "kubus" => Self::Box,
            "cylinder" | "silinder" => Self::Cylinder,
            "sphere" | "bola" => Self::Sphere,
            "cone" | "kerucut" => Self::Cone,
            // Legacy scenarios stored the old flat disc as `circle`; keep them
            // as a plane so their footprint stays flat.
            "circle" | "ground" | "plane" => Self::Ground,
            _ => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum EditorParticleKind {
    #[default]
    Fire,
    Smoke,
    Water,
}

/// Per-object particle effect tuning, persisted with the scenario.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorParticleSettings {
    pub size: f32,
    pub density: f32,
    pub height: f32,
    pub spread: f32,
    pub speed: f32,
    pub opacity: f32,
}

impl Default for EditorParticleSettings {
    fn default() -> Self {
        Self {
            size: 1.0,
            density: 1.0,
            height: 1.0,
            spread: 1.0,
            speed: 1.0,
            opacity: 1.0,
        }
    }
}

impl EditorParticleSettings {
    fn clamped(self) -> Self {
        Self {
            size: self.size.clamp(0.25, 4.0),
            density: self.density.clamp(0.25, 3.0),
            height: self.height.clamp(0.25, 3.0),
            spread: self.spread.clamp(0.25, 3.0),
            speed: self.speed.clamp(0.25, 3.0),
            opacity: self.opacity.clamp(0.1, 2.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorSceneObject {
    pub key: u64,
    pub label: String,
    pub lon: f64,
    pub lat: f64,
    pub selected: bool,
    pub kind: EditorSceneKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorPlayModel {
    pub model: String,
    pub draco: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditorSceneKind {
    Marker {
        model: String,
        base_scale: f32,
        /// When set, the spawned mesh is rescaled once it loads so its own
        /// height matches this value in meters. Used by models coming from the
        /// asset library, whose export units are unknown.
        normalize_height: Option<f32>,
        /// The model uses `KHR_draco_mesh_compression`.
        draco: bool,
        /// Load the whole glTF scene (with its own materials and textures)
        /// instead of the first primitive with a tint colour. Used by models
        /// coming from the asset library.
        scene_asset: bool,
        /// Play only: the model holds an animation clip. `model` is the
        /// animated file used while the unit moves; this is the idle file that
        /// is shown again once the unit stops.
        play_idle: Option<EditorPlayModel>,
        rotation_deg: [f32; 3],
        scale: [f32; 3],
        altitude: f32,
        color: [f32; 4],
    },
    Line {
        points: Vec<[f64; 2]>,
        /// Per-point height in meters, parallel to `points`.
        altitudes: Vec<f32>,
        width: f32,
        color: [f32; 4],
        alpha: f32,
    },
    Building {
        shape: EditorShapeKind,
        points: Vec<[f64; 2]>,
        height: f32,
        color: [f32; 4],
        rotation_deg: [f32; 3],
        scale: [f32; 3],
        altitude: f32,
        /// Cached/remote image path used as the base colour texture.
        texture: Option<String>,
    },
    Text {
        color: [f32; 4],
        font_size: f32,
    },
    Particle {
        kind: EditorParticleKind,
        altitude: f32,
        settings: EditorParticleSettings,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorGrid {
    pub center: [f64; 2],
    pub spacing_m: f32,
    pub extent_m: f32,
    pub floor_plan: bool,
    pub floor_size_m: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorGizmoAxis {
    X,
    Y,
    Z,
    RotateX,
    RotateY,
    RotateZ,
    /// Center handle that scales every axis at once.
    Uniform,
}

/// Which handles the transform gizmo shows. The editor toolbar picks this from
/// the active tool so the gizmo matches move/rotate/scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorGizmoMode {
    Move,
    Rotate,
    Scale,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorGizmo {
    pub lon: f64,
    pub lat: f64,
    pub altitude: f32,
    pub mode: EditorGizmoMode,
    pub active_axis: Option<EditorGizmoAxis>,
    /// Custom mesh points are flat, so their move gizmo hides the Y axis.
    pub show_y: bool,
    /// Set when this gizmo belongs to a Play `Limited` asset. The renderer
    /// follows that unit's runtime position instead of the authored one.
    pub play_key: Option<u64>,
}

#[derive(Resource, Default)]
pub(crate) struct EditorRenderCache {
    revision: u64,
    entities: Vec<Entity>,
    gizmo: Option<Entity>,
    grid: Option<Entity>,
    floor: Option<Entity>,
    marker_meshes: HashMap<String, Handle<Mesh>>,
    marker_materials: HashMap<[u8; 4], Handle<StandardMaterial>>,
    gizmo_meshes: Option<GizmoMeshes>,
    gizmo_materials: Option<GizmoMaterials>,
    solid_material: Option<Handle<StandardMaterial>>,
    wire_material: Option<Handle<StandardMaterial>>,
    /// Animation graph and clip index per animated model path.
    marker_animations: HashMap<String, (Handle<AnimationGraph>, AnimationNodeIndex)>,
    /// Unit primitive meshes shared by all shape objects.
    shape_meshes: HashMap<EditorShapeKind, Handle<Mesh>>,
    /// Materials for shape objects, keyed by colour + texture path.
    building_materials: HashMap<([u8; 4], Option<String>), Handle<StandardMaterial>>,
    /// Loaded image textures used by shape objects.
    building_textures: HashMap<String, Handle<Image>>,
    /// Warm glTF scene handles for Play mode (idle + animated files), so
    /// swapping a unit between them does not wait for a cold asset load.
    play_scene_assets: HashMap<String, Handle<WorldAsset>>,
    /// Hidden scene roots kept for reuse when a unit switches between idle and
    /// animated state, keyed by model path. Bounded to avoid keeping too much
    /// duplicate instance data alive.
    idle_pool: HashMap<String, Vec<Entity>>,
    animated_pool: HashMap<String, Vec<Entity>>,
    /// Active Play root per editor object key, with its current scene mode
    /// (`true` = animated). Tracked explicitly so swaps never depend on the
    /// timing of deferred commands.
    play_roots: HashMap<u64, (Entity, bool)>,
    floor_image: Option<Handle<Image>>,
    floor_material: Option<Handle<StandardMaterial>>,
    /// Soft radial sprite shared by all particle puffs.
    particle_texture: Option<Handle<Image>>,
    /// Unit quad the particle puffs are drawn with.
    particle_mesh: Option<Handle<Mesh>>,
    /// Particle materials keyed by effect kind, colour shade, and quantised
    /// opacity step.
    particle_materials: HashMap<(EditorParticleKind, usize, u8), Handle<StandardMaterial>>,
}

struct GizmoMeshes {
    shaft_x: Handle<Mesh>,
    shaft_y: Handle<Mesh>,
    shaft_z: Handle<Mesh>,
    tip: Handle<Mesh>,
    center: Handle<Mesh>,
    ring: Handle<Mesh>,
}

struct GizmoMaterials {
    x: Handle<StandardMaterial>,
    y: Handle<StandardMaterial>,
    z: Handle<StandardMaterial>,
    uniform: Handle<StandardMaterial>,
    highlight: Handle<StandardMaterial>,
}

#[derive(Component)]
pub(crate) struct EditorGizmoRoot;

/// Links a Play move gizmo to the runtime unit it follows.
#[derive(Component)]
pub(crate) struct EditorPlayGizmoKey(pub u64);

/// Requests a one-time rescale of a marker mesh once its asset has loaded.
#[derive(Component)]
pub(crate) struct MarkerNormalize {
    height: f32,
}

#[derive(Component)]
pub(crate) struct MarkerNormalized;

/// Shading requested for the glTF scene root of a marker.
#[derive(Component, Clone, Copy)]
pub(crate) struct MarkerShading(pub EditorModelShading);

/// Marks a scene mesh whose material has already been overridden.
#[derive(Component)]
pub(crate) struct ShadingApplied;

/// Links a rendered marker root to its editor object key.
#[derive(Component)]
pub(crate) struct EditorPlayKey(pub u64);

/// Play-mode animation state for an animated marker root. The `AnimationPlayer`
/// is created by the glTF scene instance, so it is looked up once and cached.
#[derive(Component)]
pub(crate) struct EditorPlayAnimation {
    graph: Handle<AnimationGraph>,
    index: AnimationNodeIndex,
    player: Option<Entity>,
}

/// Root of one particle effect. Its transform keeps the geographic anchor and
/// the authored altitude; the children are billboarded puffs.
#[derive(Component)]
pub(crate) struct ParticleEffectRoot;

/// One animated puff/droplet inside a [`ParticleEffectRoot`]. All motion is
/// derived from `elapsed` and the stored phase, so no per-frame state has to be
/// written back.
#[derive(Component)]
pub(crate) struct ParticleElement {
    kind: EditorParticleKind,
    /// Offset within the cycle, 0..1.
    phase: f32,
    lifetime: f32,
    angle: f32,
    /// Horizontal swirl radius (fire/smoke) or outward speed (water).
    spread: f32,
    size: f32,
    /// Rise height for fire/smoke or peak height for water.
    height: f32,
    /// Sideways wind drift for smoke.
    drift: f32,
    /// Animation speed; water uses it to scale gravity as well.
    speed: f32,
}

/// Particle sizes are authored for a close-up view. Scaling the effect with the
/// map zoom keeps a flame or plume readable at every zoom level.
const PARTICLE_REFERENCE_MPP: f32 = 0.35;
const PARTICLE_GRAVITY: f32 = 9.8;

/// Upper bound for pooled scene roots per model path and for the whole pool.
const PLAY_POOL_PER_MODEL: usize = 128;
const PLAY_POOL_TOTAL: usize = 512;
/// Maximum number of new scene instances per frame. Releases still happen
/// immediately; only spawns are spread over frames so a large group changing
/// state at once does not cause a single-frame hitch.
const PLAY_SPAWN_BUDGET: usize = 64;

impl EditorRenderCache {
    fn pooled_roots(&self) -> usize {
        self.idle_pool.values().map(Vec::len).sum::<usize>()
            + self.animated_pool.values().map(Vec::len).sum::<usize>()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sync_editor_scene(
    mut commands: Commands,
    scene: Res<EditorScene>,
    mode: Res<EditorMode>,
    mut cache: ResMut<EditorRenderCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    camera: Res<MapCameraResource>,
) {
    if scene.revision == cache.revision {
        return;
    }
    cache.revision = scene.revision;

    for entity in cache.entities.drain(..) {
        commands.entity(entity).despawn();
    }
    // Pooled roots were part of `cache.entities`, so they were just despawned.
    cache.idle_pool.clear();
    cache.animated_pool.clear();
    cache.play_roots.clear();
    if let Some(entity) = cache.gizmo.take() {
        commands.entity(entity).despawn();
    }
    if let Some(entity) = cache.grid.take() {
        commands.entity(entity).despawn();
    }
    if let Some(entity) = cache.floor.take() {
        commands.entity(entity).despawn();
    }

    if !mode.active {
        return;
    }

    if let Some(grid) = &scene.grid {
        spawn_grid(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut images,
            &mut cache,
            grid,
        );
    }

    let ground_scale = camera.0.mercator_ground_scale();
    for object in &scene.objects {
        spawn_object(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut images,
            &asset_server,
            &mut cache,
            object,
            ground_scale,
            scene.model_shading,
        );
    }

    if let Some(gizmo) = &scene.gizmo {
        spawn_gizmo(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut cache,
            gizmo,
            camera.0.meters_per_pixel() as f32,
        );
    }
}

/// Tags the gizmo and every child it spawns with the overlay render layer so
/// the dedicated gizmo camera draws them after (and on top of) the map scene.
pub(crate) fn tag_gizmo_parts(
    mut commands: Commands,
    roots: Query<Entity, With<EditorGizmoRoot>>,
    children: Query<&Children>,
    untagged: Query<Entity, Without<RenderLayers>>,
) {
    for root in &roots {
        for entity in std::iter::once(root).chain(children.iter_descendants(root)) {
            if untagged.contains(entity) {
                commands
                    .entity(entity)
                    .insert(RenderLayers::layer(crate::map::GIZMO_RENDER_LAYER));
            }
        }
    }
}

/// Rescales a marker mesh once the glTF primitive is loaded. The asset library
/// stores the export as-is, so the model's own units are unknown; matching a
/// target height keeps newly added markers visible at a comparable size.
pub(crate) fn normalize_marker_meshes(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    markers: Query<(Entity, &Mesh3d, &Transform, &MarkerNormalize), Without<MarkerNormalized>>,
) {
    for (entity, mesh3d, transform, normalize) in &markers {
        let Some(mesh) = meshes.get(&mesh3d.0) else {
            continue;
        };
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            commands.entity(entity).insert(MarkerNormalized);
            continue;
        };
        let (min, max) = positions.iter().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(min, max), position| (min.min(position[1]), max.max(position[1])),
        );
        let height = max - min;
        if !height.is_finite() || height <= 1.0e-4 {
            commands.entity(entity).insert(MarkerNormalized);
            continue;
        }
        let factor = normalize.height / height;
        commands.entity(entity).insert((
            transform.with_scale(transform.scale * factor),
            MarkerNormalized,
        ));
    }
}

/// Loads the first primitive of a marker model. Draco-compressed files must
/// skip glTF validation, same as the asset preview.
fn load_marker_mesh(asset_server: &AssetServer, model: &str, draco: bool) -> Handle<Mesh> {
    let path = GltfAssetLabel::Primitive {
        mesh: 0,
        primitive: 0,
    }
    .from_asset(model.to_owned());
    if draco {
        asset_server
            .load_builder()
            .with_settings(|settings: &mut GltfLoaderSettings| settings.validate = false)
            .load(path)
    } else {
        asset_server.load(path)
    }
}

/// Loads the whole glTF scene so its own materials and textures are used.
fn load_marker_scene(asset_server: &AssetServer, model: &str, draco: bool) -> Handle<WorldAsset> {
    let path = GltfAssetLabel::Scene(0).from_asset(model.to_owned());
    if draco {
        asset_server
            .load_builder()
            .with_settings(|settings: &mut GltfLoaderSettings| settings.validate = false)
            .load(path)
    } else {
        asset_server.load(path)
    }
}

/// Loads the first animation clip of a play-mode marker model.
fn load_marker_animation(
    asset_server: &AssetServer,
    model: &str,
    draco: bool,
) -> Handle<AnimationClip> {
    let path = GltfAssetLabel::Animation(0).from_asset(model.to_owned());
    if draco {
        asset_server
            .load_builder()
            .with_settings(|settings: &mut GltfLoaderSettings| settings.validate = false)
            .load(path)
    } else {
        asset_server.load(path)
    }
}

/// Loads (or reuses) a Play-mode scene handle. Keeping the handle alive means
/// both the idle and animated files stay warm while Play is active, so swapping
/// a unit between them does not trigger a cold asset load.
fn play_scene_handle(
    asset_server: &AssetServer,
    cache: &mut EditorRenderCache,
    model: &str,
    draco: bool,
) -> Handle<WorldAsset> {
    cache
        .play_scene_assets
        .entry(model.to_owned())
        .or_insert_with(|| load_marker_scene(asset_server, model, draco))
        .clone()
}

/// Swaps each animated unit between its idle and animated glTF scene when the
/// moving state changes. Only one scene instance is active per unit; the
/// previous root is kept in a bounded pool and reused instead of being
/// respawned, which avoids instantiation hitches for repeated moves.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_editor_play_scenes(
    mut commands: Commands,
    frame: Res<EditorPlayFrame>,
    scene: Res<EditorScene>,
    mut cache: ResMut<EditorRenderCache>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    asset_server: Res<AssetServer>,
    animations: Query<&EditorPlayAnimation>,
    mut players: Query<&mut AnimationPlayer>,
) {
    if !frame.active {
        return;
    }
    let units = frame
        .units
        .iter()
        .map(|unit| (unit.key, unit))
        .collect::<HashMap<_, _>>();
    let mut spawned = 0usize;
    for object in &scene.objects {
        let EditorSceneKind::Marker {
            model,
            base_scale,
            draco,
            play_idle: Some(idle),
            scale,
            altitude,
            ..
        } = &object.kind
        else {
            continue;
        };
        let Some(unit) = units.get(&object.key) else {
            continue;
        };
        let Some(&(entity, animated)) = cache.play_roots.get(&object.key) else {
            continue;
        };
        if animated == unit.moving {
            continue;
        }
        let Some(anchor) = GeoAnchor::new(GeoPoint::new(unit.lon, unit.lat)) else {
            continue;
        };
        let transform = Transform::from_xyz(0.0, *altitude, 0.0)
            .with_rotation(Quat::from_rotation_y(
                std::f32::consts::PI - unit.heading_deg.to_radians(),
            ))
            .with_scale(Vec3::new(scale[0], scale[1], scale[2]) * *base_scale);
        let (path, path_draco) = if unit.moving {
            (model.as_str(), *draco)
        } else {
            (idle.model.as_str(), idle.draco)
        };
        let pooled_available = if unit.moving {
            cache
                .animated_pool
                .get(path)
                .is_some_and(|pool| !pool.is_empty())
        } else {
            cache
                .idle_pool
                .get(path)
                .is_some_and(|pool| !pool.is_empty())
        };
        if !pooled_available && spawned >= PLAY_SPAWN_BUDGET {
            // Try again next frame; the unit keeps its current scene meanwhile.
            continue;
        }

        // Return the old root to its mode-specific pool. Over the cap it is
        // despawned so pooled instance data stays bounded.
        let old_model = if animated {
            model.clone()
        } else {
            idle.model.clone()
        };
        let player = animations
            .get(entity)
            .ok()
            .and_then(|animation| animation.player);
        if let Some(player) = player
            && let Ok(mut player) = players.get_mut(player)
        {
            player.stop_all();
        }
        commands
            .entity(entity)
            .remove::<(EditorPlayKey, EditorPlayAnimation)>();
        commands.entity(entity).insert(Visibility::Hidden);
        let per_model = if animated {
            cache.animated_pool.get(&old_model).map_or(0, Vec::len)
        } else {
            cache.idle_pool.get(&old_model).map_or(0, Vec::len)
        };
        if per_model >= PLAY_POOL_PER_MODEL || cache.pooled_roots() >= PLAY_POOL_TOTAL {
            commands.entity(entity).despawn();
            cache.entities.retain(|candidate| *candidate != entity);
        } else if animated {
            cache
                .animated_pool
                .entry(old_model)
                .or_default()
                .push(entity);
        } else {
            cache.idle_pool.entry(old_model).or_default().push(entity);
        }

        let animation = if unit.moving {
            Some(
                cache
                    .marker_animations
                    .entry(model.clone())
                    .or_insert_with(|| {
                        let clip = load_marker_animation(&asset_server, model, *draco);
                        let (graph, index) = AnimationGraph::from_clip(clip);
                        (graphs.add(graph), index)
                    })
                    .clone(),
            )
        } else {
            None
        };
        let pooled = if unit.moving {
            cache.animated_pool.get_mut(path).and_then(Vec::pop)
        } else {
            cache.idle_pool.get_mut(path).and_then(Vec::pop)
        };
        if let Some(pooled) = pooled {
            commands.entity(pooled).insert((
                transform,
                anchor,
                Name::new(object.label.clone()),
                EditorPlayKey(object.key),
                Visibility::Visible,
            ));
            if let Some((graph, index)) = animation {
                commands.entity(pooled).insert(EditorPlayAnimation {
                    graph,
                    index,
                    player: None,
                });
            }
            cache.play_roots.insert(object.key, (pooled, unit.moving));
            continue;
        }
        let handle = play_scene_handle(&asset_server, &mut cache, path, path_draco);
        let mut spawn = commands.spawn((
            WorldAssetRoot(handle),
            transform,
            anchor,
            Name::new(object.label.clone()),
            EditorPlayKey(object.key),
            Visibility::Visible,
        ));
        if let Some((graph, index)) = animation {
            spawn.insert(EditorPlayAnimation {
                graph,
                index,
                player: None,
            });
        }
        let new_entity = spawn.id();
        spawned += 1;
        cache
            .play_roots
            .insert(object.key, (new_entity, unit.moving));
        cache.entities.push(new_entity);
    }
}

/// Keeps the Play move gizmo on its runtime unit while the gizmo is dragged.
/// The editor scene is not rebuilt for every drag frame, so the anchor has to
/// be refreshed here.
pub(crate) fn follow_play_gizmo(
    frame: Res<EditorPlayFrame>,
    mut gizmos: Query<(&EditorPlayGizmoKey, &mut GeoAnchor, &mut Transform)>,
) {
    if !frame.active {
        return;
    }
    for (key, mut anchor, mut transform) in &mut gizmos {
        let Some(unit) = frame.units.iter().find(|unit| unit.key == key.0) else {
            continue;
        };
        if let Ok(mercator) = GeoPoint::new(unit.lon, unit.lat).mercator()
            && anchor.mercator != mercator
        {
            anchor.mercator = mercator;
        }
        transform.translation.y = unit.altitude;
    }
}

/// Applies the RTS play positions every frame without rebuilding the scene.
/// Every moving unit turns toward its travel direction; only animated units
/// swap to the walk clip. Altitude is applied to the local transform so the
/// Play gizmo can lift a `Limited` asset.
pub(crate) fn apply_editor_play_frame(
    frame: Res<EditorPlayFrame>,
    mut markers: Query<(&EditorPlayKey, &mut GeoAnchor, &mut Transform)>,
) {
    if !frame.active {
        return;
    }
    let units = frame
        .units
        .iter()
        .map(|unit| (unit.key, unit))
        .collect::<HashMap<_, _>>();
    for (key, mut anchor, mut transform) in &mut markers {
        let Some(unit) = units.get(&key.0) else {
            continue;
        };
        if let Ok(mercator) = GeoPoint::new(unit.lon, unit.lat).mercator()
            && anchor.mercator != mercator
        {
            anchor.mercator = mercator;
        }
        transform.translation.y = unit.altitude;
        // Static movable units have no walk animation, but they must still
        // face the direction they travel instead of sliding sideways.
        if unit.moving {
            transform.rotation =
                Quat::from_rotation_y(std::f32::consts::PI - unit.heading_deg.to_radians());
        }
    }
}

/// Ensures every animated play root has a live `AnimationPlayer` running its
/// clip. The player belongs to the glTF scene instance and appears
/// asynchronously (and can be replaced when a scene is re-instanced), so this
/// runs every frame and re-discovers/restarts as needed. Idle units have no
/// animated scene at all, so no hidden animation keeps running.
pub(crate) fn sync_editor_play_animations(
    mut commands: Commands,
    mut roots: Query<(Entity, &mut EditorPlayAnimation)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    handles: Query<&AnimationGraphHandle>,
) {
    for (root, mut animation) in &mut roots {
        // The scene instance may replace the player entity (asset reload), so
        // do not trust a cached entity that is no longer valid.
        if animation
            .player
            .is_some_and(|player| players.get_mut(player).is_err())
        {
            animation.player = None;
        }
        if animation.player.is_none() {
            animation.player = std::iter::once(root)
                .chain(children.iter_descendants(root))
                .find(|entity| players.contains(*entity));
        }
        let Some(player_entity) = animation.player else {
            continue;
        };
        let Ok(mut player) = players.get_mut(player_entity) else {
            animation.player = None;
            continue;
        };
        // Retry every frame until the clip is actually running. This covers the
        // first spawn where the graph handle and the player become ready on
        // different frames.
        if handles.get(player_entity).is_err() || !player.is_playing_animation(animation.index) {
            commands
                .entity(player_entity)
                .insert(AnimationGraphHandle(animation.graph.clone()));
            player.play(animation.index).repeat();
        }
    }
}

/// Applies the editor's viewport shading to imported glTF models. The scene is
/// instantiated asynchronously, so this runs every frame and marks the meshes it
/// has already overridden.
pub(crate) fn apply_marker_shading(
    mut commands: Commands,
    mut cache: ResMut<EditorRenderCache>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    roots: Query<(Entity, &MarkerShading)>,
    children: Query<&Children>,
    parts: Query<&MeshMaterial3d<StandardMaterial>, Without<ShadingApplied>>,
) {
    for (root, shading) in &roots {
        let wireframe = match shading.0 {
            EditorModelShading::Textured => continue,
            EditorModelShading::Solid => false,
            EditorModelShading::Wireframe => true,
        };
        let material = if wireframe {
            cache
                .wire_material
                .get_or_insert_with(|| materials.add(wireframe_material()))
                .clone()
        } else {
            cache
                .solid_material
                .get_or_insert_with(|| materials.add(solid_material()))
                .clone()
        };
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            let Ok(list) = children.get(entity) else {
                continue;
            };
            for child in list.iter() {
                stack.push(child);
                if parts.get(child).is_err() {
                    continue;
                }
                let mut part = commands.entity(child);
                part.insert((MeshMaterial3d(material.clone()), ShadingApplied));
                if wireframe {
                    part.insert(Wireframe);
                }
            }
        }
    }
}

fn solid_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(0.72, 0.76, 0.82),
        perceptual_roughness: 0.75,
        metallic: 0.0,
        ..default()
    }
}

fn wireframe_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(0.05, 0.06, 0.09, 0.18),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_object(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    asset_server: &AssetServer,
    cache: &mut EditorRenderCache,
    object: &EditorSceneObject,
    ground_scale: f64,
    shading: EditorModelShading,
) {
    let Some(anchor) = GeoAnchor::new(GeoPoint::new(object.lon, object.lat)) else {
        return;
    };
    let name = Name::new(object.label.clone());

    match &object.kind {
        EditorSceneKind::Marker {
            model,
            base_scale,
            normalize_height,
            draco,
            scene_asset,
            play_idle,
            rotation_deg,
            scale,
            altitude,
            color,
        } => {
            let rotation = Quat::from_euler(
                EulerRot::YXZ,
                rotation_deg[1].to_radians(),
                rotation_deg[0].to_radians(),
                rotation_deg[2].to_radians(),
            );
            let scale = Vec3::new(scale[0], scale[1], scale[2]) * *base_scale;
            if *scene_asset {
                // Keep the glTF's own materials and textures.
                let transform = Transform::from_xyz(0.0, *altitude, 0.0)
                    .with_rotation(rotation)
                    .with_scale(scale);
                if let Some(idle) = play_idle {
                    // Play starts with the idle scene only. The animated scene is
                    // spawned on demand while the unit moves, and the idle scene
                    // is restored when it stops (one instance per unit).
                    let _ = play_scene_handle(asset_server, cache, model, *draco);
                    let handle = play_scene_handle(asset_server, cache, &idle.model, idle.draco);
                    let entity = commands
                        .spawn((
                            WorldAssetRoot(handle),
                            transform,
                            anchor,
                            name,
                            EditorPlayKey(object.key),
                            Visibility::Visible,
                            MarkerShading(shading),
                        ))
                        .id();
                    cache.play_roots.insert(object.key, (entity, false));
                    cache.entities.push(entity);
                    return;
                }
                let handle = load_marker_scene(asset_server, model, *draco);
                let entity = commands
                    .spawn((
                        WorldAssetRoot(handle),
                        transform,
                        anchor,
                        name,
                        EditorPlayKey(object.key),
                        MarkerShading(shading),
                    ))
                    .id();
                cache.entities.push(entity);
                return;
            }
            let mesh = cache
                .marker_meshes
                .entry(model.clone())
                .or_insert_with(|| load_marker_mesh(asset_server, model, *draco))
                .clone();
            let material = cache
                .marker_materials
                .entry(color_key(*color))
                .or_insert_with(|| materials.add(lit_material(*color)))
                .clone();
            let mut spawn = commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, *altitude, 0.0)
                    .with_rotation(rotation)
                    .with_scale(scale),
                anchor,
                name,
                EditorPlayKey(object.key),
            ));
            if let Some(height) = normalize_height {
                spawn.insert(MarkerNormalize { height: *height });
            }
            let entity = spawn.id();
            cache.entities.push(entity);
        }
        EditorSceneKind::Line {
            points,
            altitudes,
            width,
            color,
            alpha,
        } => {
            let Some(first) = points.first() else {
                return;
            };
            let anchor_point = GeoPoint::new(first[0], first[1]);
            let local = local_points(points, anchor_point, ground_scale);
            if local.len() < 2 {
                return;
            }
            let Some(ribbon) = mesh::ribbon_mesh(&local, altitudes, *width, 0.6) else {
                return;
            };
            let mesh = meshes.add(ribbon);
            let material = materials.add(unlit_material(*color, *alpha));
            let entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::default(),
                    anchor,
                    name,
                ))
                .id();
            cache.entities.push(entity);
        }
        EditorSceneKind::Building {
            shape,
            points,
            height,
            color,
            rotation_deg,
            scale,
            altitude,
            texture,
        } => {
            let rotation = Quat::from_euler(
                EulerRot::YXZ,
                rotation_deg[1].to_radians(),
                rotation_deg[0].to_radians(),
                rotation_deg[2].to_radians(),
            );
            let mesh = match shape {
                EditorShapeKind::Custom => {
                    if points.is_empty() {
                        return;
                    }
                    let origin = GeoPoint::new(object.lon, object.lat);
                    let local = local_points(points, origin, ground_scale);
                    if local.len() < 3 {
                        return;
                    }
                    meshes.add(mesh::prism_mesh(&local, *height))
                }
                _ => cache
                    .shape_meshes
                    .entry(*shape)
                    .or_insert_with(|| {
                        meshes.add(match shape {
                            EditorShapeKind::Cylinder => mesh::shape_cylinder_mesh(),
                            EditorShapeKind::Sphere => mesh::shape_sphere_mesh(),
                            EditorShapeKind::Cone => mesh::shape_cone_mesh(),
                            EditorShapeKind::Ground => mesh::shape_ground_mesh(),
                            _ => mesh::shape_box_mesh(),
                        })
                    })
                    .clone(),
            };
            let texture_handle = texture.as_ref().map(|path| {
                cache
                    .building_textures
                    .entry(path.clone())
                    .or_insert_with(|| asset_server.load(path.clone()))
                    .clone()
            });
            let material_key = (color_key(*color), texture.clone());
            let material = cache
                .building_materials
                .entry(material_key)
                .or_insert_with(|| {
                    // A texture is modulated by base colour, so keep it white
                    // when a texture is active.
                    let mut material = if texture.is_some() {
                        unlit_material([1.0, 1.0, 1.0, 1.0], 1.0)
                    } else {
                        unlit_material(*color, 0.9)
                    };
                    material.base_color_texture = texture_handle;
                    materials.add(material)
                })
                .clone();
            // A plane at altitude 0 would z-fight with the map tiles (also at
            // y = 0). Lift it slightly so its texture stays visible.
            let plane_lift = if *shape == EditorShapeKind::Ground {
                0.02
            } else {
                0.0
            };
            let entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, *altitude + plane_lift, 0.0)
                        .with_rotation(rotation)
                        .with_scale(Vec3::new(scale[0], scale[1], scale[2])),
                    anchor,
                    name,
                ))
                .id();
            cache.entities.push(entity);
        }
        // Text is rendered as a screen-space label overlay by the editor UI;
        // there is no 3D placeholder mesh anymore.
        EditorSceneKind::Text { .. } => {}
        EditorSceneKind::Particle {
            kind,
            altitude,
            settings,
        } => {
            spawn_particle_effect(
                commands, meshes, materials, images, cache, object.key, *kind, *altitude,
                *settings, anchor, name,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_particle_effect(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    cache: &mut EditorRenderCache,
    key: u64,
    kind: EditorParticleKind,
    altitude: f32,
    settings: EditorParticleSettings,
    anchor: GeoAnchor,
    name: Name,
) {
    let settings = settings.clamped();
    let texture = cache
        .particle_texture
        .get_or_insert_with(|| images.add(particle_texture()))
        .clone();
    let quad = cache
        .particle_mesh
        .get_or_insert_with(|| meshes.add(mesh::billboard_quad_mesh()))
        .clone();

    let opacity_step = ((settings.opacity * 10.0).round() as u8).max(1);
    let shade_count = match kind {
        EditorParticleKind::Fire => 3,
        EditorParticleKind::Smoke | EditorParticleKind::Water => 2,
    };
    let mut shades = Vec::with_capacity(shade_count);
    for shade in 0..shade_count {
        let handle = cache
            .particle_materials
            .entry((kind, shade, opacity_step))
            .or_insert_with(|| {
                materials.add(particle_puff_material(
                    &texture,
                    kind,
                    shade,
                    opacity_step as f32 / 10.0,
                ))
            })
            .clone();
        shades.push(handle);
    }

    let root = commands
        .spawn((
            Transform::from_xyz(0.0, altitude, 0.0),
            anchor,
            name,
            Visibility::Visible,
            ParticleEffectRoot,
        ))
        .id();
    cache.entities.push(root);

    let base_count = match kind {
        EditorParticleKind::Fire => 24,
        EditorParticleKind::Smoke => 18,
        EditorParticleKind::Water => 32,
    };
    let count = ((base_count as f32 * settings.density).round() as usize).clamp(4, 96);
    commands.entity(root).with_children(|parent| {
        for index in 0..count {
            let seed = (key as u32)
                .wrapping_mul(0x9E37_79B9)
                .wrapping_add(index as u32)
                .wrapping_mul(0x85EB_CA6B);
            let r1 = particle_noise(seed);
            let r2 = particle_noise(seed ^ 0x1234_5678);
            let r3 = particle_noise(seed ^ 0x9ABC_DEF0);
            let r4 = particle_noise(seed ^ 0x0F0F_0F0F);
            let r5 = particle_noise(seed ^ 0xF0F0_F0F0);
            let r6 = particle_noise(seed ^ 0xDEAD_BEEF);
            let element = match kind {
                EditorParticleKind::Fire => ParticleElement {
                    kind,
                    phase: r1,
                    lifetime: (0.9 + r2 * 0.9) / settings.speed,
                    angle: r3 * std::f32::consts::TAU,
                    spread: (0.35 + r4 * 0.75) * settings.spread,
                    size: (1.2 + r5 * 1.2) * settings.size,
                    height: (5.0 + r6 * 3.0) * settings.height,
                    drift: 0.0,
                    speed: settings.speed,
                },
                EditorParticleKind::Smoke => ParticleElement {
                    kind,
                    phase: r1,
                    lifetime: (3.0 + r2 * 2.2) / settings.speed,
                    angle: r3 * std::f32::consts::TAU,
                    spread: (1.0 + r4 * 1.6) * settings.spread,
                    size: (2.0 + r5 * 2.2) * settings.size,
                    height: (9.0 + r6 * 5.0) * settings.height,
                    drift: (0.35 + r4 * 0.55) * settings.spread,
                    speed: settings.speed,
                },
                EditorParticleKind::Water => {
                    let height = (5.0 + r6 * 3.0) * settings.height;
                    let gravity = PARTICLE_GRAVITY * settings.speed * settings.speed;
                    ParticleElement {
                        kind,
                        phase: r1,
                        // A projectile that reaches `height` needs this long to
                        // rise and fall back to the ground.
                        lifetime: 2.0 * (2.0 * height / gravity).sqrt(),
                        angle: r3 * std::f32::consts::TAU,
                        spread: (if index % 6 == 0 { 0.08 } else { 0.6 + r4 * 1.3 })
                            * settings.spread,
                        size: (0.35 + r5 * 0.45) * settings.size,
                        height,
                        drift: 0.0,
                        speed: settings.speed,
                    }
                }
            };
            parent.spawn((
                Mesh3d(quad.clone()),
                MeshMaterial3d(shades[index % shades.len()].clone()),
                Transform::default(),
                element,
            ));
        }
    });
}

fn particle_texture() -> Image {
    let size = 64u32;
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let dy = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let distance = (dx * dx + dy * dy).sqrt();
            let falloff = (1.0 - distance).clamp(0.0, 1.0);
            let alpha = (falloff * falloff * 255.0) as u8;
            let index = ((y * size + x) * 4) as usize;
            data[index] = 255;
            data[index + 1] = 255;
            data[index + 2] = 255;
            data[index + 3] = alpha;
        }
    }
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn particle_puff_material(
    texture: &Handle<Image>,
    kind: EditorParticleKind,
    shade: usize,
    opacity_scale: f32,
) -> StandardMaterial {
    let (color, alpha_mode, alpha) = match (kind, shade) {
        (EditorParticleKind::Fire, 0) => (Color::srgb(1.0, 0.18, 0.02), AlphaMode::Add, 0.85),
        (EditorParticleKind::Fire, 1) => (Color::srgb(1.0, 0.45, 0.05), AlphaMode::Add, 0.85),
        (EditorParticleKind::Fire, _) => (Color::srgb(1.0, 0.78, 0.22), AlphaMode::Add, 0.8),
        (EditorParticleKind::Smoke, 0) => (Color::srgb(0.45, 0.47, 0.5), AlphaMode::Blend, 0.34),
        (EditorParticleKind::Smoke, _) => (Color::srgb(0.68, 0.7, 0.73), AlphaMode::Blend, 0.3),
        (EditorParticleKind::Water, 0) => (Color::srgb(0.22, 0.58, 1.0), AlphaMode::Blend, 0.8),
        (EditorParticleKind::Water, _) => (Color::srgb(0.7, 0.92, 1.0), AlphaMode::Blend, 0.85),
    };
    StandardMaterial {
        base_color: color.with_alpha((alpha * opacity_scale).min(1.0)),
        base_color_texture: Some(texture.clone()),
        unlit: true,
        alpha_mode,
        // Billboards are rotated to face the camera; showing both faces keeps
        // them visible at grazing angles as well.
        cull_mode: None,
        ..default()
    }
}

fn particle_noise(seed: u32) -> f32 {
    let mut value = seed;
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^= value >> 16;
    (value & 0x00FF_FFFF) as f32 / 16_777_215.0
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let x = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Advances every particle effect. Positions are derived from the elapsed time
/// and the per-element phase, so the effect loops indefinitely without storing
/// per-frame state. Billboards copy the map camera rotation to always face the
/// viewer.
#[allow(clippy::type_complexity)]
pub(crate) fn animate_particle_effects(
    time: Res<Time>,
    mode: Res<EditorMode>,
    camera_state: Res<MapCameraResource>,
    cameras: Query<&GlobalTransform, With<MapCamera>>,
    mut roots: Query<
        (&mut Transform, &mut Visibility),
        (With<ParticleEffectRoot>, Without<ParticleElement>),
    >,
    mut elements: Query<(&ParticleElement, &mut Transform), Without<ParticleEffectRoot>>,
) {
    let animate = mode.preview || mode.particles_animate;
    if !animate {
        for (_, mut visibility) in &mut roots {
            *visibility = Visibility::Hidden;
        }
        return;
    }
    let Ok(camera) = cameras.single() else {
        return;
    };
    let camera_rotation = camera.rotation();
    let elapsed = time.elapsed_secs();
    let zoom_scale =
        (camera_state.0.meters_per_pixel() as f32 / PARTICLE_REFERENCE_MPP).clamp(1.0, 24.0);
    for (mut transform, mut visibility) in &mut roots {
        *visibility = Visibility::Visible;
        transform.scale = Vec3::splat(zoom_scale);
    }
    for (element, mut transform) in &mut elements {
        let t = (elapsed / element.lifetime + element.phase).fract();
        let (translation, size) = particle_pose(element, t);
        transform.translation = translation;
        transform.rotation = camera_rotation;
        transform.scale = Vec3::splat(size);
    }
}

fn particle_pose(element: &ParticleElement, t: f32) -> (Vec3, f32) {
    match element.kind {
        EditorParticleKind::Fire => {
            let fade = 1.0 - smoothstep(0.62, 1.0, t);
            let angle = element.angle + t * 7.0;
            let sway = element.spread * t;
            let translation = Vec3::new(
                angle.cos() * sway,
                0.15 + t * element.height,
                angle.sin() * sway,
            );
            (translation, element.size * (1.0 - 0.35 * t) * fade)
        }
        EditorParticleKind::Smoke => {
            let fade = 1.0 - smoothstep(0.68, 1.0, t);
            let radius = element.spread * (0.3 + t);
            let drift = element.drift * t * element.height * 0.35;
            let translation = Vec3::new(
                element.angle.cos() * radius + drift,
                0.3 + t * element.height,
                element.angle.sin() * radius + drift * 0.3,
            );
            (translation, element.size * (0.55 + 1.5 * t) * fade)
        }
        EditorParticleKind::Water => {
            let duration = element.lifetime;
            let gravity = PARTICLE_GRAVITY * element.speed * element.speed;
            let launch = 0.5 * gravity * duration;
            let flight = t * duration;
            let fade = 1.0 - smoothstep(0.92, 1.0, t);
            let translation = Vec3::new(
                element.angle.cos() * element.spread * flight,
                launch * flight - 0.5 * gravity * flight * flight + 0.12,
                element.angle.sin() * element.spread * flight,
            );
            (translation, element.size * (1.0 - 0.25 * t) * fade)
        }
    }
}

fn spawn_grid(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    cache: &mut EditorRenderCache,
    grid: &EditorGrid,
) {
    let Some(anchor) = GeoAnchor::new(GeoPoint::new(grid.center[0], grid.center[1])) else {
        return;
    };
    let grid_mesh = meshes.add(mesh::grid_mesh(
        grid.spacing_m.max(0.25),
        grid.extent_m.max(10.0),
        0.04,
    ));
    let grid_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.62, 0.85, 0.55),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let grid_entity = commands
        .spawn((
            Mesh3d(grid_mesh),
            MeshMaterial3d(grid_material),
            Transform::default(),
            anchor,
            Name::new("Indoor grid"),
        ))
        .id();
    cache.grid = Some(grid_entity);

    if !grid.floor_plan {
        return;
    }
    let image = cache
        .floor_image
        .get_or_insert_with(|| images.add(floor_plan_image()))
        .clone();
    let material = cache
        .floor_material
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::srgba(0.70, 0.88, 1.0, 0.9),
                base_color_texture: Some(image.clone()),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })
        })
        .clone();
    let floor = meshes.add(mesh::quad_mesh(
        grid.floor_size_m[0].max(10.0),
        grid.floor_size_m[1].max(10.0),
        0.08,
    ));
    let floor_entity = commands
        .spawn((
            Mesh3d(floor),
            MeshMaterial3d(material),
            Transform::default(),
            anchor,
            Name::new("Floor plan"),
        ))
        .id();
    cache.floor = Some(floor_entity);
}

fn spawn_gizmo(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut EditorRenderCache,
    gizmo: &EditorGizmo,
    meters_per_pixel: f32,
) {
    let Some(anchor) = GeoAnchor::new(GeoPoint::new(gizmo.lon, gizmo.lat)) else {
        return;
    };
    let handles = cache.gizmo_meshes.get_or_insert_with(|| GizmoMeshes {
        shaft_x: meshes.add(mesh::cuboid_mesh(Vec3::new(0.9, 0.06, 0.06))),
        shaft_y: meshes.add(mesh::cuboid_mesh(Vec3::new(0.06, 0.9, 0.06))),
        shaft_z: meshes.add(mesh::cuboid_mesh(Vec3::new(0.06, 0.06, 0.9))),
        tip: meshes.add(mesh::cone_mesh(0.12, 0.26)),
        center: meshes.add(mesh::cuboid_mesh(Vec3::splat(0.13))),
        ring: meshes.add(mesh::ring_mesh(1.0, 0.935, 96)),
    });
    let materials = cache.gizmo_materials.get_or_insert_with(|| GizmoMaterials {
        x: materials.add(gizmo_material([0.92, 0.24, 0.24, 1.0])),
        y: materials.add(gizmo_material([0.28, 0.92, 0.38, 1.0])),
        z: materials.add(gizmo_material([0.24, 0.52, 1.0, 1.0])),
        uniform: materials.add(gizmo_material([0.85, 0.88, 0.92, 1.0])),
        highlight: materials.add(gizmo_material([1.0, 0.86, 0.2, 1.0])),
    });

    let size = (meters_per_pixel * 90.0).clamp(1.0, 60_000.0);
    let root = commands
        .spawn((
            Transform::from_xyz(0.0, gizmo.altitude, 0.0).with_scale(Vec3::splat(size)),
            Visibility::default(),
            anchor,
            EditorGizmoRoot,
            Name::new("Transform gizmo"),
        ))
        .id();
    if let Some(key) = gizmo.play_key {
        commands.entity(root).insert(EditorPlayGizmoKey(key));
    }

    let active = gizmo.active_axis;
    let pick = |axis: EditorGizmoAxis, base: &Handle<StandardMaterial>| {
        if active == Some(axis) {
            materials.highlight.clone()
        } else {
            base.clone()
        }
    };

    let quarter_turn = std::f32::consts::FRAC_PI_2;
    commands.entity(root).with_children(|parent| {
        match gizmo.mode {
            EditorGizmoMode::Move => {
                parent.spawn((
                    Mesh3d(handles.shaft_x.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::X, &materials.x)),
                    Transform::from_xyz(0.45, 0.0, 0.0),
                ));
                parent.spawn((
                    Mesh3d(handles.tip.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::X, &materials.x)),
                    Transform::from_xyz(1.0, 0.0, 0.0)
                        .with_rotation(Quat::from_rotation_z(-quarter_turn)),
                ));
                if gizmo.show_y {
                    parent.spawn((
                        Mesh3d(handles.shaft_y.clone()),
                        MeshMaterial3d(pick(EditorGizmoAxis::Y, &materials.y)),
                        Transform::from_xyz(0.0, 0.45, 0.0),
                    ));
                    parent.spawn((
                        Mesh3d(handles.tip.clone()),
                        MeshMaterial3d(pick(EditorGizmoAxis::Y, &materials.y)),
                        Transform::from_xyz(0.0, 1.0, 0.0),
                    ));
                }
                parent.spawn((
                    Mesh3d(handles.shaft_z.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::Z, &materials.z)),
                    Transform::from_xyz(0.0, 0.0, 0.45),
                ));
                parent.spawn((
                    Mesh3d(handles.tip.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::Z, &materials.z)),
                    Transform::from_xyz(0.0, 0.0, 1.0)
                        .with_rotation(Quat::from_rotation_x(quarter_turn)),
                ));
                parent.spawn((
                    Mesh3d(handles.center.clone()),
                    MeshMaterial3d(materials.uniform.clone()),
                ));
            }
            EditorGizmoMode::Rotate => {
                // The ring mesh lies in the XZ plane, i.e. it rotates around Y.
                // Rotating it a quarter turn around Z / X moves it onto the YZ /
                // XY planes for the X and Z handles.
                let lift = Vec3::new(0.0, 0.02, 0.0);
                parent.spawn((
                    Mesh3d(handles.ring.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::RotateX, &materials.x)),
                    Transform::from_rotation(Quat::from_rotation_z(quarter_turn))
                        .with_translation(lift),
                ));
                parent.spawn((
                    Mesh3d(handles.ring.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::RotateY, &materials.y)),
                    Transform::from_translation(lift),
                ));
                parent.spawn((
                    Mesh3d(handles.ring.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::RotateZ, &materials.z)),
                    Transform::from_rotation(Quat::from_rotation_x(quarter_turn))
                        .with_translation(lift),
                ));
            }
            EditorGizmoMode::Scale => {
                parent.spawn((
                    Mesh3d(handles.shaft_x.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::X, &materials.x)),
                    Transform::from_xyz(0.42, 0.0, 0.0),
                ));
                parent.spawn((
                    Mesh3d(handles.center.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::X, &materials.x)),
                    Transform::from_xyz(0.9, 0.0, 0.0).with_scale(Vec3::splat(1.9)),
                ));
                parent.spawn((
                    Mesh3d(handles.shaft_y.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::Y, &materials.y)),
                    Transform::from_xyz(0.0, 0.42, 0.0),
                ));
                parent.spawn((
                    Mesh3d(handles.center.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::Y, &materials.y)),
                    Transform::from_xyz(0.0, 0.9, 0.0).with_scale(Vec3::splat(1.9)),
                ));
                parent.spawn((
                    Mesh3d(handles.shaft_z.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::Z, &materials.z)),
                    Transform::from_xyz(0.0, 0.0, 0.42),
                ));
                parent.spawn((
                    Mesh3d(handles.center.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::Z, &materials.z)),
                    Transform::from_xyz(0.0, 0.0, 0.9).with_scale(Vec3::splat(1.9)),
                ));
                // Center handle: scales every axis at once (Blender style).
                // A flat ring makes it easy to spot and grab even when the
                // model covers the gizmo origin.
                parent.spawn((
                    Mesh3d(handles.ring.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::Uniform, &materials.uniform)),
                    Transform::from_xyz(0.0, 0.02, 0.0).with_scale(Vec3::splat(0.3)),
                ));
                parent.spawn((
                    Mesh3d(handles.center.clone()),
                    MeshMaterial3d(pick(EditorGizmoAxis::Uniform, &materials.uniform)),
                    Transform::from_xyz(0.0, 0.02, 0.0).with_scale(Vec3::splat(1.6)),
                ));
            }
        }
    });
    cache.gizmo = Some(root);
}

pub fn update_gizmo_scale(
    camera: Res<MapCameraResource>,
    mut gizmos: Query<&mut Transform, With<EditorGizmoRoot>>,
) {
    let size = (camera.0.meters_per_pixel() as f32 * 90.0).clamp(1.0, 60_000.0);
    for mut transform in &mut gizmos {
        transform.scale = Vec3::splat(size);
    }
}

fn local_points(points: &[[f64; 2]], anchor: GeoPoint, scale: f64) -> Vec<[f32; 2]> {
    points
        .iter()
        .filter_map(|point| {
            let point = GeoPoint::new(point[0], point[1]);
            let offset = point.local_offset(anchor).ok()?;
            Some([
                (offset.east_meters * scale) as f32,
                (-offset.north_meters * scale) as f32,
            ])
        })
        .collect()
}

fn color_key(color: [f32; 4]) -> [u8; 4] {
    [
        (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn lit_material(color: [f32; 4]) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(color[0], color[1], color[2], color[3]),
        perceptual_roughness: 0.8,
        metallic: 0.0,
        ..default()
    }
}

fn unlit_material(color: [f32; 4], alpha: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(color[0], color[1], color[2], color[3] * alpha),
        unlit: true,
        alpha_mode: if color[3] * alpha < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        ..default()
    }
}

fn gizmo_material(color: [f32; 4]) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(color[0], color[1], color[2], color[3]),
        unlit: true,
        // Rotation rings are flat annuli; show both faces so a ring is visible
        // from any camera angle.
        cull_mode: None,
        ..default()
    }
}

fn floor_plan_image() -> Image {
    let size = 256u32;
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let major = x.is_multiple_of(64) || y.is_multiple_of(64);
            let grid = x.is_multiple_of(16) || y.is_multiple_of(16);
            let (r, g, b, a) = if major {
                (150, 210, 255, 210)
            } else if grid {
                (68, 118, 158, 130)
            } else {
                (16, 30, 46, 96)
            };
            let index = ((y * size + x) * 4) as usize;
            data[index] = r;
            data[index + 1] = g;
            data[index + 2] = b;
            data[index + 3] = a;
        }
    }
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

#[cfg(test)]
mod particle_system_tests {
    use super::*;
    use bevy::asset::{AssetApp, AssetPlugin};

    #[test]
    fn particle_effect_spawns_animated_elements() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .init_resource::<EditorRenderCache>()
            .insert_resource(MapCameraResource(twg_map::MapCameraState::default()))
            .insert_resource(EditorMode {
                active: true,
                preview: false,
                particles_animate: true,
            })
            .insert_resource(EditorScene {
                revision: 1,
                objects: vec![EditorSceneObject {
                    key: 7,
                    label: "Api".to_owned(),
                    lon: 115.225,
                    lat: -8.648,
                    selected: false,
                    kind: EditorSceneKind::Particle {
                        kind: EditorParticleKind::Fire,
                        altitude: 2.0,
                        settings: EditorParticleSettings::default(),
                    },
                }],
                ..Default::default()
            })
            .add_systems(
                Update,
                (sync_editor_scene, animate_particle_effects).chain(),
            );
        app.world_mut()
            .spawn((MapCamera, Transform::default(), GlobalTransform::default()));

        app.update();
        app.update();

        let (element_count, any_scaled) = {
            let world = app.world_mut();
            let mut elements = world.query::<(&ParticleElement, &Transform)>();
            let elements = elements.iter(world).collect::<Vec<_>>();
            (
                elements.len(),
                elements
                    .iter()
                    .any(|(_, transform)| transform.scale.x > 0.0),
            )
        };
        assert!(element_count > 0, "particle effect must spawn puffs");
        assert!(any_scaled);

        let visibility = |app: &mut App| {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&Visibility, With<ParticleEffectRoot>>();
            *query.single(world).expect("particle root")
        };
        assert_eq!(visibility(&mut app), Visibility::Visible);
        app.world_mut()
            .resource_mut::<EditorMode>()
            .particles_animate = false;
        app.update();
        assert_eq!(visibility(&mut app), Visibility::Hidden);
    }
}

#[cfg(test)]
mod particle_tests {
    use super::{EditorParticleKind, PARTICLE_GRAVITY, ParticleElement, particle_pose};

    fn element(kind: EditorParticleKind, height: f32, lifetime: f32) -> ParticleElement {
        ParticleElement {
            kind,
            phase: 0.0,
            lifetime,
            angle: 0.0,
            spread: 1.0,
            size: 1.0,
            height,
            drift: 0.0,
            speed: 1.0,
        }
    }

    #[test]
    fn fire_rises_and_fades_out() {
        let fire = element(EditorParticleKind::Fire, 6.0, 1.0);
        let (start, start_size) = particle_pose(&fire, 0.0);
        let (end, end_size) = particle_pose(&fire, 0.99);
        assert!(start.y < 0.5);
        assert!(end.y > 5.0);
        assert!(start_size > end_size);
        assert!(end_size < 0.05);
    }

    #[test]
    fn water_arc_returns_to_the_ground() {
        let height = 5.0;
        let lifetime = 2.0 * (2.0 * height / PARTICLE_GRAVITY).sqrt();
        let water = element(EditorParticleKind::Water, height, lifetime);
        let (start, _) = particle_pose(&water, 0.0);
        let (peak, _) = particle_pose(&water, 0.5);
        let (end, _) = particle_pose(&water, 0.999);
        assert!((start.y - 0.12).abs() < 1.0e-3);
        assert!((peak.y - (height + 0.12)).abs() < 0.05);
        assert!((end.y - 0.12).abs() < 0.05);
    }

    #[test]
    fn smoke_grows_while_it_rises() {
        let smoke = element(EditorParticleKind::Smoke, 12.0, 4.0);
        let (start, start_size) = particle_pose(&smoke, 0.0);
        let (middle, middle_size) = particle_pose(&smoke, 0.5);
        assert!(middle.y > start.y);
        assert!(middle_size > start_size);
    }
}

#[cfg(test)]
mod shape_tests {
    use super::EditorShapeKind;

    #[test]
    fn cone_replaces_the_old_circle_and_legacy_circle_stays_flat() {
        assert_eq!(EditorShapeKind::from_key("cone"), EditorShapeKind::Cone);
        assert_eq!(EditorShapeKind::from_key("kerucut"), EditorShapeKind::Cone);
        assert_eq!(EditorShapeKind::from_key("circle"), EditorShapeKind::Ground);
        assert_eq!(EditorShapeKind::from_key("plane"), EditorShapeKind::Ground);
        assert_eq!(EditorShapeKind::Cone.key(), "cone");
        assert_eq!(EditorShapeKind::Ground.key(), "ground");
    }
}
