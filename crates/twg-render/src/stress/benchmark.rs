use bevy::app::AppExit;
use bevy::prelude::*;

use super::config::{StressMode, StressRun, StressState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BenchPhase {
    #[default]
    Idle,
    WaitingForReady,
    WarmingUp,
    Sampling,
    Done,
}

#[derive(Debug, Clone)]
pub struct BenchResult {
    pub mode: StressMode,
    pub count: usize,
    pub seconds: f32,
    pub frames: u32,
    pub fps: f32,
    pub worst_frame_ms: f32,
}

impl BenchResult {
    pub fn line(&self) -> String {
        format!(
            "STRESS_RESULT mode={} count={} frames={} seconds={:.2} fps={:.2} worst_frame_ms={:.2}",
            self.mode.label(),
            self.count,
            self.frames,
            self.seconds,
            self.fps,
            self.worst_frame_ms
        )
    }
}

#[derive(Resource, Debug, Default)]
pub struct StressBenchmark {
    pub phase: BenchPhase,
    pub warmup_secs: f32,
    pub sample_secs: f32,
    pub exit_when_done: bool,
    pub waited: f32,
    pub elapsed: f32,
    pub frames: u32,
    pub worst_frame: f32,
    pub result: Option<BenchResult>,
}

impl StressBenchmark {
    pub fn reset(&mut self, run: &StressRun) {
        self.phase = if run.benchmark && run.count > 0 {
            BenchPhase::WaitingForReady
        } else {
            BenchPhase::Idle
        };
        self.warmup_secs = run.warmup_secs.max(0.0);
        self.sample_secs = run.sample_secs.max(0.5);
        self.exit_when_done = run.exit_when_done;
        self.waited = 0.0;
        self.elapsed = 0.0;
        self.frames = 0;
        self.worst_frame = 0.0;
        self.result = None;
    }
}

pub fn tick_benchmark(
    time: Res<Time>,
    state: Res<StressState>,
    mut benchmark: ResMut<StressBenchmark>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    match benchmark.phase {
        BenchPhase::Idle | BenchPhase::Done => {}
        BenchPhase::WaitingForReady => {
            benchmark.waited += dt;
            if state.active && state.ready >= state.count {
                benchmark.phase = BenchPhase::WarmingUp;
                benchmark.elapsed = 0.0;
                info!(
                    "stress scenes ready ({}/{}), warming up for {:.1}s",
                    state.ready, state.count, benchmark.warmup_secs
                );
            } else if benchmark.waited > 180.0 {
                benchmark.phase = BenchPhase::Done;
                println!(
                    "STRESS_RESULT mode={} count={} ERROR=timeout ready={}/{}",
                    state.mode.label(),
                    state.count,
                    state.ready,
                    state.count
                );
                if benchmark.exit_when_done {
                    exit.write(AppExit::Success);
                }
            }
        }
        BenchPhase::WarmingUp => {
            benchmark.elapsed += dt;
            if benchmark.elapsed >= benchmark.warmup_secs {
                benchmark.phase = BenchPhase::Sampling;
                benchmark.elapsed = 0.0;
                benchmark.frames = 0;
                benchmark.worst_frame = 0.0;
                info!("stress sampling started");
            }
        }
        BenchPhase::Sampling => {
            benchmark.elapsed += dt;
            benchmark.frames += 1;
            benchmark.worst_frame = benchmark.worst_frame.max(dt);
            if benchmark.elapsed >= benchmark.sample_secs {
                let fps = benchmark.frames as f32 / benchmark.elapsed.max(f32::EPSILON);
                let result = BenchResult {
                    mode: state.mode,
                    count: state.count,
                    seconds: benchmark.elapsed,
                    frames: benchmark.frames,
                    fps,
                    worst_frame_ms: benchmark.worst_frame * 1_000.0,
                };
                println!("{}", result.line());
                info!(target: "twg_stress", "{}", result.line());
                benchmark.result = Some(result);
                benchmark.phase = BenchPhase::Done;
                if benchmark.exit_when_done {
                    exit.write(AppExit::Success);
                }
            }
        }
    }
}
