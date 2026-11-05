use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use twg_render::UiPointerState;

mod compass_panel;
mod map_panel;
mod stress_panel;

pub struct TacticalUiPlugin;

impl Plugin for TacticalUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default()).add_systems(
            EguiPrimaryContextPass,
            (
                track_ui_pointer,
                compass_panel::draw_compass,
                map_panel::draw_map_panel,
                stress_panel::draw_stress_panel,
            )
                .chain(),
        );
    }
}

fn track_ui_pointer(mut contexts: EguiContexts, mut state: ResMut<UiPointerState>) -> Result {
    let context = contexts.ctx_mut()?;
    state.over_ui = context.egui_wants_pointer_input() || context.egui_is_using_pointer();
    Ok(())
}
