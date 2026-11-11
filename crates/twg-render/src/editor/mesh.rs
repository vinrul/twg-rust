use bevy::asset::RenderAssetUsages;
use bevy::mesh::{ConeAnchor, Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

/// Flat ribbon along a polyline.
///
/// Consecutive duplicate points are dropped and interior corners use a miter
/// join, so the visual width stays constant along the whole line. Each point
/// can have its own height through `altitudes` (missing values are 0) and
/// `base_height` lifts the whole ribbon. Returns `None` when fewer than two
/// distinct points remain.
pub fn ribbon_mesh(
    points: &[[f32; 2]],
    altitudes: &[f32],
    width: f32,
    base_height: f32,
) -> Option<Mesh> {
    let mut cleaned: Vec<([f32; 2], f32)> = Vec::with_capacity(points.len());
    for (index, &point) in points.iter().enumerate() {
        if !point[0].is_finite() || !point[1].is_finite() {
            continue;
        }
        let altitude = altitudes
            .get(index)
            .copied()
            .filter(|value| value.is_finite())
            .unwrap_or(0.0);
        if let Some((last, _)) = cleaned.last() {
            let dx = point[0] - last[0];
            let dz = point[1] - last[1];
            if dx * dx + dz * dz < 0.000_1 {
                continue;
            }
        }
        cleaned.push((point, altitude));
    }
    if cleaned.len() < 2 {
        return None;
    }

    let half = width.max(0.001) * 0.5;
    let mut directions = Vec::with_capacity(cleaned.len() - 1);
    for pair in cleaned.windows(2) {
        let dx = pair[1].0[0] - pair[0].0[0];
        let dz = pair[1].0[1] - pair[0].0[1];
        let length = (dx * dx + dz * dz).sqrt().max(0.001);
        directions.push([dx / length, dz / length]);
    }

    let mut positions = Vec::with_capacity(cleaned.len() * 2);
    let mut normals = Vec::with_capacity(cleaned.len() * 2);
    for (index, (point, altitude)) in cleaned.iter().enumerate() {
        let incoming = index.checked_sub(1).map(|segment| directions[segment]);
        let outgoing = directions.get(index).copied();
        let offset = match (incoming, outgoing) {
            (Some(incoming), Some(outgoing)) => {
                let left_in = [-incoming[1], incoming[0]];
                let left_out = [-outgoing[1], outgoing[0]];
                let mut miter = [left_in[0] + left_out[0], left_in[1] + left_out[1]];
                let length = (miter[0] * miter[0] + miter[1] * miter[1]).sqrt();
                if length < 0.001 {
                    [left_out[0] * half, left_out[1] * half]
                } else {
                    miter = [miter[0] / length, miter[1] / length];
                    // Cap the miter length at 4x half width to avoid spikes on
                    // near-180-degree turns.
                    let denom = (miter[0] * left_out[0] + miter[1] * left_out[1]).clamp(0.25, 1.0);
                    [miter[0] * half / denom, miter[1] * half / denom]
                }
            }
            (Some(incoming), None) => {
                let left = [-incoming[1], incoming[0]];
                [left[0] * half, left[1] * half]
            }
            (None, Some(outgoing)) => {
                let left = [-outgoing[1], outgoing[0]];
                [left[0] * half, left[1] * half]
            }
            (None, None) => [0.0, 0.0],
        };
        let y = altitude + base_height;
        positions.push([point[0] + offset[0], y, point[1] + offset[1]]);
        positions.push([point[0] - offset[0], y, point[1] - offset[1]]);
        normals.push([0.0, 1.0, 0.0]);
        normals.push([0.0, 1.0, 0.0]);
    }

    let mut indices = Vec::with_capacity((cleaned.len() - 1) * 6);
    for segment in 0..cleaned.len() - 1 {
        let base = segment as u32 * 2;
        // Counter-clockwise from above so the ribbon faces +Y.
        indices.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
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
    Some(mesh)
}

/// Extruded polygon with top/bottom caps and side walls.
///
/// Caps are triangulated with ear clipping so concave custom meshes render
/// correctly, and the footprint orientation is normalised so the top cap faces
/// up and the side walls face outwards.
pub fn prism_mesh(footprint: &[[f32; 2]], height: f32) -> Mesh {
    let mut footprint = sanitize_footprint(footprint);
    let count = footprint.len() as u32;
    if count < 3 {
        return Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
    }
    // Canonical orientation: the triangulation below then produces a top cap
    // facing +Y and outward-facing side quads.
    if polygon_area2(&footprint) > 0.0 {
        footprint.reverse();
    }

    let mut positions = Vec::with_capacity(footprint.len() * 2);
    let mut normals = Vec::with_capacity(footprint.len() * 2);
    let mut uvs = Vec::with_capacity(footprint.len() * 2);
    let mut indices = Vec::new();

    for (index, &[x, z]) in footprint.iter().enumerate() {
        let u = index as f32 / count as f32;
        positions.push([x, height, z]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([u, 1.0]);
        positions.push([x, 0.0, z]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([u, 0.0]);
    }

    // Side walls.
    for index in 0..count {
        let next = (index + 1) % count;
        let top = index * 2;
        let bottom = top + 1;
        let next_top = next * 2;
        let next_bottom = next_top + 1;
        indices.extend_from_slice(&[bottom, next_bottom, next_top]);
        indices.extend_from_slice(&[bottom, next_top, top]);
    }

    // Caps. Ear clipping returns indices into the canonical footprint.
    for [a, b, c] in triangulate_polygon(&footprint) {
        let (a, b, c) = if cross2d(
            footprint[a as usize],
            footprint[b as usize],
            footprint[c as usize],
        ) > 0.0
        {
            (a, c, b)
        } else {
            (a, b, c)
        };
        indices.extend_from_slice(&[a * 2, b * 2, c * 2]);
        indices.extend_from_slice(&[a * 2 + 1, c * 2 + 1, b * 2 + 1]);
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

fn cross2d(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn polygon_area2(points: &[[f32; 2]]) -> f32 {
    let mut area = 0.0;
    for index in 0..points.len() {
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        area += current[0] * next[1] - next[0] * current[1];
    }
    area * 0.5
}

fn sanitize_footprint(footprint: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut points: Vec<[f32; 2]> = Vec::with_capacity(footprint.len());
    for &point in footprint {
        if !point[0].is_finite() || !point[1].is_finite() {
            continue;
        }
        if let Some(last) = points.last() {
            let dx = point[0] - last[0];
            let dz = point[1] - last[1];
            if dx * dx + dz * dz < 1.0e-8 {
                continue;
            }
        }
        points.push(point);
    }
    if points.len() > 1 {
        let first = points[0];
        let last = points[points.len() - 1];
        let dx = last[0] - first[0];
        let dz = last[1] - first[1];
        if dx * dx + dz * dz < 1.0e-8 {
            points.pop();
        }
    }
    // Drop collinear vertices; ear clipping stalls otherwise.
    let mut index = 0;
    while points.len() > 3 && index < points.len() {
        let count = points.len();
        let prev = points[(index + count - 1) % count];
        let current = points[index];
        let next = points[(index + 1) % count];
        if cross2d(prev, current, next).abs() < 1.0e-5 {
            points.remove(index);
            index = index.saturating_sub(1);
        } else {
            index += 1;
        }
    }
    points
}

fn point_in_triangle(point: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let d1 = cross2d(a, b, point);
    let d2 = cross2d(b, c, point);
    let d3 = cross2d(c, a, point);
    let has_negative = d1 < -1.0e-6 || d2 < -1.0e-6 || d3 < -1.0e-6;
    let has_positive = d1 > 1.0e-6 || d2 > 1.0e-6 || d3 > 1.0e-6;
    !(has_negative && has_positive)
}

fn triangulate_polygon(points: &[[f32; 2]]) -> Vec<[u32; 3]> {
    let count = points.len();
    if count < 3 {
        return Vec::new();
    }
    let orientation = if polygon_area2(points) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let mut remaining: Vec<usize> = (0..count).collect();
    let mut triangles = Vec::with_capacity(count - 2);
    let mut guard = 0;
    while remaining.len() > 3 && guard < count * count {
        guard += 1;
        let mut clipped = false;
        for position in 0..remaining.len() {
            let length = remaining.len();
            let prev = remaining[(position + length - 1) % length];
            let current = remaining[position];
            let next = remaining[(position + 1) % length];
            if cross2d(points[prev], points[current], points[next]) * orientation <= 0.0 {
                continue;
            }
            let mut blocked = false;
            for &candidate in &remaining {
                if candidate == prev || candidate == current || candidate == next {
                    continue;
                }
                if point_in_triangle(
                    points[candidate],
                    points[prev],
                    points[current],
                    points[next],
                ) {
                    blocked = true;
                    break;
                }
            }
            if blocked {
                continue;
            }
            triangles.push([prev as u32, current as u32, next as u32]);
            remaining.remove(position);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if remaining.len() == 3 {
        triangles.push([
            remaining[0] as u32,
            remaining[1] as u32,
            remaining[2] as u32,
        ]);
    }
    if triangles.len() != count - 2 {
        // Degenerate/self-intersecting input: fall back to a fan so something
        // still renders instead of an empty mesh.
        triangles.clear();
        for index in 1..count - 1 {
            triangles.push([0, index as u32, index as u32 + 1]);
        }
    }
    triangles
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

/// Unit quad on the XY plane centred at the origin and facing +Z. Particle
/// elements are billboarded by copying the camera rotation onto this mesh.
pub fn billboard_quad_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(vec![
            [-0.5, -0.5, 0.0],
            [0.5, -0.5, 0.0],
            [0.5, 0.5, 0.0],
            [-0.5, 0.5, 0.0],
        ]),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        VertexAttributeValues::Float32x3(vec![[0.0, 0.0, 1.0]; 4]),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        VertexAttributeValues::Float32x2(vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]),
    );
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
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
    fn particle_quad_faces_positive_z() {
        let mesh = billboard_quad_mesh();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("particle quad has no position attribute");
        };
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("particle quad has no u32 indices");
        };
        let [a, b, c] = [indices[0], indices[1], indices[2]].map(|index| index as usize);
        let ab = Vec3::from(positions[b]) - Vec3::from(positions[a]);
        let ac = Vec3::from(positions[c]) - Vec3::from(positions[a]);
        assert!(ab.cross(ac).z > 0.0, "particle quad must face +Z");
    }

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

    #[test]
    fn ribbon_faces_up_so_lines_are_visible() {
        let mesh = ribbon_mesh(&[[0.0, 0.0], [10.0, 0.0]], &[], 2.0, 0.0).expect("ribbon mesh");
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("ribbon has no position attribute");
        };
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("ribbon has no u32 indices");
        };
        let [a, b, c] = [indices[0], indices[1], indices[2]].map(|index| index as usize);
        let ab = Vec3::from(positions[b]) - Vec3::from(positions[a]);
        let ac = Vec3::from(positions[c]) - Vec3::from(positions[a]);
        assert!(ab.cross(ac).y > 0.0, "ribbon front face must point up");
    }

    #[test]
    fn ribbon_drops_duplicate_points_and_keeps_its_width() {
        let mesh = ribbon_mesh(&[[0.0, 0.0], [10.0, 0.0], [10.0, 0.0]], &[], 2.0, 0.0)
            .expect("ribbon mesh");
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("ribbon has no position attribute");
        };
        assert_eq!(positions.len(), 4, "duplicate tail point must be removed");
        assert!((positions[0][2] - 1.0).abs() < 1.0e-3);
        assert!((positions[1][2] + 1.0).abs() < 1.0e-3);
        assert!((positions[2][2] - 1.0).abs() < 1.0e-3);
        assert!((positions[3][2] + 1.0).abs() < 1.0e-3);
    }

    #[test]
    fn prism_caps_triangulate_concave_footprints() {
        let footprint = [
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 1.0],
            [1.0, 1.0],
            [1.0, 4.0],
            [0.0, 4.0],
        ];
        let height = 2.0;
        let mesh = prism_mesh(&footprint, height);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("prism has no position attribute");
        };
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("prism has no u32 indices");
        };

        let mut top_area = 0.0f32;
        let mut top_triangles = 0;
        for triangle in indices.chunks(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|index| index as usize);
            let a = Vec3::from(positions[a]);
            let b = Vec3::from(positions[b]);
            let c = Vec3::from(positions[c]);
            if (a.y - height).abs() > 1.0e-4
                || (b.y - height).abs() > 1.0e-4
                || (c.y - height).abs() > 1.0e-4
            {
                continue;
            }
            top_triangles += 1;
            let normal = (b - a).cross(c - a);
            assert!(normal.y > 0.0, "top cap triangle must face up");
            top_area += normal.y * 0.5;
        }

        assert_eq!(top_triangles, footprint.len() - 2);
        let expected = polygon_area2(&footprint).abs();
        assert!(
            (top_area - expected).abs() < 1.0e-3,
            "top area {top_area} != footprint area {expected}"
        );
    }

    #[test]
    fn prism_side_walls_face_outwards() {
        let footprint = [[-2.0, -1.0], [2.0, -1.0], [2.0, 1.0], [-2.0, 1.0]];
        let mesh = prism_mesh(&footprint, 3.0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("prism has no position attribute");
        };
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("prism has no u32 indices");
        };
        for triangle in indices.chunks(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|index| index as usize);
            let a = Vec3::from(positions[a]);
            let b = Vec3::from(positions[b]);
            let c = Vec3::from(positions[c]);
            let normal = (b - a).cross(c - a);
            if normal.y.abs() > normal.length() * 0.5 {
                continue;
            }
            let centroid = (a + b + c) / 3.0;
            assert!(
                normal.dot(Vec3::new(centroid.x, 0.0, centroid.z)) > 0.0,
                "side wall normal must point away from the centre"
            );
        }
    }

    #[test]
    fn ribbon_uses_per_point_altitudes() {
        let mesh =
            ribbon_mesh(&[[0.0, 0.0], [10.0, 0.0]], &[2.0, 6.0], 2.0, 0.5).expect("ribbon mesh");
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("ribbon has no position attribute");
        };
        assert!((positions[0][1] - 2.5).abs() < 1.0e-3);
        assert!((positions[1][1] - 2.5).abs() < 1.0e-3);
        assert!((positions[2][1] - 6.5).abs() < 1.0e-3);
        assert!((positions[3][1] - 6.5).abs() < 1.0e-3);
    }
}
