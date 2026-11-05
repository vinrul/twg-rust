use std::f32::consts::TAU;

use bevy::prelude::*;

use super::assets::MarkerAssets;
use super::bake::BakedAnimation;
use super::benchmark::StressBenchmark;
use super::config::{StressMarker, StressMode, StressRequest, StressState};
use crate::map::{DemoMarker, MapCameraResource};

const STRESS_SPAN_METERS: f32 = 300.0;
const MARKER_MODEL_SCALE: f32 = 0.02;

pub fn apply_stress_request(
    mut commands: Commands,
    mut request: ResMut<StressRequest>,
    mut state: ResMut<StressState>,
    mut benchmark: ResMut<StressBenchmark>,
    mut camera: ResMut<MapCameraResource>,
    assets: Res<MarkerAssets>,
    baked: Res<BakedAnimation>,
    markers: Query<Entity, With<StressMarker>>,
    demos: Query<Entity, With<DemoMarker>>,
) {
    let Some(run) = request.0 else {
        return;
    };

    let needs_animation = run.mode != StressMode::Static;
    if !assets.ready {
        return;
    }
    if needs_animation && (!baked.ready || baked.frames.len() < 2) {
        return;
    }

    request.0 = None;

    for entity in &markers {
        commands.entity(entity).despawn();
    }
    if run.count > 0 {
        for entity in &demos {
            commands.entity(entity).despawn();
        }
    }

    state.count = run.count;
    state.mode = run.mode;
    state.ready = 0;
    state.active = run.count > 0;
    state.generation += 1;
    state.last_error = None;
    benchmark.reset(&run);

    if run.count == 0 {
        return;
    }

    camera.0.set_zoom(run.zoom);
    camera.0.pitch_degrees = 55.0;
    camera.0.bearing_degrees = 0.0;

    let side = (run.count as f32).sqrt().ceil().max(1.0) as usize;
    let spacing = (STRESS_SPAN_METERS / side as f32).max(0.25);
    let half = (side as f32 - 1.0) * 0.5;

    info!(
        "spawning {} instanced markers ({}), grid {}x{} at {:.2}m spacing, {} baked frames",
        run.count,
        run.mode.label(),
        side,
        side,
        spacing,
        baked.frames.len()
    );

    for index in 0..run.count {
        let animated = run.mode.animated_share(index);
        let mesh = if animated {
            baked.frames[index % baked.frames.len()].clone()
        } else {
            assets.static_mesh.clone()
        };
        let material = if animated {
            assets.animated_material.clone()
        } else {
            assets.static_material.clone()
        };
        let column = (index % side) as f32;
        let row = (index / side) as f32;
        let rotation = Quat::from_rotation_y((index as f32 * 0.618_034) % TAU);
        let phase = index as f32 / run.count as f32;

        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_xyz((column - half) * spacing, 0.0, (row - half) * spacing)
                .with_scale(Vec3::splat(MARKER_MODEL_SCALE))
                .with_rotation(rotation),
            StressMarker { animated, phase },
        ));
    }

    state.ready = run.count;
}

pub fn animate_instanced_markers(
    time: Res<Time>,
    baked: Res<BakedAnimation>,
    mut markers: Query<(&StressMarker, &mut Mesh3d)>,
    mut accumulator: Local<f32>,
) {
    if !baked.ready || baked.duration <= 0.0 || baked.frames.len() < 2 {
        return;
    }

    // WebGL2 has no GPU preprocessing, so re-batching every instance each
    // frame is expensive. Updating at 10 Hz keeps animation moving while
    // leaving the renderer time to draw.
    if cfg!(target_arch = "wasm32") {
        *accumulator += time.delta_secs();
        if *accumulator < 0.1 {
            return;
        }
        *accumulator = 0.0;
    }

    let frame_count = baked.frames.len();
    let cycle = time.elapsed_secs() / baked.duration;
    for (marker, mut mesh) in &mut markers {
        if !marker.animated {
            continue;
        }
        let position = (cycle + marker.phase).rem_euclid(1.0);
        let frame = ((position * frame_count as f32) as usize) % frame_count;
        let handle = &baked.frames[frame];
        if mesh.0 != *handle {
            mesh.0 = handle.clone();
        }
    }
}
