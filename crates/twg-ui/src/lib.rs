use std::sync::Arc;

use bevy::prelude::*;
use bevy::window::FileDragAndDrop;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use std::path::PathBuf;
use twg_client::{ApiClient, WsTransport};
use twg_contracts::assets::{AssetSourceSlot, AssetVariantKind};
use twg_render::{
    MapUpdateSet, MarkerModelOverride, MarkerModelPath, MarkerModelReload, UiPointerState,
    cache_asset_path,
};

mod assets_screen;
mod compass_panel;
pub mod editor;
mod map_panel;
mod screens;
pub mod state;
mod stress_panel;
pub mod theme;
pub mod widgets;

pub use state::ClientUi;

/// Asset selected with **Gunakan**, resolved to offline cache paths. Editors
/// and gameplay systems read this resource instead of loading from the server.
#[derive(Resource, Default, Clone, Debug)]
pub struct ActiveAsset(pub Option<state::UsedAsset>);

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
        app.init_resource::<ActiveAsset>();
        if let Some(backend) = &self.backend {
            app.insert_resource(ClientUi::new(
                backend.api.clone(),
                backend.ws_transport.clone(),
                backend.base_url.clone(),
            ));
            app.add_systems(
                Update,
                (
                    editor::input::sync_editor_mode,
                    editor::input::editor_input,
                    collect_file_drops,
                    sync_active_asset,
                )
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

/// Feeds window drag & drop messages into the asset upload form.
fn collect_file_drops(mut events: MessageReader<FileDragAndDrop>, mut client: ResMut<ClientUi>) {
    for event in events.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = event {
            client.push_dropped_file(path_buf.clone());
        }
    }
}

/// Publishes the **Gunakan** selection so render/editor systems can load the
/// offline files without talking to the UI state, and swaps the marker models
/// over to those files.
fn sync_active_asset(
    client: Res<ClientUi>,
    mut active: ResMut<ActiveAsset>,
    mut models: ResMut<MarkerModelOverride>,
    mut reload: ResMut<MarkerModelReload>,
) {
    if active.0 == client.used_asset {
        return;
    }
    active.0 = client.used_asset.clone();
    let next = marker_override(&client);
    if *models != next {
        *models = next;
        reload.pending = true;
    }
}

/// Maps the selected asset's offline files to the marker model override.
fn marker_override(client: &ClientUi) -> MarkerModelOverride {
    let Some(used) = &client.used_asset else {
        return MarkerModelOverride::default();
    };
    let Some(root) = client.asset_cache_root().map(PathBuf::from) else {
        return MarkerModelOverride::default();
    };
    let mut result = MarkerModelOverride::default();
    for file in &used.files {
        let Some(path) = cache_asset_path(&root, &file.path) else {
            continue;
        };
        let entry = MarkerModelPath {
            path,
            draco: matches!(
                file.variant,
                Some(AssetVariantKind::Draco) | Some(AssetVariantKind::DracoLod2)
            ),
        };
        match file.slot {
            AssetSourceSlot::Static | AssetSourceSlot::Idle => result.still = Some(entry),
            AssetSourceSlot::Animated => result.animated = Some(entry),
        }
    }
    result
}
