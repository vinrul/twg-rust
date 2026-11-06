use std::collections::HashMap;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use twg_map::BaseLayer;

use super::benchmark::StressBenchmark;
use super::config::{
    DEFAULT_STRESS_ZOOM, MarkerModelSet, StressMode, StressRequest, StressRun, StressState,
};
use super::memory::{MemoryProbe, MemoryStats};
use super::selection::SelectionState;
use crate::map::{BaseLayerResource, MapCameraResource, TileProxyConfig};

fn query_params() -> HashMap<String, String> {
    let mut params = HashMap::new();
    let Some(window) = web_sys::window() else {
        return params;
    };
    let Ok(search) = window.location().search() else {
        return params;
    };
    let search = search.trim_start_matches('?');
    for pair in search.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        if !key.is_empty() {
            params.insert(percent_decode(key), percent_decode(value));
        }
    }
    params
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            output.push(high * 16 + low);
            index += 3;
            continue;
        }
        if bytes[index] == b'+' {
            output.push(b' ');
        } else {
            output.push(bytes[index]);
        }
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

/// Reads the optional `lod` query parameter before marker assets are picked.
pub fn lod_preference() -> Option<bool> {
    let params = query_params();
    params.get("lod").map(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "lod" | "low"
        )
    })
}

pub fn read_web_stress_config(
    mut commands: Commands,
    mut request: ResMut<StressRequest>,
    mut camera: ResMut<MapCameraResource>,
    mut layers: ResMut<BaseLayerResource>,
    mut tile_proxy: ResMut<TileProxyConfig>,
) {
    let params = query_params();
    if params.is_empty() {
        return;
    }

    if let Some(proxy) = params.get("proxy") {
        tile_proxy.0 = if proxy.trim().is_empty() {
            None
        } else {
            Some(proxy.clone())
        };
    }

    if matches!(params.get("memory").map(String::as_str), Some("1" | "true")) {
        commands.insert_resource(MemoryProbe::new(false));
    }

    if let Some(zoom) = params
        .get("zoom")
        .and_then(|value| value.parse::<f32>().ok())
    {
        camera.0.set_zoom(zoom);
    }
    let bearing = params
        .get("bearing")
        .and_then(|value| value.parse::<f32>().ok());
    if let Some(bearing) = bearing {
        camera.0.bearing_degrees = bearing.rem_euclid(360.0);
    }
    let pitch = params
        .get("pitch")
        .and_then(|value| value.parse::<f32>().ok());
    if let Some(pitch) = pitch {
        camera.0.pitch_degrees = pitch.clamp(0.0, 85.0);
    }
    match params.get("layer").map(String::as_str) {
        Some("satellite" | "esri") => layers.0 = BaseLayer::ArcGisImagery,
        Some("osm" | "street") => layers.0 = BaseLayer::OpenStreetMap,
        _ => {}
    }

    let Some(mode) = params
        .get("stress")
        .and_then(|value| StressMode::parse(value))
    else {
        return;
    };
    let count = params
        .get("count")
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_000usize);
    let benchmark = matches!(
        params.get("benchmark").map(String::as_str),
        Some("1" | "true")
    );
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
        pitch_degrees: pitch,
        bearing_degrees: bearing,
    });
}

#[allow(clippy::too_many_arguments)]
pub fn publish_web_status(
    time: Res<Time>,
    state: Res<StressState>,
    camera: Res<MapCameraResource>,
    models: Res<MarkerModelSet>,
    memory: Res<MemoryStats>,
    selection: Res<SelectionState>,
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

    let zoom = camera.0.zoom;
    let lat = camera.0.center.latitude;
    let lon = camera.0.center.longitude;
    let bearing = camera.0.bearing_degrees;
    let pitch = camera.0.pitch_degrees;
    let lod = u8::from(models.lod);
    let mem = memory.current_mb;
    let loaded = memory
        .loaded_mb
        .map_or_else(|| "-".to_owned(), |value| format!("{value:.1}"));
    let cleared = memory
        .cleared_mb
        .map_or_else(|| "-".to_owned(), |value| format!("{value:.1}"));
    let selected = selection
        .index
        .map_or_else(|| "-".to_owned(), |index| index.to_string());
    let status = if let Some(result) = &benchmark.result {
        format!(
            "RESULT mode={} count={} fps={:.2} worst_ms={:.2} zoom={zoom:.2} lat={lat:.6} lon={lon:.6} bearing={bearing:.1} pitch={pitch:.1} lod={lod} sel={selected} mem_mb={mem:.1} loaded_mb={loaded} cleared_mb={cleared}",
            result.mode.label(),
            result.count,
            result.fps,
            result.worst_frame_ms
        )
    } else if state.active {
        format!(
            "mode={} ready={}/{} phase={:?} fps={:.1} zoom={zoom:.2} lat={lat:.6} lon={lon:.6} bearing={bearing:.1} pitch={pitch:.1} lod={lod} sel={selected} mem_mb={mem:.1} loaded_mb={loaded} cleared_mb={cleared}",
            state.mode.label(),
            state.ready,
            state.count,
            benchmark.phase,
            fps
        )
    } else {
        format!(
            "idle fps={fps:.1} zoom={zoom:.2} lat={lat:.6} lon={lon:.6} bearing={bearing:.1} pitch={pitch:.1} lod={lod} sel={selected} mem_mb={mem:.1} loaded_mb={loaded} cleared_mb={cleared}"
        )
    };

    if let Some(window) = web_sys::window()
        && let Some(document) = window.document()
    {
        document.set_title(&format!("TWG G1 | {status}"));
    }
}
