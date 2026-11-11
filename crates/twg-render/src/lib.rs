use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;

mod editor;
mod map;
mod preview;
mod stress;

pub use editor::{
    EditorGizmo, EditorGizmoAxis, EditorGizmoMode, EditorGrid, EditorMode, EditorModelShading,
    EditorParticleKind, EditorParticleSettings, EditorPlayFrame, EditorPlayModel, EditorPlayUnit,
    EditorRenderPlugin, EditorScene, EditorSceneKind, EditorSceneObject, EditorShapeKind,
};
pub use map::{
    BaseLayerResource, CAMERA_FOV_Y, CameraFlightState, DemoMarker, GeoAnchor, MapCamera,
    MapCameraResource, MapCursorResource, MapRenderPlugin, MapUpdateSet, Tile, TileProxyConfig,
    TileWorld, UiPointerState,
};
pub use preview::{
    AssetPreview, AssetPreviewPlugin, AssetPreviewRequest, ImagePreviewKey, ModelPreviewKey,
    PREVIEW_LAYER_BASE, PREVIEW_SIZE, PREVIEW_SLOTS, PreviewOrbitInput, PreviewOrbitInputs,
    PreviewPhase, PreviewSource,
};
pub use stress::{
    ANIMATED_MODEL, ANIMATED_MODEL_DRACO, ANIMATED_MODEL_LOD, ANIMATED_MODEL_LOD_DRACO, BenchPhase,
    BenchResult, CACHE_ASSET_SOURCE, DEFAULT_STRESS_ZOOM, DracoProbe, MarkerModelOverride,
    MarkerModelPath, MarkerModelReload, MarkerModelSet, MemoryProbe, MemoryProbePhase, MemoryStats,
    STATIC_MODEL, STATIC_MODEL_DRACO, STATIC_MODEL_LOD, STATIC_MODEL_LOD_DRACO, STRESS_COUNTS,
    SelectionState, StressBenchmark, StressMode, StressPlugin, StressRequest, StressRun,
    StressState, cache_asset_path,
};
pub use twg_map::{CameraFlight, MapCameraState};

pub struct TacticalRenderPlugin;

impl Plugin for TacticalRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.025, 0.045, 0.075)))
            .add_plugins((
                FrameTimeDiagnosticsPlugin::default(),
                twg_gltf_draco::GltfDracoDecoderPlugin,
                MapRenderPlugin,
                EditorRenderPlugin,
                StressPlugin,
                AssetPreviewPlugin,
            ));
    }
}
