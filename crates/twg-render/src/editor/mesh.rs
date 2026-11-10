use bevy::asset::RenderAssetUsages;
use bevy::mesh::{ConeAnchor, Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
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
    let mut uvs = Vec::new();
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
    uvs.push([0.5, 0.5]);
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    for (index, &[x, z]) in footprint.iter().enumerate() {
        let u = index as f32 / count as f32;
        positions.push([x, height, z]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([u, 1.0]);
        positions.push([x, 0.0, z]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([u, 0.0]);
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, VertexAttributeValues::Float32x2(uvs));
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
    mesh.insert_indices(Indices::U32(vec![0, 2, 1, 0, 3, 2]));
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

/// Flat annulus in the XZ plane, used by the rotation gizmo. Triangles render
/// with the standard material pipeline, unlike line-list meshes.
pub fn ring_mesh(radius_outer: f32, radius_inner: f32, segments: u32) -> Mesh {
    let segments = segments.max(12);
    let inner = radius_inner.clamp(0.0, radius_outer);
    let mut positions = Vec::with_capacity((segments * 2) as usize);
    let mut normals = Vec::with_capacity((segments * 2) as usize);
    let mut indices = Vec::with_capacity((segments * 6) as usize);
    for index in 0..segments {
        let angle = index as f32 / segments as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        positions.push([cos * radius_outer, 0.0, sin * radius_outer]);
        positions.push([cos * inner, 0.0, sin * inner]);
        normals.push([0.0, 1.0, 0.0]);
        normals.push([0.0, 1.0, 0.0]);
    }
    for index in 0..segments {
        let next = (index + 1) % segments;
        let outer = index * 2;
        let inner_index = index * 2 + 1;
        let next_outer = next * 2;
        let next_inner = next * 2 + 1;
        indices.extend_from_slice(&[outer, inner_index, next_outer]);
        indices.extend_from_slice(&[inner_index, next_inner, next_outer]);
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

/// Unit primitive meshes for editor shape objects. All are anchored at the
/// centre of their footprint on the ground plane (y = 0), so scale and
/// rotation happen around the object's placement point.
pub fn shape_box_mesh() -> Mesh {
    cuboid_mesh(Vec3::ONE).translated_by(Vec3::Y * 0.5)
}

pub fn shape_cylinder_mesh() -> Mesh {
    Cylinder::new(0.5, 1.0)
        .mesh()
        .build()
        .translated_by(Vec3::Y * 0.5)
}

pub fn shape_sphere_mesh() -> Mesh {
    Sphere::new(0.5)
        .mesh()
        .uv(32, 18)
        .translated_by(Vec3::Y * 0.5)
}

/// Unit cone anchored at the centre of its base (y = 0).
pub fn shape_cone_mesh() -> Mesh {
    Cone::new(0.5, 1.0).mesh().anchor(ConeAnchor::Base).build()
}

/// Flat plane with UVs, so it can be given a texture.
pub fn shape_ground_mesh() -> Mesh {
    quad_mesh(1.0, 1.0, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_faces_up_so_it_is_not_backface_culled() {
        let mesh = quad_mesh(1.0, 1.0, 0.0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("quad has no position attribute");
        };
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("quad has no u32 indices");
        };
        let [a, b, c] = [indices[0], indices[1], indices[2]].map(|index| index as usize);
        let ab = Vec3::from(positions[b]) - Vec3::from(positions[a]);
        let ac = Vec3::from(positions[c]) - Vec3::from(positions[a]);
        assert!(ab.cross(ac).y > 0.0, "front face must point up");
    }
}
