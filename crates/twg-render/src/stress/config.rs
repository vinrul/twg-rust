use bevy::prelude::*;

pub const STRESS_COUNTS: [usize; 3] = [1_000, 5_000, 10_000];
pub const DEFAULT_STRESS_ZOOM: f32 = 19.0;
pub const ANIMATED_MODEL: &str = "models/polwan_airud_walk_non_draco.glb";
pub const STATIC_MODEL: &str = "models/polwan_airud_idle_non_draco.glb";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StressMode {
    Animated,
    Static,
    Mixed,
}

impl StressMode {
    pub const ALL: [Self; 3] = [Self::Animated, Self::Static, Self::Mixed];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Animated => "Animated",
            Self::Static => "Static",
            Self::Mixed => "Mixed 50/50",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "animated" | "anim" => Some(Self::Animated),
            "static" | "idle" => Some(Self::Static),
            "mixed" | "mix" => Some(Self::Mixed),
            _ => None,
        }
    }

    pub fn animated_share(self, index: usize) -> bool {
        match self {
            Self::Animated => true,
            Self::Static => false,
            Self::Mixed => index % 2 == 0,
        }
    }
}

impl Default for StressMode {
    fn default() -> Self {
        Self::Mixed
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct StressRun {
    pub count: usize,
    pub mode: StressMode,
    pub benchmark: bool,
    pub warmup_secs: f32,
    pub sample_secs: f32,
    pub exit_when_done: bool,
    pub zoom: f32,
}

impl Default for StressRun {
    fn default() -> Self {
        Self {
            count: 1_000,
            mode: StressMode::default(),
            benchmark: false,
            warmup_secs: 3.0,
            sample_secs: 8.0,
            exit_when_done: false,
            zoom: DEFAULT_STRESS_ZOOM,
        }
    }
}

#[derive(Resource, Default)]
pub struct StressRequest(pub Option<StressRun>);

#[derive(Resource, Debug)]
pub struct StressState {
    pub count: usize,
    pub mode: StressMode,
    pub ready: usize,
    pub active: bool,
    pub generation: u64,
    pub last_error: Option<String>,
}

impl Default for StressState {
    fn default() -> Self {
        Self {
            count: 0,
            mode: StressMode::default(),
            ready: 0,
            active: false,
            generation: 0,
            last_error: None,
        }
    }
}

#[derive(Component)]
pub struct StressMarker {
    pub animated: bool,
    pub phase: f32,
}

#[derive(Component)]
pub struct BakeTemplate;
