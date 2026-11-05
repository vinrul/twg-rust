use bevy::asset::RenderAssetUsages;
use bevy::gltf::GltfAssetLabel;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};

use super::config::{ANIMATED_MODEL, BakeTemplate};
use super::{BAKED_FRAMES, StressAnimations};

#[derive(Resource)]
pub struct BakedAnimation {
    pub clip: Handle<AnimationClip>,
    pub index: AnimationNodeIndex,
    pub total_frames: usize,
    pub frames: Vec<Handle<Mesh>>,
    pub duration: f32,
    pub next_frame: usize,
    pub template: Option<Entity>,
    pub player: Option<Entity>,
    pub skinned: Option<Entity>,
    pub duration_known: bool,
    pub started: bool,
    pub ready: bool,
}

pub fn setup_bake(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let clip =
        asset_server.load(GltfAssetLabel::Animation(0).from_asset(ANIMATED_MODEL));
    let (graph, index) = AnimationGraph::from_clip(clip.clone());
    commands.insert_resource(StressAnimations {
        graph: graphs.add(graph),
        index,
    });
    commands.insert_resource(BakedAnimation {
        clip,
        index,
        total_frames: BAKED_FRAMES,
        frames: Vec::new(),
        duration: 0.0,
        next_frame: 0,
        template: None,
        player: None,
        skinned: None,
        duration_known: false,
        started: false,
        ready: false,
    });

    commands.spawn((
        WorldAssetRoot(
            asset_server.load(GltfAssetLabel::Scene(0).from_asset(ANIMATED_MODEL)),
        ),
        Transform::default(),
        Visibility::Hidden,
        BakeTemplate,
        Name::new("animation bake template"),
    ));
}

pub fn on_bake_template_ready(
    ready: On<WorldInstanceReady>,
    templates: Query<&BakeTemplate>,
    mut bake: ResMut<BakedAnimation>,
    mut commands: Commands,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    skinned_meshes: Query<&SkinnedMesh>,
    animations: Res<StressAnimations>,
) {
    if templates.get(ready.entity).is_err() {
        return;
    }
    bake.template = Some(ready.entity);

    for child in children.iter_descendants(ready.entity) {
        if bake.player.is_none() {
            if let Ok(mut player) = players.get_mut(child) {
                player.play(animations.index).repeat().set_speed(0.0);
                commands
                    .entity(child)
                    .insert(AnimationGraphHandle(animations.graph.clone()));
                bake.player = Some(child);
            }
        }
        if bake.skinned.is_none() && skinned_meshes.contains(child) {
            bake.skinned = Some(child);
        }
    }
}

pub fn advance_bake_time(
    mut bake: ResMut<BakedAnimation>,
    clips: Res<Assets<AnimationClip>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    if bake.ready {
        return;
    }
    let Some(player_entity) = bake.player else {
        return;
    };

    if !bake.duration_known {
        let Some(clip) = clips.get(&bake.clip) else {
            return;
        };
        bake.duration = clip.duration();
        if bake.duration <= 0.0 {
            return;
        }
        bake.duration_known = true;
    }

    if bake.next_frame >= bake.total_frames {
        return;
    }

    let time = bake.next_frame as f32 / bake.total_frames as f32 * bake.duration;
    let Ok(mut player) = players.get_mut(player_entity) else {
        return;
    };
    let Some(active) = player.animation_mut(bake.index) else {
        return;
    };
    active.set_speed(0.0);
    active.set_seek_time(time);
    bake.started = true;
}

