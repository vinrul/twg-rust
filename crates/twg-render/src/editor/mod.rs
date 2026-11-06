mod mesh;
mod scene;

use bevy::prelude::*;

pub use scene::{
    EditorGizmo, EditorGizmoAxis, EditorGrid, EditorMode, EditorScene, EditorSceneKind,
    EditorSceneObject,
};

/// Renders the editor scene graph on the tactical map.
pub struct EditorRenderPlugin;

impl Plugin for EditorRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorScene>()
            .init_resource::<EditorMode>()
            .init_resource::<scene::EditorRenderCache>()
            .add_systems(
                Update,
                (scene::sync_editor_scene, scene::update_gizmo_scale)
                    .chain()
                    .before(crate::map::MapUpdateSet),
            );
    }
}
