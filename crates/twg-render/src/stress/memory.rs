use bevy::app::AppExit;
use bevy::prelude::*;

use super::config::{StressRequest, StressRun, StressState};

const SAMPLE_INTERVAL: f32 = 1.0;
const SETTLE_SECS: f32 = 1.0;

#[derive(Resource, Debug, Default)]
pub struct MemoryStats {
    pub current_mb: f32,
    pub peak_mb: f32,
    pub loaded_mb: Option<f32>,
    pub cleared_mb: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MemoryProbePhase {
    #[default]
    Idle,
    WaitingLoaded,
    WaitingCleared,
    Clearing,
    Done,
}

/// Optional scripted load/unload memory measurement.
#[derive(Resource, Debug)]
pub struct MemoryProbe {
    pub phase: MemoryProbePhase,
    pub exit_when_done: bool,
    elapsed: f32,
}

impl MemoryProbe {
    pub const fn new(exit_when_done: bool) -> Self {
        Self {
            phase: MemoryProbePhase::Idle,
            exit_when_done,
            elapsed: 0.0,
        }
    }
}

impl Default for MemoryProbe {
    fn default() -> Self {
        Self::new(false)
    }
}

pub fn sample_memory(mut stats: ResMut<MemoryStats>, mut timer: Local<f32>, time: Res<Time>) {
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = SAMPLE_INTERVAL;
    if let Some(bytes) = current_bytes() {
        let mb = bytes as f32 / (1024.0 * 1024.0);
        stats.current_mb = mb;
        stats.peak_mb = stats.peak_mb.max(mb);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn probe_memory(
    time: Res<Time>,
    state: Res<StressState>,
    mut probe: Option<ResMut<MemoryProbe>>,
    mut stats: ResMut<MemoryStats>,
    mut request: ResMut<StressRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(probe) = probe.as_mut() else {
        return;
    };
    let dt = time.delta_secs();

    match probe.phase {
        MemoryProbePhase::WaitingLoaded => {
            probe.phase = MemoryProbePhase::Idle;
        }
        MemoryProbePhase::Idle => {
            if state.active && state.count > 0 && state.ready >= state.count {
                probe.elapsed += dt;
                if probe.elapsed >= SETTLE_SECS {
                    let mb = current_mb();
                    stats.loaded_mb = Some(mb);
                    info!("MEMORY phase=loaded count={} mb={mb:.1}", state.count);
                    probe.phase = MemoryProbePhase::WaitingCleared;
                    probe.elapsed = 0.0;
                }
            } else {
                probe.elapsed = 0.0;
            }
        }
        MemoryProbePhase::WaitingCleared => {
            probe.elapsed += dt;
            if probe.elapsed >= SETTLE_SECS {
                request.0 = Some(StressRun {
                    count: 0,
                    mode: state.mode,
                    benchmark: false,
                    warmup_secs: 0.0,
                    sample_secs: 0.0,
                    exit_when_done: false,
                    zoom: state.zoom.max(0.1),
                    pitch_degrees: None,
                    bearing_degrees: None,
                });
                probe.phase = MemoryProbePhase::Clearing;
                probe.elapsed = 0.0;
            }
        }
        MemoryProbePhase::Clearing => {
            if !state.active && state.count == 0 {
                probe.elapsed += dt;
                if probe.elapsed >= SETTLE_SECS {
                    let mb = current_mb();
                    stats.cleared_mb = Some(mb);
                    info!("MEMORY phase=cleared mb={mb:.1}");
                    probe.phase = MemoryProbePhase::Done;
                    if probe.exit_when_done {
                        exit.write(AppExit::Success);
                    }
                }
            } else {
                probe.elapsed = 0.0;
            }
        }
        MemoryProbePhase::Done => {}
    }
}

fn current_mb() -> f32 {
    current_bytes().map_or(0.0, |bytes| bytes as f32 / (1024.0 * 1024.0))
}

#[cfg(not(target_arch = "wasm32"))]
fn current_bytes() -> Option<u64> {
    memory_stats::memory_stats().map(|stats| stats.physical_mem as u64)
}

#[cfg(target_arch = "wasm32")]
fn current_bytes() -> Option<u64> {
    // Bevy and its allocations live in the WebAssembly linear memory. The
    // number of 64 KiB pages is a stable, dependency-free measurement.
    Some(core::arch::wasm32::memory_size(0) as u64 * 65_536)
}
