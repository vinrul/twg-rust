use std::sync::Arc;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use twg_client::{ApiClient, WsTransport};
use twg_render::{MapUpdateSet, UiPointerState};

mod compass_panel;
pub mod editor;
mod map_panel;
mod screens;
pub mod state;
mod stress_panel;
pub mod theme;
pub mod widgets;

pub use state::ClientUi;

pub struct ClientBackend {
    pub api: ApiClient,
    pub ws_transport: Arc<dyn WsTransport>,
    pub base_url: String,
}

/// Adds the egui shell. Without a backend the plugin runs the map demo only,
/// which keeps the Gate G1 stress harness working.
pub struct TacticalUiPlugin {
    backend: Option<ClientBackend>,
}

impl TacticalUiPlugin {
    pub fn new(backend: ClientBackend) -> Self {
        Self {
            backend: Some(backend),
        }
    }

    pub fn map_demo() -> Self {
        Self { backend: None }
    }
}

impl Default for TacticalUiPlugin {
    fn default() -> Self {
        Self::map_demo()
    }
}

impl Plugin for TacticalUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default());
        app.init_resource::<editor::input::EditorInputFocus>();
        if let Some(backend) = &self.backend {
            app.insert_resource(ClientUi::new(
                backend.api.clone(),
                backend.ws_transport.clone(),
                backend.base_url.clone(),
            ));
            app.add_systems(
                Update,
                (editor::input::sync_editor_mode, editor::input::editor_input)
                    .chain()
                    .after(MapUpdateSet),
            );
            app.add_systems(
                EguiPrimaryContextPass,
                (apply_theme, draw_client_shell, track_ui_pointer).chain(),
            );
        } else {
            app.add_systems(
                EguiPrimaryContextPass,
                (
                    apply_theme,
                    track_ui_pointer,
                    compass_panel::draw_compass,
                    map_panel::draw_map_panel,
                    stress_panel::draw_stress_panel,
                )
                    .chain(),
            );
        }
    }
}

fn apply_theme(mut contexts: EguiContexts, mut applied: Local<bool>) -> Result {
    let context = contexts.ctx_mut()?;
    if !*applied {
        theme::apply(context);
        *applied = true;
    }
    Ok(())
}

fn track_ui_pointer(
    mut contexts: EguiContexts,
    mut state: ResMut<UiPointerState>,
    mut focus: ResMut<editor::input::EditorInputFocus>,
) -> Result {
    let context = contexts.ctx_mut()?;
    state.over_ui = context.egui_wants_pointer_input() || context.egui_is_using_pointer();
    focus.keyboard = context.text_edit_focused();
    Ok(())
}

fn draw_client_shell(mut contexts: EguiContexts, mut client: ResMut<ClientUi>) -> Result {
    let context = contexts.ctx_mut()?;
    client.poll();
    screens::draw(context, &mut client);
    Ok(())
}
