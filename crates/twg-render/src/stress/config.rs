use bevy::prelude::*;
use std::path::Path;

/// Name of the Bevy asset source that maps to the offline asset cache. The
/// desktop app registers it in `main.rs` before adding `DefaultPlugins`.
pub const CACHE_ASSET_SOURCE: &str = "twg-cache";

pub const STRESS_COUNTS: [usize; 3] = [1_000, 5_000, 10_000];
pub const DEFAULT_STRESS_ZOOM: f32 = 19.0;
pub const MARKER_MODEL_SCALE: f32 = 0.02;
pub const ANIMATED_MODEL: &str = "models/polwan_airud_walk_non_draco.glb";
pub const STATIC_MODEL: &str = "models/polwan_airud_idle_non_draco.glb";
pub const ANIMATED_MODEL_LOD: &str = "models/polwan_airud_walk_lod2.glb";
pub const STATIC_MODEL_LOD: &str = "models/polwan_airud_idle_lod2.glb";
pub const ANIMATED_MODEL_DRACO: &str = "models/polwan_airud_walk_draco.glb";
pub const STATIC_MODEL_DRACO: &str = "models/polwan_airud_idle_draco.glb";
pub const ANIMATED_MODEL_LOD_DRACO: &str = "models/polwan_airud_walk_lod2_draco.glb";
pub const STATIC_MODEL_LOD_DRACO: &str = "models/polwan_airud_idle_lod2_draco.glb";

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
    pub draco: bool,
}

impl MarkerModelSet {
    pub const FULL: Self = Self {
        animated: ANIMATED_MODEL,
        still: STATIC_MODEL,
        lod: false,
        draco: false,
    };

    pub const LOD: Self = Self {
        animated: ANIMATED_MODEL_LOD,
        still: STATIC_MODEL_LOD,
        lod: true,
        draco: false,
    };

    pub const FULL_DRACO: Self = Self {
        animated: ANIMATED_MODEL_DRACO,
        still: STATIC_MODEL_DRACO,
        lod: false,
        draco: true,
    };

    pub const LOD_DRACO: Self = Self {
        animated: ANIMATED_MODEL_LOD_DRACO,
        still: STATIC_MODEL_LOD_DRACO,
        lod: true,
        draco: true,
    };

    pub const fn for_variant(lod: bool, draco: bool) -> Self {
        match (lod, draco) {
            (false, false) => Self::FULL,
            (true, false) => Self::LOD,
            (false, true) => Self::FULL_DRACO,
            (true, true) => Self::LOD_DRACO,
        }
    }

    pub const fn for_lod(lod: bool) -> Self {
        Self::for_variant(lod, false)
    }
}

impl Default for MarkerModelSet {
    fn default() -> Self {
        // The browser needs the lower-detail proxy to reach the Gate G1 FPS
        // floor with 10.000 markers. Draco is a transfer-size optimisation
        // only, so it stays opt-in on both targets.
        Self::for_variant(cfg!(target_arch = "wasm32"), false)
    }
}

/// One glTF path selected from the asset library, already pointing at the
/// offline cache through [`CACHE_ASSET_SOURCE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerModelPath {
    pub path: String,
    pub draco: bool,
}

/// Overrides the built-in marker models with files picked in the Aset screen.
/// When unset, rendering falls back to [`MarkerModelSet`].
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct MarkerModelOverride {
    /// Model used for animated markers (slot `animated`).
    pub animated: Option<MarkerModelPath>,
    /// Model used for static markers (slot `static` or `idle`).
    pub still: Option<MarkerModelPath>,
}

impl MarkerModelOverride {
    pub fn is_set(&self) -> bool {
        self.animated.is_some() || self.still.is_some()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Path and Draco flag for one slot, falling back to the built-in set.
    pub fn resolve(&self, models: &MarkerModelSet, animated: bool) -> (String, bool) {
        let selected = if animated {
            self.animated.as_ref()
        } else {
            self.still.as_ref()
        };
        match selected {
            Some(model) => (model.path.clone(), model.draco),
            None => {
                let path = if animated {
                    models.animated
                } else {
                    models.still
                };
                (path.to_owned(), models.draco)
            }
        }
    }
}

/// Converts an absolute file from the offline cache into a Bevy asset path on
/// the [`CACHE_ASSET_SOURCE`] source, e.g.
/// `twg-cache://<asset-id>/animated/draco_lod2.glb`.
pub fn cache_asset_path(cache_root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(cache_root).ok()?;
    let relative = relative.to_string_lossy().replace('\\', "/");
    if relative.is_empty() {
        return None;
    }
    Some(format!("{CACHE_ASSET_SOURCE}://{relative}"))
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
    pub fn request(
        &mut self,
        models: &mut MarkerModelSet,
        state: &StressState,
        lod: bool,
        draco: bool,
    ) {
        *models = MarkerModelSet::for_variant(lod, draco);
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

#[cfg(test)]
mod tests {
    use super::{
        CACHE_ASSET_SOURCE, MarkerModelOverride, MarkerModelPath, MarkerModelSet, cache_asset_path,
    };
    use bevy::asset::AssetPath;
    use std::path::Path;

    #[test]
    fn cache_paths_become_asset_source_paths() {
        let root = Path::new("/Users/me/Library/Caches/twg/assets");
        let file = root.join("0f8fad5b-d9cb-469f-a165-70867728950e/animated/draco_lod2.glb");
        let asset_path = cache_asset_path(root, &file).expect("path inside cache");
        assert_eq!(
            asset_path,
            "twg-cache://0f8fad5b-d9cb-469f-a165-70867728950e/animated/draco_lod2.glb"
        );
        let parsed = AssetPath::parse(&asset_path);
        assert_eq!(parsed.source().as_str(), Some(CACHE_ASSET_SOURCE));
        assert_eq!(
            parsed.path().to_string_lossy(),
            "0f8fad5b-d9cb-469f-a165-70867728950e/animated/draco_lod2.glb"
        );
        assert!(cache_asset_path(root, Path::new("/tmp/elsewhere.glb")).is_none());
    }

    #[test]
    fn override_resolves_selected_models_first() {
        let models = MarkerModelSet::LOD;
        let empty = MarkerModelOverride::default();
        assert_eq!(
            empty.resolve(&models, true),
            (models.animated.to_owned(), false)
        );
        assert_eq!(
            empty.resolve(&models, false),
            (models.still.to_owned(), false)
        );

        let selected = MarkerModelOverride {
            animated: Some(MarkerModelPath {
                path: "twg-cache://a/animated/draco_lod2.glb".to_owned(),
                draco: true,
            }),
            still: Some(MarkerModelPath {
                path: "twg-cache://a/idle/original.glb".to_owned(),
                draco: false,
            }),
        };
        assert_eq!(
            selected.resolve(&models, true),
            ("twg-cache://a/animated/draco_lod2.glb".to_owned(), true)
        );
        assert_eq!(
            selected.resolve(&models, false),
            ("twg-cache://a/idle/original.glb".to_owned(), false)
        );
    }
}