pub fn bake_frame(
    mut commands: Commands,
    mut bake: ResMut<BakedAnimation>,
    skinned_meshes: Query<&SkinnedMesh>,
    mesh_handles: Query<&Mesh3d>,
    joints: Query<&GlobalTransform>,
    inverse_bindposes: Res<Assets<SkinnedMeshInverseBindposes>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if bake.ready || !bake.started {
        return;
    }
    let Some(skinned_entity) = bake.skinned else {
        return;
    };
    let Some(template) = bake.template else {
        return;
    };
    let Ok(skinned) = skinned_meshes.get(skinned_entity) else {
        return;
    };
    let Ok(Mesh3d(mesh_handle)) = mesh_handles.get(skinned_entity) else {
        return;
    };
    let Some(inverse) = inverse_bindposes.get(&skinned.inverse_bindposes) else {
        return;
    };

    let frame = {
        let Some(source_mesh) = meshes.get(mesh_handle) else {
            return;
        };
        let Some(frame) = build_skinned_frame(source_mesh, skinned, inverse, &joints) else {
            warn!("could not bake animation frame {}", bake.next_frame);
            return;
        };
        frame
    };

    bake.frames.push(meshes.add(frame));
    bake.next_frame += 1;

    if bake.next_frame >= bake.total_frames {
        bake.ready = true;
        commands.entity(template).despawn();
        info!(
            "baked {} animation frames over {:.2}s",
            bake.frames.len(),
            bake.duration
        );
    }
}

fn build_skinned_frame(
    source: &Mesh,
    skinned: &SkinnedMesh,
    inverse_bindposes: &SkinnedMeshInverseBindposes,
    joints: &Query<&GlobalTransform>,
) -> Option<Mesh> {
    let positions = source.attribute(Mesh::ATTRIBUTE_POSITION)?.as_float3()?;
    let normals = source
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .and_then(VertexAttributeValues::as_float3);
    let uvs = match source.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(values)) => Some(values.as_slice()),
        _ => None,
    };
    let weights = match source.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT) {
        Some(VertexAttributeValues::Float32x4(values)) => values.as_slice(),
        _ => return None,
    };

    let joint_indices = match source.attribute(Mesh::ATTRIBUTE_JOINT_INDEX)? {
        VertexAttributeValues::Uint16x4(values) => JointIndices::U16(values.clone()),
        VertexAttributeValues::Uint8x4(values) => JointIndices::U8(values.clone()),
        _ => return None,
    };

    let joint_count = skinned.joints.len();
    if joint_count == 0 || inverse_bindposes.len() < joint_count {
        return None;
    }

    let mut matrices = Vec::with_capacity(joint_count);
    for (&entity, &inverse_bindpose) in
        skinned.joints.iter().zip(inverse_bindposes.iter())
    {
        let global = joints.get(entity).ok()?;
        matrices.push(Mat4::from(global.affine()) * inverse_bindpose);
    }

    let vertex_count = positions.len();
    let mut baked_positions = Vec::with_capacity(vertex_count);
    let mut baked_normals = Vec::with_capacity(vertex_count);

    for index in 0..vertex_count {
        let position = Vec3::from_array(positions[index]);
        let normal = normals
            .map(|normals| Vec3::from_array(normals[index]))
            .unwrap_or(Vec3::Y);
        let vertex_joints = joint_indices.get(index);
        let vertex_weights = weights[index];

        let mut skinned_position = Vec3::ZERO;
        let mut skinned_normal = Vec3::ZERO;
        for slot in 0..4 {
            let weight = vertex_weights[slot];
            if weight == 0.0 {
                continue;
            }
            let joint = vertex_joints[slot] as usize;
            if joint >= matrices.len() {
                continue;
            }
            let matrix = matrices[joint];
            skinned_position += weight * matrix.transform_point3(position);
            skinned_normal += weight * matrix.transform_vector3(normal);
        }

        baked_positions.push(skinned_position.to_array());
        baked_normals.push(skinned_normal.normalize_or_zero().to_array());
    }

    let indices = source
        .indices()
        .map(|indices| indices.iter().map(|index| index as u32).collect::<Vec<u32>>())
        .unwrap_or_default();

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(baked_positions),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        VertexAttributeValues::Float32x3(baked_normals),
    );
    if let Some(uvs) = uvs {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            VertexAttributeValues::Float32x2(uvs.to_vec()),
        );
    }
    if !indices.is_empty() {
        mesh.insert_indices(Indices::U32(indices));
    }
    Some(mesh)
}

enum JointIndices {
    U16(Vec<[u16; 4]>),
    U8(Vec<[u8; 4]>),
}

impl JointIndices {
    fn get(&self, index: usize) -> [u16; 4] {
        match self {
            Self::U16(values) => values[index],
            Self::U8(values) => values[index].map(u16::from),
        }
    }
}
