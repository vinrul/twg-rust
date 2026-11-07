mod assets;
mod bake;
mod benchmark;
mod config;
mod gltf;
mod memory;
mod probe;
mod reload;
mod selection;
mod spawn;

#[cfg(target_arch = "wasm32")]
mod web;

use bevy::prelude::*;
use bevy::transform::TransformSystems;

pub use benchmark::{BenchPhase, BenchResult, StressBenchmark};
pub use config::{
    ANIMATED_MODEL, ANIMATED_MODEL_DRACO, ANIMATED_MODEL_LOD, ANIMATED_MODEL_LOD_DRACO,
    CACHE_ASSET_SOURCE, DEFAULT_STRESS_ZOOM, MarkerModelOverride, MarkerModelPath,
    MarkerModelReload, MarkerModelSet, STATIC_MODEL, STATIC_MODEL_DRACO, STATIC_MODEL_LOD,
    STATIC_MODEL_LOD_DRACO, STRESS_COUNTS, StressMode, StressRequest, StressRun, StressState,
    cache_asset_path,
};
pub use memory::{MemoryProbe, MemoryProbePhase, MemoryStats};
pub use probe::DracoProbe;
pub use selection::SelectionState;

pub const BAKED_FRAMES: usize = 20;

#[derive(Resource)]
pub struct StressAnimations {
    pub graph: Handle<AnimationGraph>,
    pub index: AnimationNodeIndex,
}

pub struct StressPlugin;

impl Plugin for StressPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StressRequest>()
            .init_resource::<StressState>()
            .init_resource::<StressBenchmark>()
            .init_resource::<MarkerModelSet>()
            .init_resource::<MarkerModelOverride>()
            .init_resource::<MarkerModelReload>()
            .init_resource::<MemoryStats>()
            .init_resource::<SelectionState>()
            .add_systems(
                Startup,
                (
                    resolve_marker_models,
                    bake::setup_bake,
                    assets::load_marker_assets,
                    auto_start_from_run,
                    probe::setup_probe,
                    selection::spawn_selection_assets,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    keyboard_toggle_lod,
                    keyboard_toggle_draco,
                    keyboard_clear,
                    reload::reload_marker_models,
                    assets::finalize_marker_assets,
                    bake::advance_bake_time,
                    spawn::apply_stress_request,
                    memory::sample_memory,
                    memory::probe_memory,
                    spawn::animate_instanced_markers,
                    benchmark::tick_benchmark,
                )
                    .chain(),
            )
            .add_systems(Update, probe::report_probe)
            .add_systems(
                PostUpdate,
                (
                    selection::validate_selection,
                    selection::handle_selection_click,
                )
                    .chain()
                    .after(TransformSystems::Propagate),
            )
            .add_systems(Last, bake::bake_frame)
            .add_observer(bake::on_bake_template_ready);

        #[cfg(target_arch = "wasm32")]
        app.add_systems(Startup, web::read_web_stress_config)
            .add_systems(Update, web::publish_web_status);
    }
}

fn auto_start_from_run(run: Option<Res<StressRun>>, mut request: ResMut<StressRequest>) {
    if let Some(run) = run {
        request.0 = Some(*run);
    }
}

/// Chooses the marker mesh set before the asset handles are created.
///
/// Native builds keep the full-detail mesh. Browser builds default to the
/// lower-detail proxy because WebGL2 is triangle-throughput bound, and the
/// `lod` query parameter can still override the choice per run.
/// Keyboard equivalent of the LOD button: `M` switches full <-> LOD.
fn keyboard_toggle_lod(
    keys: Res<ButtonInput<KeyCode>>,
    editor: Option<Res<crate::editor::EditorMode>>,
    mut models: ResMut<MarkerModelSet>,
    mut reload: ResMut<MarkerModelReload>,
    state: Res<StressState>,
) {
    if editor.is_some_and(|editor| editor.active) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        let lod = !models.lod;
        let draco = models.draco;
        reload.request(&mut models, &state, lod, draco);
    }
}

/// Keyboard equivalent of the Draco button: `D` switches plain <-> Draco.
fn keyboard_toggle_draco(
    keys: Res<ButtonInput<KeyCode>>,
    editor: Option<Res<crate::editor::EditorMode>>,
    mut models: ResMut<MarkerModelSet>,
    mut reload: ResMut<MarkerModelReload>,
    state: Res<StressState>,
) {
    if editor.is_some_and(|editor| editor.active) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyD) {
        let lod = models.lod;
        let draco = !models.draco;
        reload.request(&mut models, &state, lod, draco);
    }
}

fn keyboard_clear(
    keys: Res<ButtonInput<KeyCode>>,
    editor: Option<Res<crate::editor::EditorMode>>,
    mut request: ResMut<StressRequest>,
) {
    if editor.is_some_and(|editor| editor.active) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyC) {
        request.0 = Some(StressRun {
            count: 0,
            mode: StressMode::default(),
            benchmark: false,
            warmup_secs: 0.0,
            sample_secs: 0.0,
            exit_when_done: false,
            zoom: DEFAULT_STRESS_ZOOM,
            pitch_degrees: None,
            bearing_degrees: None,
        });
    }
}

fn resolve_marker_models(mut models: ResMut<MarkerModelSet>) {
    #[cfg(target_arch = "wasm32")]
    {
        let lod = web::lod_preference().unwrap_or(models.lod);
        let draco = web::draco_preference().unwrap_or(models.draco);
        *models = MarkerModelSet::for_variant(lod, draco);
    }

    #[cfg(not(target_arch = "wasm32"))]
    let _ = &mut models;
}
