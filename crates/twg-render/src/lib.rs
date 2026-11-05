use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;

mod map;
mod stress;

pub use map::{
    BaseLayerResource, CAMERA_FOV_Y, CameraFlightState, DemoMarker, GeoAnchor, MapCamera,
    MapCameraResource, MapCursorResource, MapRenderPlugin, Tile, TileProxyConfig, TileWorld,
    UiPointerState,
};
pub use stress::{
    ANIMATED_MODEL, ANIMATED_MODEL_LOD, BenchPhase, BenchResult, DEFAULT_STRESS_ZOOM, DracoProbe,
    MarkerModelReload, MarkerModelSet, MemoryProbe, MemoryProbePhase, MemoryStats, STATIC_MODEL,
    STATIC_MODEL_LOD, STRESS_COUNTS, SelectionState, StressBenchmark, StressMode, StressPlugin,
    StressRequest, StressRun, StressState,
};
pub use twg_map::{CameraFlight, MapCameraState};

pub struct TacticalRenderPlugin;

impl Plugin for TacticalRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.025, 0.045, 0.075)))
            .add_plugins((
                FrameTimeDiagnosticsPlugin::default(),
                MapRenderPlugin,
                StressPlugin,
            ));
    }
}
