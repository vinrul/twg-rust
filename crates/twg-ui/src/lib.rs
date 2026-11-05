use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

mod map_panel;
mod stress_panel;

pub struct TacticalUiPlugin;

impl Plugin for TacticalUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default()).add_systems(
            EguiPrimaryContextPass,
            (map_panel::draw_map_panel, stress_panel::draw_stress_panel),
        );
    }
}
