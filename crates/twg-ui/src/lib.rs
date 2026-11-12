use std::sync::Arc;

use bevy::prelude::*;
use bevy::window::FileDragAndDrop;
use bevy_egui::{
    EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, EguiTextureHandle,
    PrimaryEguiContext,
};
use std::path::PathBuf;
use twg_client::{ApiClient, WsTransport};
use twg_contracts::assets::{AssetSourceSlot, AssetVariantKind};
#[cfg(not(target_arch = "wasm32"))]
use twg_render::PreviewPhase;
use twg_render::{
    AssetPreview, AssetPreviewRequest, MapUpdateSet, MarkerModelOverride, MarkerModelPath,
    MarkerModelReload, PREVIEW_SLOTS, PreviewOrbitInput, PreviewOrbitInputs, TileProxyConfig,
    UiPointerState, cache_asset_path,
};
#[cfg(not(target_arch = "wasm32"))]
use uuid::Uuid;

mod asset_groups_screen;
mod assets_screen;
mod compass_panel;
pub mod editor;
mod file_picker;
mod map_panel;
mod scenario_preview;
mod screens;
pub mod state;
mod stress_panel;
pub mod theme;
pub mod widgets;

pub use state::ClientUi;
#[cfg(not(target_arch = "wasm32"))]
use state::ToastKind;

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
        // The preview camera renders to an offscreen texture. Do not let
        // bevy_egui attach the UI context to it just because it is one of the
        // first cameras in the world.
        if let Some(mut settings) = app.world_mut().get_resource_mut::<EguiGlobalSettings>() {
            settings.auto_create_primary_context = false;
        }
        app.init_resource::<editor::input::EditorInputFocus>();
        app.init_resource::<ActiveAsset>();
        app.add_systems(PostStartup, attach_primary_egui_context);
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
                    sync_tile_proxy,
                    collect_file_drops,
                    publish_preview_request,
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

fn sync_tile_proxy(proxy: Res<TileProxyConfig>, mut client: ResMut<ClientUi>) {
    if client.tile_proxy != proxy.0 {
        client.tile_proxy.clone_from(&proxy.0);
        client.scenario_preview.clear_tile_cache();
    }
}

/// Attaches the egui primary context to the camera that renders to the window.
/// The preview camera renders to an offscreen image and must never receive it.
fn attach_primary_egui_context(
    mut commands: Commands,
    map_camera: Query<Entity, With<twg_render::MapCamera>>,
) {
    if let Some(entity) = map_camera.iter().next() {
        commands.entity(entity).insert(PrimaryEguiContext);
    } else {
        commands.spawn((Camera2d, PrimaryEguiContext));
    }
}

fn track_ui_pointer(
    mut contexts: EguiContexts,
    mut state: ResMut<UiPointerState>,
    client: Option<Res<ClientUi>>,
    mut focus: ResMut<editor::input::EditorInputFocus>,
) -> Result {
    let context = contexts.ctx_mut()?;
    let mut over_ui = context.egui_wants_pointer_input() || context.egui_is_using_pointer();
    // The scenario editor lays its dock panels out as plain `Ui`s, which egui's
    // area hit test does not see. Anything outside the map viewport counts as
    // UI so wheel/drag input does not leak through to the map.
    if let Some(client) = client
        && client.state.screen == twg_client::session::Screen::MapEditor
        && let Some(position) = context.input(|input| input.pointer.latest_pos())
        && !client.editor.viewport_rect.contains(position)
    {
        over_ui = true;
    }
    state.over_ui = over_ui;
    state.keyboard = context.text_edit_focused();
    focus.keyboard = state.keyboard;
    Ok(())
}

