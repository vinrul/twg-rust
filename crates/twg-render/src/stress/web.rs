use std::collections::HashMap;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use twg_map::BaseLayer;

use super::benchmark::StressBenchmark;
use super::config::{
    DEFAULT_STRESS_ZOOM, StressMode, StressRequest, StressRun, StressState,
};
use crate::map::{BaseLayerResource, MapCameraResource};

pub fn read_web_stress_config(
    mut request: ResMut<StressRequest>,
    mut camera: ResMut<MapCameraResource>,
    mut layers: ResMut<BaseLayerResource>,
) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(search) = window.location().search() else {
        return;
    };
    let search = search.trim_start_matches('?');
    if search.is_empty() {
        return;
    }

    let mut params = HashMap::new();
    for pair in search.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        params.insert(key, value);
    }

    if let Some(zoom) = params.get("zoom").and_then(|value| value.parse::<f32>().ok()) {
        camera.0.set_zoom(zoom);
    }
    match params.get("layer").copied() {
        Some("satellite" | "esri") => layers.0 = BaseLayer::ArcGisImagery,
        Some("osm" | "street") => layers.0 = BaseLayer::OpenStreetMap,
        _ => {}
    }

    let Some(mode) = params.get("stress").and_then(|value| StressMode::parse(value)) else {
        return;
    };
    let count = params
        .get("count")
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_000usize);
    let benchmark = matches!(params.get("benchmark").copied(), Some("1" | "true"));
    let warmup_secs = params
        .get("warmup")
        .and_then(|value| value.parse().ok())
        .unwrap_or(3.0f32);
    let sample_secs = params
        .get("sample")
        .and_then(|value| value.parse().ok())
        .unwrap_or(8.0f32);
    let zoom = params
        .get("zoom")
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_STRESS_ZOOM);

    request.0 = Some(StressRun {
        count,
        mode,
        benchmark,
        warmup_secs,
        sample_secs,
        exit_when_done: false,
        zoom,
    });
}

pub fn publish_web_status(
    time: Res<Time>,
    state: Res<StressState>,
    benchmark: Res<StressBenchmark>,
    diagnostics: Res<DiagnosticsStore>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer < 0.5 {
        return;
    }
    *timer = 0.0;

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or_default();

    let status = if let Some(result) = &benchmark.result {
        format!(
            "RESULT mode={} count={} fps={:.2} worst_ms={:.2}",
            result.mode.label(),
            result.count,
            result.fps,
            result.worst_frame_ms
        )
    } else if state.active {
        format!(
            "mode={} ready={}/{} phase={:?} fps={:.1}",
            state.mode.label(),
            state.ready,
            state.count,
            benchmark.phase,
            fps
        )
    } else {
        format!("idle fps={fps:.1}")
    };

    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            document.set_title(&format!("TWG G1 | {status}"));
        }
    }
}
