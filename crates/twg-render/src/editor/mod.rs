pub(crate) mod mesh;
mod scene;

use bevy::prelude::*;

pub use scene::{
    EditorGizmo, EditorGizmoAxis, EditorGizmoMode, EditorGrid, EditorMode, EditorModelShading,
    EditorPlayFrame, EditorPlayModel, EditorPlayUnit, EditorScene, EditorSceneKind,
    EditorSceneObject, EditorShapeKind,
};

/// Renders the editor scene graph on the tactical map.
pub struct EditorRenderPlugin;

impl Plugin for EditorRenderPlugin {
    fn build(&self, app: &mut App) {
        // Wireframes rely on a native-only render pipeline.
        #[cfg(not(target_arch = "wasm32"))]
        app.add_plugins(bevy::pbr::wireframe::WireframePlugin::default());
        app.init_resource::<EditorScene>()
            .init_resource::<EditorMode>()
            .init_resource::<scene::EditorPlayFrame>()
            .init_resource::<scene::EditorRenderCache>()
            .add_systems(
                Update,
                (
                    scene::sync_editor_scene,
                    scene::sync_editor_play_scenes,
                    scene::apply_editor_play_frame,
                    scene::sync_editor_play_animations,
                    scene::update_gizmo_scale,
                    scene::normalize_marker_meshes,
                    scene::apply_marker_shading,
                )
                    .chain()
                    .before(crate::map::MapUpdateSet),
            );
    }
}
