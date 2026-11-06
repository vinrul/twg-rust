use bevy::prelude::*;

use super::StressAnimations;
use super::assets::MarkerAssets;
use super::bake::{self, BakedAnimation};
use super::benchmark::{BenchPhase, StressBenchmark};
use super::config::{MarkerModelReload, MarkerModelSet, StressMarker, StressRequest, StressState};

/// Rebuilds marker assets and animation frames after a runtime LOD switch.
#[allow(clippy::too_many_arguments)]
pub fn reload_marker_models(
    mut commands: Commands,
    mut reload: ResMut<MarkerModelReload>,
    models: Res<MarkerModelSet>,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut assets: ResMut<MarkerAssets>,
    mut bake: ResMut<BakedAnimation>,
    mut animations: ResMut<StressAnimations>,
    mut request: ResMut<StressRequest>,
    mut state: ResMut<StressState>,
    mut benchmark: ResMut<StressBenchmark>,
    markers: Query<Entity, With<StressMarker>>,
) {
    if !reload.pending {
        return;
    }
    reload.pending = false;

    for entity in &markers {
        commands.entity(entity).despawn();
    }
    if let Some(template) = bake.template.take() {
        commands.entity(template).despawn();
    }

    *assets = MarkerAssets::load(&asset_server, &models);
    let (new_animations, new_bake) =
        bake::begin_bake(&mut commands, &asset_server, &mut graphs, &models);
    *animations = new_animations;
    *bake = new_bake;

    state.active = false;
    state.ready = 0;
    benchmark.phase = BenchPhase::Idle;
    benchmark.result = None;

    if let Some(run) = reload.respawn.take() {
        state.count = run.count;
        state.mode = run.mode;
        state.zoom = run.zoom;
        request.0 = Some(run);
    } else {
        state.count = 0;
        request.0 = None;
    }

    info!(
        "marker models reloaded: {} (lod={}, draco={}), waiting for assets and bake",
        models.animated, models.lod, models.draco
    );
}
