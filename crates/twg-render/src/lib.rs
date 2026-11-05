use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;

mod map;
mod stress;

pub use map::{
    CAMERA_FOV_Y, BaseLayerResource, DemoMarker, MapCamera, MapCameraResource, MapCursorResource,
    MapRenderPlugin, Tile, TileWorld,
};
pub use stress::{
    ANIMATED_MODEL, BenchPhase, BenchResult, DEFAULT_STRESS_ZOOM, DracoProbe, STATIC_MODEL,
    STRESS_COUNTS, StressBenchmark, StressMode, StressPlugin, StressRequest, StressRun, StressState,
};

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
