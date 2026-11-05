mod assets;
mod bake;
mod benchmark;
mod config;
mod probe;
mod spawn;

#[cfg(target_arch = "wasm32")]
mod web;

use bevy::prelude::*;

pub use benchmark::{BenchPhase, BenchResult, StressBenchmark};
pub use config::{
    ANIMATED_MODEL, DEFAULT_STRESS_ZOOM, STATIC_MODEL, STRESS_COUNTS, StressMode, StressRequest,
    StressRun, StressState,
};
pub use probe::DracoProbe;

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
            .add_systems(
                Startup,
                (
                    bake::setup_bake,
                    assets::load_marker_assets,
                    auto_start_from_run,
                    probe::setup_probe,
                ),
            )
            .add_systems(
                Update,
                (
                    assets::finalize_marker_assets,
                    bake::advance_bake_time,
                    spawn::apply_stress_request,
                    spawn::animate_instanced_markers,
                    benchmark::tick_benchmark,
                )
                    .chain(),
            )
            .add_systems(Update, probe::report_probe)
            .add_systems(Last, bake::bake_frame)
            .add_observer(bake::on_bake_template_ready);

        #[cfg(target_arch = "wasm32")]
        app.add_systems(Startup, web::read_web_stress_config)
            .add_systems(Update, web::publish_web_status);
    }
}

fn auto_start_from_run(
    run: Option<Res<StressRun>>,
    mut request: ResMut<StressRequest>,
) {
    if let Some(run) = run {
        request.0 = Some(*run);
    }
}
