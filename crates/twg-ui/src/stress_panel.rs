use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use twg_render::{
    DEFAULT_STRESS_ZOOM, MarkerModelReload, MarkerModelSet, MemoryStats, STRESS_COUNTS,
    SelectionState, StressBenchmark, StressMode, StressRequest, StressRun, StressState,
};

#[allow(clippy::too_many_arguments)]
pub fn draw_stress_panel(
    mut contexts: EguiContexts,
    mut request: ResMut<StressRequest>,
    state: Res<StressState>,
    benchmark: Res<StressBenchmark>,
    mut models: ResMut<MarkerModelSet>,
    mut reload: ResMut<MarkerModelReload>,
    memory: Res<MemoryStats>,
    selection: Res<SelectionState>,
    diagnostics: Res<DiagnosticsStore>,
    mut count_index: Local<usize>,
    mut mode: Local<StressMode>,
) -> Result {
    let context = contexts.ctx_mut()?;
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or_default();
    draw_stress_panel_ctx(
        context,
        &mut request,
        &state,
        &benchmark,
        &mut models,
        &mut reload,
        &memory,
        &selection,
        fps,
        &mut count_index,
        &mut mode,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn draw_stress_panel_ctx(
    context: &egui::Context,
    request: &mut StressRequest,
    state: &StressState,
    benchmark: &StressBenchmark,
    models: &mut MarkerModelSet,
    reload: &mut MarkerModelReload,
    memory: &MemoryStats,
    selection: &SelectionState,
    fps: f64,
    count_index: &mut usize,
    mode: &mut StressMode,
) {
    let mut spawn_benchmark = None;
    let mut spawn_interactive = false;
    let mut clear = false;

    egui::Window::new("G1 Marker Stress")
        .default_pos(egui::pos2(16.0, 380.0))
        .resizable(false)
        .show(context, |ui| {
            ui.label("Marker count");
            ui.horizontal(|ui| {
                for (index, count) in STRESS_COUNTS.iter().enumerate() {
                    if ui
                        .selectable_label(*count_index == index, format!("{count}"))
                        .clicked()
                    {
                        *count_index = index;
                    }
                }
            });

            ui.label("Scenario");
            ui.horizontal(|ui| {
                for candidate in StressMode::ALL {
                    if ui
                        .selectable_label(*mode == candidate, candidate.label())
                        .clicked()
                    {
                        *mode = candidate;
                    }
                }
            });

            ui.label("Marker mesh");
            ui.horizontal(|ui| {
                if ui.selectable_label(!models.lod, "Full 9.6k").clicked() && models.lod {
                    reload.request(models, state, false);
                }
                if ui.selectable_label(models.lod, "LOD 2.4k").clicked() && !models.lod {
                    reload.request(models, state, true);
                }
            });
            if models.lod && !cfg!(target_arch = "wasm32") {
                ui.small("LOD aktif — cocok untuk grup marker besar");
            }
            ui.small("M: toggle full/LOD  ·  C: clear markers");

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Spawn / apply").clicked() {
                    spawn_interactive = true;
                }
                if ui.button("Benchmark 8s").clicked() {
                    spawn_benchmark = Some(true);
                }
                if ui.button("Clear").clicked() {
                    clear = true;
                }
            });
            ui.separator();
            ui.monospace(format!(
                "mesh    {}",
                if models.lod {
                    "LOD 2.4k tris"
                } else {
                    "full 9.6k tris"
                }
            ));
            ui.monospace(format!(
                "markers {}/{}   fps {:.1}",
                state.ready, state.count, fps
            ));
            ui.monospace(format!(
                "mem     {:.1} MB (peak {:.1})",
                memory.current_mb, memory.peak_mb
            ));
            match selection.index {
                Some(index) => ui.monospace(format!("select  marker #{index}")),
                None => ui.monospace("select  —"),
            };
            if let (Some(loaded), Some(cleared)) = (memory.loaded_mb, memory.cleared_mb) {
                ui.monospace(format!("load/unload {loaded:.1} / {cleared:.1} MB"));
            }
            ui.monospace(format!("phase {:?}", benchmark.phase));
            if let Some(result) = &benchmark.result {
                ui.monospace(result.line());
            }
            if let Some(error) = &state.last_error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
        });

    if clear {
        request.0 = Some(StressRun {
            count: 0,
            mode: *mode,
            benchmark: false,
            warmup_secs: 3.0,
            sample_secs: 8.0,
            exit_when_done: false,
            zoom: DEFAULT_STRESS_ZOOM,
            pitch_degrees: None,
            bearing_degrees: None,
        });
    } else if spawn_benchmark.is_some() || spawn_interactive {
        request.0 = Some(StressRun {
            count: STRESS_COUNTS[*count_index],
            mode: *mode,
            benchmark: spawn_benchmark.is_some(),
            warmup_secs: 3.0,
            sample_secs: 8.0,
            exit_when_done: false,
            zoom: DEFAULT_STRESS_ZOOM,
            pitch_degrees: None,
            bearing_degrees: None,
        });
    }
}
