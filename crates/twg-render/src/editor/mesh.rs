use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

/// Flat ribbon along a polyline.
pub fn ribbon_mesh(points: &[[f32; 2]], width: f32, height: f32) -> Mesh {
    let half = width * 0.5;
    let mut positions = Vec::with_capacity(points.len() * 2);
    let mut normals = Vec::with_capacity(points.len() * 2);
    let mut indices = Vec::new();

    for (index, &[x, z]) in points.iter().enumerate() {
        let (dx, dz) = if index + 1 < points.len() {
            let [nx, nz] = points[index + 1];
            (nx - x, nz - z)
        } else if index > 0 {
            let [px, pz] = points[index - 1];
            (x - px, z - pz)
        } else {
            (1.0, 0.0)
        };
        let length = (dx * dx + dz * dz).sqrt().max(0.001);
        let (nx, nz) = (-dz / length, dx / length);
        positions.push([x + nx * half, height, z + nz * half]);
        positions.push([x - nx * half, height, z - nz * half]);
        normals.push([0.0, 1.0, 0.0]);
        normals.push([0.0, 1.0, 0.0]);
        if index > 0 {
            let base = (index as u32 - 1) * 2;
            indices.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
        }
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

/// Extruded polygon with top/bottom caps and side walls.
pub fn prism_mesh(footprint: &[[f32; 2]], height: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let count = footprint.len() as u32;
    if count < 3 {
        return Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
    }

    positions.push([0.0, height, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);

    for &[x, z] in footprint {
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
        indices.extend_from_slice(&[0, top, next_top]);
        indices.extend_from_slice(&[1, next_bottom, bottom]);
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

pub fn quad_mesh(width: f32, depth: f32, height: f32) -> Mesh {
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
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    mesh
}

/// Ground grid centred on the local origin.
pub fn grid_mesh(spacing: f32, extent: f32, height: f32) -> Mesh {
    let spacing = spacing.max(0.1);
    let half = extent.max(spacing) * 0.5;
    let steps = (extent / spacing).ceil() as i32;
    let mut positions = Vec::new();
    let mut indices = Vec::new();

    for step in -steps..=steps {
        let offset = step as f32 * spacing;
        if offset.abs() > half {
            continue;
        }
        let base = positions.len() as u32;
        positions.push([-half, height, offset]);
        positions.push([half, height, offset]);
        indices.extend_from_slice(&[base, base + 1]);
        let base = positions.len() as u32;
        positions.push([offset, height, -half]);
        positions.push([offset, height, half]);
        indices.extend_from_slice(&[base, base + 1]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Circle outline used by the rotate gizmo.
pub fn circle_mesh(radius: f32, segments: u32, height: f32) -> Mesh {
    let segments = segments.max(8);
    let mut positions = Vec::with_capacity(segments as usize);
    let mut indices = Vec::new();
    for index in 0..segments {
        let angle = index as f32 / segments as f32 * std::f32::consts::TAU;
        positions.push([angle.cos() * radius, height, angle.sin() * radius]);
    }
    for index in 0..segments {
        let next = (index + 1) % segments;
        indices.extend_from_slice(&[index, next]);
    }
    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub fn cuboid_mesh(size: Vec3) -> Mesh {
    Cuboid::new(size.x, size.y, size.z).mesh().build()
}

pub fn cone_mesh(radius: f32, height: f32) -> Mesh {
    Cone::new(radius, height).mesh().build()
}

pub fn sphere_mesh(radius: f32) -> Mesh {
    Sphere::new(radius)
        .mesh()
        .ico(2)
        .unwrap_or_else(|_| Sphere::new(radius).mesh().build())
}
