use std::sync::Arc;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use twg_client::{ApiClient, WsTransport};
use twg_render::UiPointerState;

mod compass_panel;
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
        if let Some(backend) = &self.backend {
            app.insert_resource(ClientUi::new(
                backend.api.clone(),
                backend.ws_transport.clone(),
                backend.base_url.clone(),
            ));
            app.add_systems(
                EguiPrimaryContextPass,
                (
                    apply_theme,
                    track_ui_pointer,
                    draw_client_shell,
                    draw_map_panels.run_if(map_screen_active),
                )
                    .chain(),
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

fn track_ui_pointer(mut contexts: EguiContexts, mut state: ResMut<UiPointerState>) -> Result {
    let context = contexts.ctx_mut()?;
    state.over_ui = context.egui_wants_pointer_input() || context.egui_is_using_pointer();
    Ok(())
}

fn draw_client_shell(mut contexts: EguiContexts, mut client: ResMut<ClientUi>) -> Result {
    let context = contexts.ctx_mut()?;
    client.poll();
    screens::draw(context, &mut client);
    Ok(())
}

fn map_screen_active(client: Option<Res<ClientUi>>) -> bool {
    client
        .map(|client| {
            matches!(
                client.state.screen,
                twg_client::session::Screen::Preview | twg_client::session::Screen::MapEditor
            )
        })
        .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
fn draw_map_panels(
    mut contexts: EguiContexts,
    mut layer: ResMut<twg_render::BaseLayerResource>,
    mut camera: ResMut<twg_render::MapCameraResource>,
    mut flight: ResMut<twg_render::CameraFlightState>,
    cursor: Res<twg_render::MapCursorResource>,
    mut stress: ResMut<twg_render::StressRequest>,
    stress_state: Res<twg_render::StressState>,
    benchmark: Res<twg_render::StressBenchmark>,
    mut models: ResMut<twg_render::MarkerModelSet>,
    mut reload: ResMut<twg_render::MarkerModelReload>,
    memory: Res<twg_render::MemoryStats>,
    selection: Res<twg_render::SelectionState>,
    diagnostics: Res<DiagnosticsStore>,
    mut count_index: Local<usize>,
    mut stress_mode: Local<twg_render::StressMode>,
) -> Result {
    let context = contexts.ctx_mut()?;
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or_default();
    compass_panel::draw_compass_ctx(context, &mut camera, &mut flight);
    map_panel::draw_map_panel_ctx(context, &mut layer, &camera, &cursor, fps);
    stress_panel::draw_stress_panel_ctx(
        context,
        &mut stress,
        &stress_state,
        &benchmark,
        &mut models,
        &mut reload,
        &memory,
        &selection,
        fps,
        &mut count_index,
        &mut stress_mode,
    );
    Ok(())
}