fn draw_client_shell(
    mut contexts: EguiContexts,
    mut client: ResMut<ClientUi>,
    mut commands: Commands,
    preview: Option<Res<AssetPreview>>,
    orbit: Option<ResMut<PreviewOrbitInputs>>,
) -> Result {
    client.poll();
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(asset_id) = client.asset_preview_capture.take() {
        request_asset_preview_capture(&mut commands, &mut client, preview.as_deref(), asset_id);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = client.asset_preview_capture.take();
        let _ = &mut commands;
    }
    if let Some(preview) = preview {
        let mut panels = Vec::with_capacity(PREVIEW_SLOTS);
        for (slot, preview_slot) in preview.slots.iter().enumerate() {
            let texture = preview_slot
                .image
                .as_ref()
                .map(|image| contexts.add_image(EguiTextureHandle::Weak(image.id())));
            panels.push(state::PreviewPanelUi {
                slot,
                asset_id: preview_slot.asset_id,
                phase: preview_slot.phase,
                error: preview_slot.error.clone(),
                texture,
                dimensions: preview_slot.dimensions,
                label: preview_slot.label.clone(),
                is_model: preview_slot.is_model(),
            });
        }
        client.preview.panels = panels;
    } else {
        client.preview.panels.clear();
    }
    let context = contexts.ctx_mut()?;
    screens::draw(context, &mut client);
    if let Some(mut orbit) = orbit {
        orbit.0 = client.preview_orbit;
        client.preview_orbit = [PreviewOrbitInput::default(); PREVIEW_SLOTS];
    }
    Ok(())
}

/// Captures the offscreen asset preview render target to a timestamped PNG in
/// the offline cache. The editor tooltip looks up the newest file per asset.
#[cfg(not(target_arch = "wasm32"))]
fn request_asset_preview_capture(
    commands: &mut Commands,
    client: &mut ClientUi,
    preview: Option<&AssetPreview>,
    asset_id: Uuid,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};

    let Some(preview) = preview else {
        client.toast("Preview aset tidak tersedia.", ToastKind::Error);
        return;
    };
    let Some(slot) = preview
        .slots
        .iter()
        .find(|slot| slot.asset_id == Some(asset_id))
    else {
        client.toast(
            "Preview aset belum dimuat. Buka detail aset terlebih dahulu.",
            ToastKind::Error,
        );
        return;
    };
    if slot.phase != PreviewPhase::Ready {
        client.toast(
            "Preview aset belum siap; coba lagi sebentar.",
            ToastKind::Error,
        );
        return;
    }
    let Some(image) = slot.image.clone() else {
        client.toast("Preview aset belum siap.", ToastKind::Error);
        return;
    };
    let Some(root) = client.asset_cache_root() else {
        client.toast("Cache aset lokal tidak tersedia.", ToastKind::Error);
        return;
    };
    let dir = std::path::Path::new(&root).join("previews");
    if let Err(error) = std::fs::create_dir_all(&dir) {
        client.toast(
            format!("Gagal membuat folder preview: {error}"),
            ToastKind::Error,
        );
        return;
    }
    // Only the newest screenshot per asset is used by the editor tooltip.
    let prefix = format!("{asset_id}_");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();
    let path = dir.join(format!("{asset_id}_{stamp}.png"));
    client
        .pending_preview_uploads
        .push((asset_id, path.clone()));
    commands
        .spawn(Screenshot::image(image))
        .observe(save_to_disk(path));
    client.toast(
        "Preview aset dibuat; mengunggah ke server...",
        ToastKind::Info,
    );
}

/// Feeds window drag & drop messages into the asset upload form.
fn collect_file_drops(mut events: MessageReader<FileDragAndDrop>, mut client: ResMut<ClientUi>) {
    for event in events.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = event {
            client.push_dropped_file(path_buf.clone());
        }
    }
}

/// Hands the selected asset's server URL to the renderer that draws the
/// preview. The offline cache is deliberately not used here.
fn publish_preview_request(client: Res<ClientUi>, mut request: ResMut<AssetPreviewRequest>) {
    let wanted = if client.state.nav == twg_client::session::NavItem::Assets {
        client.preview_wanted.clone()
    } else {
        std::array::from_fn(|_| None)
    };
    if request.0 != wanted {
        request.0 = wanted;
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
