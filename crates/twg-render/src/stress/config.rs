use bevy::prelude::*;

pub const STRESS_COUNTS: [usize; 3] = [1_000, 5_000, 10_000];
pub const DEFAULT_STRESS_ZOOM: f32 = 19.0;
pub const MARKER_MODEL_SCALE: f32 = 0.02;
pub const ANIMATED_MODEL: &str = "models/polwan_airud_walk_non_draco.glb";
pub const STATIC_MODEL: &str = "models/polwan_airud_idle_non_draco.glb";
pub const ANIMATED_MODEL_LOD: &str = "models/polwan_airud_walk_lod2.glb";
pub const STATIC_MODEL_LOD: &str = "models/polwan_airud_idle_lod2.glb";

/// Meshes used by the stress spawner.
///
/// WebGL2 saturates its triangle throughput at roughly one billion
/// triangles per second on the reference GPU. The full-detail police model
/// has ~9.6k triangles, so 10.000 markers cannot reach 25-30 FPS in the
/// browser without a lower-detail proxy. The LOD variant keeps the skeleton
/// and animation clip but has ~2.4k triangles, which is the production path
/// for large marker groups on the web.
#[derive(Resource, Debug, Clone, Copy)]
pub struct MarkerModelSet {
    pub animated: &'static str,
    pub still: &'static str,
    pub lod: bool,
}

impl MarkerModelSet {
    pub const FULL: Self = Self {
        animated: ANIMATED_MODEL,
        still: STATIC_MODEL,
        lod: false,
    };

    pub const LOD: Self = Self {
        animated: ANIMATED_MODEL_LOD,
        still: STATIC_MODEL_LOD,
        lod: true,
    };

    pub const fn for_lod(lod: bool) -> Self {
        if lod { Self::LOD } else { Self::FULL }
    }
}

impl Default for MarkerModelSet {
    fn default() -> Self {
        // The browser needs the lower-detail proxy to reach the Gate G1 FPS
        // floor with 10.000 markers.
        Self::for_lod(cfg!(target_arch = "wasm32"))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StressMode {
    Animated,
    Static,
    #[default]
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
            Self::Mixed => index.is_multiple_of(2),
        }
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
    pub pitch_degrees: Option<f32>,
    pub bearing_degrees: Option<f32>,
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
            pitch_degrees: None,
            bearing_degrees: None,
        }
    }
}

#[derive(Resource, Default)]
pub struct StressRequest(pub Option<StressRun>);

#[derive(Resource, Debug, Default)]
pub struct StressState {
    pub count: usize,
    pub mode: StressMode,
    pub zoom: f32,
    pub ready: usize,
    pub active: bool,
    pub generation: u64,
    pub last_error: Option<String>,
}

/// Requests a marker-mesh reload after the user switches between the full and
/// LOD variants at runtime.
#[derive(Resource, Default)]
pub struct MarkerModelReload {
    pub pending: bool,
    pub respawn: Option<StressRun>,
}

impl MarkerModelReload {
    /// Switches the marker mesh set and re-runs the current scenario, if any.
    pub fn request(&mut self, models: &mut MarkerModelSet, state: &StressState, lod: bool) {
        *models = MarkerModelSet::for_lod(lod);
        self.pending = true;
        self.respawn = (state.count > 0).then_some(StressRun {
            count: state.count,
            mode: state.mode,
            benchmark: false,
            warmup_secs: 3.0,
            sample_secs: 8.0,
            exit_when_done: false,
            zoom: state.zoom.max(0.1),
            pitch_degrees: None,
            bearing_degrees: None,
        });
    }
}

#[derive(Component)]
pub struct StressMarker {
    pub animated: bool,
    pub phase: f32,
    pub index: usize,
}

#[derive(Component)]
pub struct BakeTemplate;
