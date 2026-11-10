use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use twg_map::MapCameraState;

use super::GeoAnchor;

#[derive(Component)]
pub struct DemoOverlay;

pub fn spawn_overlays(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let camera = MapCameraState::default();
    let center = camera.center;
    let Some(center_anchor) = GeoAnchor::new(center) else {
        return;
    };

    let line_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.1, 0.85, 1.0, 0.95),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let line_points = [
        (-70.0, -55.0),
        (-25.0, -55.0),
        (-25.0, -10.0),
        (35.0, -10.0),
        (35.0, 45.0),
        (75.0, 45.0),
    ]
    .map(|(east, north)| (east, -north));
    let local = line_points.iter().map(|&(x, z)| [x, z]).collect::<Vec<_>>();
    if let Some(mesh) = crate::editor::mesh::ribbon_mesh(&local, &[], 2.2, 0.25) {
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(line_material),
            Transform::from_xyz(0.0, 0.25, 0.0),
            center_anchor,
            DemoOverlay,
            Name::new("Demo route line"),
        ));
    }

    let building_center = camera.world_to_geo(120.0, -80.0).unwrap_or(center);
    if let Some(anchor) = GeoAnchor::new(building_center) {
        let footprint = [
            (-24.0, -18.0),
            (26.0, -18.0),
            (32.0, 4.0),
            (18.0, 24.0),
            (-22.0, 20.0),
        ];
        let building_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.95, 0.52, 0.16, 0.78),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            Mesh3d(meshes.add(prism_mesh(&footprint, 14.0))),
            MeshMaterial3d(building_material),
            Transform::default(),
            anchor,
            DemoOverlay,
            Name::new("Demo building extrusion"),
        ));
    }

    let floor_center = camera.world_to_geo(-105.0, -70.0).unwrap_or(center);
    if let Some(anchor) = GeoAnchor::new(floor_center) {
        let floor_image = images.add(floor_plan_image());
        let floor_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.75, 0.9, 1.0, 0.85),
            base_color_texture: Some(floor_image),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        commands.spawn((
            Mesh3d(meshes.add(quad_mesh(62.0, 42.0, 0.18))),
            MeshMaterial3d(floor_material),
            Transform::from_rotation(Quat::from_rotation_y(0.35)),
            anchor,
            DemoOverlay,
            Name::new("Demo floor plan"),
        ));
    }
}

fn prism_mesh(footprint: &[(f32, f32)], height: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let count = footprint.len() as u32;
    // Top and bottom centre vertices.
    positions.push([0.0, height, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);

    for &(x, z) in footprint {
        positions.push([x, height, z]);
        normals.push([0.0, 1.0, 0.0]);
        positions.push([x, 0.0, z]);
        normals.push([0.0, -1.0, 0.0]);
    }

    for index in 0..count {
        let next = (index + 1) % count;
        let top = 2 + index * 2;
        let bottom = top + 1;
        let next_top = 2 + next * 2;
        let next_bottom = next_top + 1;
        // Top and bottom caps.
        indices.extend_from_slice(&[0, top, next_top]);
        indices.extend_from_slice(&[1, next_bottom, bottom]);
        // Side quad.
        indices.extend_from_slice(&[bottom, next_bottom, next_top]);
        indices.extend_from_slice(&[bottom, next_top, top]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        VertexAttributeValues::Float32x3(normals),
    );
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn quad_mesh(width: f32, depth: f32, height: f32) -> Mesh {
    let half_w = width * 0.5;
    let half_d = depth * 0.5;
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(vec![
            [-half_w, height, -half_d],
            [half_w, height, -half_d],
            [half_w, height, half_d],
            [-half_w, height, half_d],
        ]),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        VertexAttributeValues::Float32x3(vec![
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        VertexAttributeValues::Float32x2(vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]),
    );
    mesh.insert_indices(Indices::U32(vec![0, 2, 1, 0, 3, 2]));
    mesh
}

fn floor_plan_image() -> Image {
    let size = 256u32;
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let grid = x.is_multiple_of(32) || y.is_multiple_of(32);
            let major = x.is_multiple_of(128) || y.is_multiple_of(128);
            let (r, g, b, a) = if major {
                (120, 200, 255, 220)
            } else if grid {
                (70, 130, 175, 170)
            } else {
                (18, 35, 55, 150)
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
