pub(crate) mod mesh;
mod scene;

use bevy::prelude::*;

pub use scene::{
    EditorGizmo, EditorGizmoAxis, EditorGizmoMode, EditorGrid, EditorMode, EditorModelShading,
    EditorParticleKind, EditorParticleSettings, EditorPlayFrame, EditorPlayModel, EditorPlayUnit,
    EditorScene, EditorSceneKind, EditorSceneObject, EditorShapeKind,
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
            .init_resource::<crate::RenderLoadStatus>()
            .add_systems(
                Update,
                (
                    scene::sync_editor_scene,
                    scene::tag_gizmo_parts,
                    scene::sync_editor_play_scenes,
                    scene::apply_editor_play_frame,
                    scene::follow_play_gizmo,
                    scene::sync_editor_play_animations,
                    scene::update_gizmo_scale,
                    scene::normalize_marker_meshes,
                    scene::apply_marker_shading,
                    scene::track_marker_loads,
                )
                    .chain()
                    .before(crate::map::MapUpdateSet),
            )
            // Particle billboards need the camera pose updated by the map set
            // in the same frame, so they run after it.
            .add_systems(
                Update,
                scene::animate_particle_effects.after(crate::map::MapUpdateSet),
            );
    }
}
