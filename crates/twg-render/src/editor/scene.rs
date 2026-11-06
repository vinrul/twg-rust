use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::gltf::GltfAssetLabel;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use twg_map::GeoPoint;

use super::mesh;
use crate::map::{GeoAnchor, MapCameraResource};

/// Enables editing behaviour and tells map input to stop using left-drag for
/// panning. `preview` keeps the scene visible while disabling editor input.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct EditorMode {
    pub active: bool,
    pub preview: bool,
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
pub enum EditorSceneKind {
    Marker {
        model: String,
        base_scale: f32,
        rotation_deg: [f32; 3],
        scale: [f32; 3],
        altitude: f32,
        color: [f32; 4],
    },
    Line {
        points: Vec<[f64; 2]>,
        width: f32,
        color: [f32; 4],
        alpha: f32,
    },
    Building {
        points: Vec<[f64; 2]>,
        height: f32,
        color: [f32; 4],
    },
    Text {
        color: [f32; 4],
        font_size: f32,
    },
    Particle {
        color: [f32; 4],
        radius: f32,
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
    RotateY,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorGizmo {
    pub lon: f64,
    pub lat: f64,
    pub altitude: f32,
    pub active_axis: Option<EditorGizmoAxis>,
    pub show_rotate: bool,
    pub show_scale: bool,
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
    floor_image: Option<Handle<Image>>,
    floor_material: Option<Handle<StandardMaterial>>,
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
    highlight: Handle<StandardMaterial>,
}

#[derive(Component)]
pub(crate) struct EditorGizmoRoot;

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
            &asset_server,
            &mut cache,
            object,
            ground_scale,
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

fn spawn_object(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    cache: &mut EditorRenderCache,
    object: &EditorSceneObject,
    ground_scale: f64,
) {
    let Some(anchor) = GeoAnchor::new(GeoPoint::new(object.lon, object.lat)) else {
        return;
    };
    let name = Name::new(object.label.clone());

    match &object.kind {
        EditorSceneKind::Marker {
            model,
            base_scale,
            rotation_deg,
            scale,
            altitude,
            color,
        } => {
            let mesh = cache
                .marker_meshes
                .entry(model.clone())
                .or_insert_with(|| {
                    asset_server.load(
                        GltfAssetLabel::Primitive {
                            mesh: 0,
                            primitive: 0,
                        }
                        .from_asset(model.clone()),
                    )
                })
                .clone();
            let material = cache
                .marker_materials
                .entry(color_key(*color))
                .or_insert_with(|| materials.add(lit_material(*color)))
                .clone();
            let rotation = Quat::from_euler(
                EulerRot::YXZ,
                rotation_deg[1].to_radians(),
                rotation_deg[0].to_radians(),
                rotation_deg[2].to_radians(),
            );
            let scale = Vec3::new(scale[0], scale[1], scale[2]) * *base_scale;
            let entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, *altitude, 0.0)
                        .with_rotation(rotation)
                        .with_scale(scale),
                    anchor,
                    name,
                ))
                .id();
            cache.entities.push(entity);
        }
        EditorSceneKind::Line {
            points,
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
            let mesh = meshes.add(mesh::ribbon_mesh(&local, *width, 0.0));
            let material = materials.add(unlit_material(*color, *alpha));
            let entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, 0.6, 0.0),
                    anchor,
                    name,
                ))
                .id();
            cache.entities.push(entity);
        }
        EditorSceneKind::Building {
            points,
            height,
            color,
        } => {
            let Some(first) = points.first() else {
                return;
            };
            let anchor_point = GeoPoint::new(first[0], first[1]);
            let local = local_points(points, anchor_point, ground_scale);
            if local.len() < 3 {
                return;
            }
            let mesh = meshes.add(mesh::prism_mesh(&local, *height));
            let material = materials.add(unlit_material(*color, 0.78));
            let entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, 0.05, 0.0),
                    anchor,
                    name,
                ))
                .id();
            cache.entities.push(entity);
        }
        EditorSceneKind::Text { color, .. } => {
            let mesh = meshes.add(mesh::cuboid_mesh(Vec3::splat(1.4)));
            let material = materials.add(unlit_material(*color, 1.0));
            let entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, 2.0, 0.0).with_rotation(Quat::from_rotation_y(0.785)),
                    anchor,
                    name,
                ))
                .id();
            cache.entities.push(entity);
        }
        EditorSceneKind::Particle { color, radius } => {
            let mesh = meshes.add(mesh::sphere_mesh((*radius).max(0.4)));
            let material = materials.add(unlit_material(*color, 0.9));
            let entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, 1.6, 0.0),
                    anchor,
                    name,
                ))
                .id();
            cache.entities.push(entity);
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
        ring: meshes.add(mesh::circle_mesh(1.0, 72, 0.02)),
    });
    let materials = cache.gizmo_materials.get_or_insert_with(|| GizmoMaterials {
        x: materials.add(gizmo_material([0.92, 0.24, 0.24, 1.0])),
        y: materials.add(gizmo_material([0.28, 0.92, 0.38, 1.0])),
        z: materials.add(gizmo_material([0.24, 0.52, 1.0, 1.0])),
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

    let active = gizmo.active_axis;
    let pick = |axis: EditorGizmoAxis, base: &Handle<StandardMaterial>| {
        if active == Some(axis) {
            materials.highlight.clone()
        } else {
            base.clone()
        }
    };

    commands.entity(root).with_children(|parent| {
        parent.spawn((
            Mesh3d(handles.shaft_x.clone()),
            MeshMaterial3d(pick(EditorGizmoAxis::X, &materials.x)),
            Transform::from_xyz(0.45, 0.0, 0.0),
        ));
        parent.spawn((
            Mesh3d(handles.tip.clone()),
            MeshMaterial3d(pick(EditorGizmoAxis::X, &materials.x)),
            Transform::from_xyz(1.0, 0.0, 0.0)
                .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
        ));
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
        parent.spawn((
            Mesh3d(handles.shaft_z.clone()),
            MeshMaterial3d(pick(EditorGizmoAxis::Z, &materials.z)),
            Transform::from_xyz(0.0, 0.0, 0.45),
        ));
        parent.spawn((
            Mesh3d(handles.tip.clone()),
            MeshMaterial3d(pick(EditorGizmoAxis::Z, &materials.z)),
            Transform::from_xyz(0.0, 0.0, 1.0)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        ));
        parent.spawn((
            Mesh3d(handles.center.clone()),
            MeshMaterial3d(materials.highlight.clone()),
        ));
        if gizmo.show_rotate {
            parent.spawn((
                Mesh3d(handles.ring.clone()),
                MeshMaterial3d(pick(EditorGizmoAxis::RotateY, &materials.highlight)),
            ));
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
