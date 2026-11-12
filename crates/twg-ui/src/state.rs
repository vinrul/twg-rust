use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_egui::egui;
use chrono::NaiveDate;
use serde_json::{Value, json};
use twg_client::ApiClient;
use twg_client::api::{ApiError, ApiHandle, ApiResponse, HttpMethod};
#[cfg(not(target_arch = "wasm32"))]
use twg_client::asset_cache::AssetCache;
use twg_client::realtime::{RealtimeState, RealtimeStatus, WsConnection, WsTransport};
use twg_client::session::{ClientSession, ClientState, NavItem, Screen};
#[cfg(not(target_arch = "wasm32"))]
use twg_contracts::assets::AssetSource;
use twg_contracts::assets::{
    Asset, AssetCategory, AssetGroup, AssetManifest, AssetModelKind, AssetMoveMode,
    AssetSourceSlot, AssetVariantKind, ConvertRequest, ConvertResult, ConvertTarget, SyncProgress,
};
use twg_contracts::categories::Category;
use twg_contracts::operations::{HomeOperation, Operation};
use twg_contracts::satkers::Satker;
use twg_contracts::scenarios::Scenario;
use twg_contracts::snapshots::Snapshot;
use twg_contracts::users::{OnlineUser, User};
use twg_domain::Role;
#[cfg(not(target_arch = "wasm32"))]
use twg_render::{ImagePreviewKey, ModelPreviewKey};
use twg_render::{PREVIEW_SLOTS, PreviewOrbitInput, PreviewPhase, PreviewSource};
use uuid::Uuid;

use crate::editor::model::EditorState;
use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub remaining: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Identify,
    Login,
    Logout,
    Operations,
    Scenarios,
    Users,
    Satkers,
    Online,
    Home,
    Categories,
    OperationSave,
    OperationDelete,
    ScenarioSave,
    ScenarioDelete,
    ScenarioCopy,
    UserSave,
    UserDelete,
    SatkerSave,
    SatkerDelete,
    ProfileSave,
    PasswordChange,
    SnapshotLoad,
    SnapshotToggle,
    SnapshotDelete,
    WsTicket,
    Assets,
    AssetManifest,
    Imports,
    ImportUpload,
    ImportDelete,
    AssetUpload,
    AssetPreviewUpload,
    AssetRename,
    AssetReplaceSource,
    AssetConvert,
    AssetDelete,
    AssetGroups,
    AssetGroupSave,
    AssetGroupDelete,
}

pub struct UiJob {
    pub kind: JobKind,
    pub handle: ApiHandle,
}

pub struct OperationForm {
    pub id: Option<Uuid>,
    pub name: String,
    pub satker_id: String,
    pub error: Option<String>,
}

pub struct ScenarioForm {
    pub id: Option<Uuid>,
    pub operasi_id: String,
    pub target_operasi_id: String,
    pub name: String,
    /// Longitude (server `center_x`).
    pub center_x: f64,
    /// Latitude (server `center_y`).
    pub center_y: f64,
    pub zoom: f64,
    pub min_zoom: f64,
    pub max_zoom: f64,
    pub pitch: f64,
    pub bearing: f64,
    pub map_mode: String,
    pub base_layer: String,
    /// Metres between grid lines for indoor grid layers.
    pub grid_size_m: f64,
    pub indoor_config: String,
    pub copy: bool,
    pub error: Option<String>,
}

pub struct UserForm {
    pub id: Option<Uuid>,
    pub name: String,
    pub username: String,
    pub password: String,
    pub confirm_password: String,
    pub password_visible: bool,
    pub confirm_password_visible: bool,
    pub units: String,
    pub role: String,
    pub satker_id: String,
    pub profile_img: String,
    pub error: Option<String>,
}

pub struct SatkerForm {
    pub id: Option<Uuid>,
    pub code: String,
    pub name: String,
    pub is_active: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    DeleteOperation(Uuid),
    DeleteScenario(Uuid),
    DeleteUser(Uuid),
    DeleteSatker(Uuid),
    DeleteSnapshot(Uuid),
    DeleteAsset(Uuid),
    DeleteAssetGroup(Uuid),
    Logout,
}

pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub action: ConfirmAction,
}

// ---------------------------------------------------------------- assets

/// Upload form for the admin asset screen. Files come from window drag & drop
/// and are assigned to source slots in order.
pub struct AssetUploadDraft {
    pub category: AssetCategory,
    pub model_kind: AssetModelKind,
    /// Only used by static models; animated models are always free to move.
    pub move_mode: AssetMoveMode,
    pub group_id: Option<Uuid>,
    pub name: String,
    pub files: Vec<PathBuf>,
    pub busy: bool,
    pub error: Option<String>,
}

impl Default for AssetUploadDraft {
    fn default() -> Self {
        Self {
            category: AssetCategory::Model,
            model_kind: AssetModelKind::Static,
            move_mode: AssetMoveMode::None,
            group_id: None,
            name: String::new(),
            files: Vec::new(),
            busy: false,
            error: None,
        }
    }
}

impl AssetUploadDraft {
    pub fn required_files(&self) -> &'static [AssetSourceSlot] {
        if self.category.is_model() {
            self.model_kind.slots()
        } else {
            &[AssetSourceSlot::Static]
        }
    }

    /// Required slots that actually hold a file. Picking a slot out of order
    /// leaves empty placeholders until the missing files arrive.
    pub fn filled_files(&self) -> usize {
        self.files
            .iter()
            .take(self.required_files().len())
            .filter(|path| !path.as_os_str().is_empty())
            .count()
    }

    /// Assigns a file to one source slot without disturbing the others.
    pub fn set_file(&mut self, index: usize, path: PathBuf) {
        if self.files.len() <= index {
            self.files.resize(index + 1, PathBuf::new());
        }
        self.files[index] = path;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Pending file replacement for one source slot of an existing asset. The next
/// file dropped on the window is uploaded into `slot` instead of the upload
/// form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetReplaceTarget {
    pub asset_id: Uuid,
    pub slot: AssetSourceSlot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsedAssetFile {
    pub slot: AssetSourceSlot,
    pub path: PathBuf,
    pub variant: Option<AssetVariantKind>,
}

/// Asset explicitly selected with the **Gunakan** button. Files are resolved
/// from the offline cache at selection time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsedAsset {
    pub asset_id: Uuid,
    pub name: String,
    pub model_kind: Option<AssetModelKind>,
    pub files: Vec<UsedAssetFile>,
    pub prefer_variants: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AssetSyncState {
    #[default]
    Idle,
    Checking,
    Syncing,
    Ready,
    /// Server unreachable; the previous offline snapshot is used.
    Offline(String),
}

impl AssetSyncState {
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::Checking | Self::Syncing)
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Idle => "Belum dicek",
            Self::Checking => "Memeriksa server",
            Self::Syncing => "Menyinkronkan",
            Self::Ready => "Sinkron",
            Self::Offline(_) => "Mode offline",
        }
    }
}

#[derive(Default)]
pub struct AssetSyncUi {
    pub state: AssetSyncState,
    pub progress: Option<Arc<Mutex<SyncProgress>>>,
    pub last_sync: Option<String>,
}

/// UI-side mirror of [`twg_render::AssetPreview`] plus the egui texture ids.
#[derive(Default)]
pub struct PreviewUi {
    pub panels: Vec<PreviewPanelUi>,
}

#[derive(Clone, Default)]
pub struct PreviewPanelUi {
    pub slot: usize,
    pub asset_id: Option<Uuid>,
    pub phase: PreviewPhase,
    pub error: Option<String>,
    pub texture: Option<egui::TextureId>,
    pub dimensions: Option<(u32, u32)>,
    pub label: String,
    pub is_model: bool,
}

pub enum Modal {
    None,
    Operation(OperationForm),
    Scenario(ScenarioForm),
    User(UserForm),
    Satker(SatkerForm),
    Confirm(ConfirmDialog),
}

impl Modal {
    pub fn is_open(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn close(&mut self) {
        *self = Self::None;
    }
}

#[derive(Default)]
pub struct ClientData {
    pub operations: Vec<Operation>,
    pub scenarios: Vec<Scenario>,
    pub users: Vec<User>,
    pub satkers: Vec<Satker>,
    pub online: Vec<OnlineUser>,
    pub home: Vec<HomeOperation>,
    pub categories: Vec<Category>,
    pub asset_groups: Vec<AssetGroup>,
    pub snapshot: Option<Snapshot>,
    pub loading: HashMap<&'static str, bool>,
}

impl ClientData {
    pub fn set_loading(&mut self, key: &'static str, value: bool) {
        self.loading.insert(key, value);
    }

    pub fn is_loading(&self, key: &'static str) -> bool {
        self.loading.get(key).copied().unwrap_or(false)
    }
}

#[derive(Default)]
pub struct WsUi {
    pub connection: Option<Arc<WsConnection>>,
    pub state: RealtimeState,
    pub scenario: Option<Uuid>,
    pub next_attempt: Option<Instant>,
    pub subscribed: bool,
    pub ticket_pending: bool,
}

#[derive(Resource)]
pub struct ClientUi {
    pub state: ClientState,
    pub api: ApiClient,
    pub ws_transport: Arc<dyn WsTransport>,
    pub base_url: String,
    pub login_username: String,
    pub login_password: String,
    pub login_password_visible: bool,
    pub login_error: Option<String>,
    pub logging_in: bool,
    pub modal: Modal,
    pub toasts: Vec<Toast>,
    pub jobs: Vec<UiJob>,
    pub data: ClientData,
    pub operation_search: String,
    pub scenario_operation: String,
    pub scenario_search: String,
    pub selected_scenario: Option<Uuid>,
    pub snapshot_date: Option<NaiveDate>,
    pub profile_name: String,
    pub profile_img: String,
    pub old_password: String,
    pub new_password: String,
    pub confirm_password: String,
    pub old_password_visible: bool,
    pub new_password_visible: bool,
    pub confirm_password_visible: bool,
    pub profile_message: Option<(bool, String)>,
    pub ws: WsUi,
    pub layer_order: Vec<&'static str>,
    pub selected_layer: &'static str,
    pub boot_checked: bool,
    pub last_screen: Screen,
    pub editor: EditorState,
    pub assets: Vec<Asset>,
    /// Interactive map preview state for the scenario form (tile cache).
    pub scenario_preview: crate::scenario_preview::ScenarioPreview,
    /// Tile proxy from `TWG_TILE_PROXY`, mirrored from the render plugin.
    pub tile_proxy: Option<String>,
    /// Import assets of the operation loaded in the editor (all users of the
    /// operation share them).
    pub imports: Vec<Asset>,
    pub asset_filter: Option<AssetCategory>,
    pub asset_group_filter: Option<Uuid>,
    pub asset_search: String,
    pub selected_asset: Option<Uuid>,
    /// Name being edited in the asset detail panel.
    pub asset_edit_name: String,
    /// Category selected while editing the current asset.
    pub asset_edit_group_id: Option<Uuid>,
    /// Play move mode selected while editing the current asset.
    pub asset_edit_move_mode: AssetMoveMode,
    /// Inline form for the asset-category module.
    pub asset_group_new_name: String,
    pub asset_group_edit_id: Option<Uuid>,
    pub asset_group_edit_name: String,
    pub asset_group_error: Option<String>,
    /// Set while the user is about to drop a replacement file for a slot.
    pub asset_replace: Option<AssetReplaceTarget>,
    /// Set when the Assets screen asks for a preview screenshot.
    pub asset_preview_capture: Option<Uuid>,
    /// Screenshot files waiting for the renderer to finish writing before the
    /// upload to the server starts.
    pub pending_preview_uploads: Vec<(Uuid, PathBuf)>,
    pub asset_upload: AssetUploadDraft,
    pub asset_sync: AssetSyncUi,
    pub preview: PreviewUi,
    pub preview_wanted: [Option<PreviewSource>; PREVIEW_SLOTS],
    pub preview_orbit: [PreviewOrbitInput; PREVIEW_SLOTS],
    pub preview_retry: u64,
    pub used_asset: Option<UsedAsset>,
    pub pending_asset_nav: Option<NavItem>,
    /// A manifest check was requested while a sync was running; run it again
    /// when the current sync finishes so newly uploaded files are downloaded.
    pub pending_asset_check: bool,
    pub dropped_files: Vec<PathBuf>,
    pub asset_prefer_variants: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub asset_cache: Option<Arc<Mutex<AssetCache>>>,
    pending_editor_scenario: Option<Uuid>,
}

impl ClientUi {
    pub fn new(api: ApiClient, ws_transport: Arc<dyn WsTransport>, base_url: String) -> Self {
        let pending_editor_scenario = initial_editor_scenario();
        let mut state = ClientState::default();
        let session = api.restore();
        bevy::log::info!("client shell: restored session = {}", session.is_some());
        if session.is_some() {
            state.screen = Screen::Boot;
        } else {
            state.screen = Screen::Login;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let asset_cache = Some(Arc::new(Mutex::new(AssetCache::new())));
        // `TWG_ACTIVE_ASSET=<uuid>` starts the app with an asset already picked
        // from the offline cache (useful for launching a simulation).
        #[cfg(not(target_arch = "wasm32"))]
        let used_asset = asset_cache.as_ref().and_then(env_selected_asset);
        #[cfg(target_arch = "wasm32")]
        let used_asset = None;
        Self {
            state,
            api,
            ws_transport,
            base_url,
            login_username: String::new(),
            login_password: String::new(),
            login_password_visible: false,
            login_error: None,
            logging_in: false,
            modal: Modal::None,
            toasts: Vec::new(),
            jobs: Vec::new(),
            data: ClientData::default(),
            operation_search: String::new(),
            scenario_operation: String::new(),
            scenario_search: String::new(),
            selected_scenario: None,
            snapshot_date: None,
            profile_name: String::new(),
            profile_img: String::new(),
            old_password: String::new(),
            new_password: String::new(),
            confirm_password: String::new(),
            old_password_visible: false,
            new_password_visible: false,
            confirm_password_visible: false,
            profile_message: None,
            ws: WsUi::default(),
            layer_order: vec!["OpenStreetMap", "Satelit"],
            selected_layer: "OpenStreetMap",
            boot_checked: false,
            last_screen: Screen::Boot,
            editor: EditorState::new(),
            assets: Vec::new(),
            scenario_preview: crate::scenario_preview::ScenarioPreview::default(),
            tile_proxy: None,
            imports: Vec::new(),
            asset_filter: None,
            asset_group_filter: None,
            asset_search: String::new(),
            selected_asset: None,
            asset_edit_name: String::new(),
            asset_edit_group_id: None,
            asset_edit_move_mode: AssetMoveMode::None,
            asset_group_new_name: String::new(),
            asset_group_edit_id: None,
            asset_group_edit_name: String::new(),
            asset_group_error: None,
            asset_replace: None,
            asset_preview_capture: None,
            pending_preview_uploads: Vec::new(),
            asset_upload: AssetUploadDraft::default(),
            asset_sync: AssetSyncUi::default(),
            preview: PreviewUi::default(),
            preview_wanted: std::array::from_fn(|_| None),
            preview_orbit: [PreviewOrbitInput::default(); PREVIEW_SLOTS],
            preview_retry: 0,
            used_asset,
            pending_asset_nav: None,
            pending_asset_check: false,
            dropped_files: Vec::new(),
            asset_prefer_variants: true,
            #[cfg(not(target_arch = "wasm32"))]
            asset_cache,
            pending_editor_scenario,
        }
    }

    pub fn session(&self) -> Option<&ClientSession> {
        self.state.session.as_ref()
    }

    pub fn role(&self) -> Role {
        self.state
            .session
            .as_ref()
            .map(|session| session.role)
            .unwrap_or(Role::User)
    }

    pub fn toast(&mut self, text: impl Into<String>, kind: ToastKind) {
        self.toasts.push(Toast {
            text: text.into(),
            kind,
            remaining: 4.0,
        });
    }

    pub fn error(&mut self, error: &ApiError) {
        self.toast(
            format!("{} {}", error.message, twg_client::api::retry_hint(error)),
            ToastKind::Error,
        );
    }

    pub fn track(&mut self, kind: JobKind, handle: ApiHandle) {
        self.jobs.push(UiJob { kind, handle });
    }

    pub fn set_loading(&mut self, key: &'static str, value: bool) {
        self.data.set_loading(key, value);
    }

    pub fn sync_profile_fields(&mut self) {
        let Some(session) = &self.state.session else {
            return;
        };
        self.profile_name = session.name.clone();
        self.profile_img = session.profile_img.clone();
    }

    pub fn begin_login(&mut self) {
        if self.login_username.trim().is_empty() || self.login_password.is_empty() {
            self.login_error = Some("Username dan password wajib diisi.".to_owned());
            return;
        }
        self.login_error = None;
        self.logging_in = true;
        let handle = self
            .api
            .login(self.login_username.trim(), &self.login_password);
        self.track(JobKind::Login, handle);
    }

    pub fn begin_identify(&mut self) {
        let handle = self.api.identify();
        self.track(JobKind::Identify, handle);
    }

    pub fn logout(&mut self) {
        let handle = self.api.logout();
        self.track(JobKind::Logout, handle);
        self.state.logout();
        self.data = ClientData::default();
        self.close_ws();
        self.assets.clear();
        self.imports.clear();
        self.selected_asset = None;
        self.asset_edit_name.clear();
        self.asset_edit_group_id = None;
        self.asset_group_filter = None;
        self.asset_group_new_name.clear();
        self.asset_group_edit_id = None;
        self.asset_group_edit_name.clear();
        self.asset_group_error = None;
        self.asset_replace = None;
        self.preview = PreviewUi::default();
        self.preview_wanted = std::array::from_fn(|_| None);
        self.preview_orbit = [PreviewOrbitInput::default(); PREVIEW_SLOTS];
        self.preview_retry = 0;
        self.asset_upload.reset();
        self.asset_sync = AssetSyncUi::default();
        self.pending_asset_nav = None;
        self.used_asset = None;
        self.toast("Logout berhasil.", ToastKind::Info);
    }

    pub fn refresh_operations(&mut self) {
        self.data.set_loading("operations", true);
        let search = encode_query(&self.operation_search);
        let handle = self.api.request(
            HttpMethod::Get,
            format!("/private/operasis?pageSize=200&pageNumber=1&search={search}"),
            None,
        );
        self.track(JobKind::Operations, handle);
    }

    pub fn refresh_scenarios(&mut self) {
        let Some(operation_id) = self.effective_operation() else {
            self.data.scenarios.clear();
            return;
        };
        self.data.set_loading("scenarios", true);
        let handle = self.api.request(
            HttpMethod::Get,
            format!("/public/skenarios/{operation_id}"),
            None,
        );
        self.track(JobKind::Scenarios, handle);
    }

    pub fn effective_operation(&self) -> Option<Uuid> {
        Uuid::parse_str(&self.scenario_operation)
            .ok()
            .or_else(|| self.data.operations.first().map(|operation| operation.id))
    }

    pub fn refresh_users(&mut self) {
        if !self.role().is_admin_like() {
            return;
        }
        self.data.set_loading("users", true);
        let handle = self.api.request(HttpMethod::Get, "/private/users", None);
        self.track(JobKind::Users, handle);
    }

    pub fn refresh_satkers(&mut self) {
        self.data.set_loading("satkers", true);
        let handle = self.api.request(HttpMethod::Get, "/private/satkers", None);
        self.track(JobKind::Satkers, handle);
    }

    pub fn refresh_online(&mut self) {
        self.data.set_loading("online", true);
        let handle = self
            .api
            .request(HttpMethod::Get, "/public/users/online", None);
        self.track(JobKind::Online, handle);
    }

    pub fn refresh_home(&mut self) {
        self.data.set_loading("home", true);
        let handle = self
            .api
            .request(HttpMethod::Get, "/public/list/operasis", None);
        self.track(JobKind::Home, handle);
    }

    pub fn refresh_categories(&mut self) {
        self.data.set_loading("categories", true);
        let handle = self
            .api
            .request(HttpMethod::Get, "/private/categories/all", None);
        self.track(JobKind::Categories, handle);
    }

    pub fn refresh_all(&mut self) {
        self.refresh_home();
        self.refresh_operations();
        self.refresh_scenarios();
        self.refresh_users();
        self.refresh_satkers();
        self.refresh_online();
    }

    pub fn load_snapshot(&mut self, scenario_id: Uuid) {
        self.data.set_loading("snapshot", true);
        let handle = self.api.request(
            HttpMethod::Get,
            format!("/private/game-sessions/snapshot/{scenario_id}"),
            None,
        );
        self.track(JobKind::SnapshotLoad, handle);
    }

    pub fn poll(&mut self) {
        self.api.tick();
        let jobs = std::mem::take(&mut self.jobs);
        for job in jobs {
            let Some(result) = job.handle.try_take() else {
                self.jobs.push(job);
                continue;
            };
            self.handle_job(job.kind, result);
        }
        self.tick_toasts();
        self.tick_ws();
        self.tick_asset_sync();
        self.tick_editor_jobs();
        self.poll_pending_preview_uploads();
    }

    fn poll_pending_preview_uploads(&mut self) {
        if self.pending_preview_uploads.is_empty() {
            return;
        }
        let mut remaining = Vec::new();
        for (asset_id, path) in std::mem::take(&mut self.pending_preview_uploads) {
            if !path.is_file() {
                remaining.push((asset_id, path));
                continue;
            }
            // Import assets have their own preview endpoint (no admin role
            // required); admin-library assets keep using the admin route.
            let endpoint = if self.imports.iter().any(|asset| asset.id == asset_id) {
                format!("/private/imports/{asset_id}/preview")
            } else {
                format!("/admin/assets/{asset_id}/preview")
            };
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let handle = self.api.upload(endpoint, "image/png", bytes);
                    self.track(JobKind::AssetPreviewUpload, handle);
                }
                Err(error) => self.toast(
                    format!("Gagal membaca file preview: {error}"),
                    ToastKind::Error,
                ),
            }
        }
        self.pending_preview_uploads = remaining;
    }

    fn enter_session(&mut self, session: ClientSession) {
        self.state.login(session);
        self.sync_profile_fields();
        self.refresh_all();
        self.refresh_assets();
        // Runs on startup (restored session) and after login: mirror any new or
        // changed server assets before the editor/game is opened.
        self.request_asset_check(None);
        if let Some(scenario) = self.pending_editor_scenario.take() {
            self.open_editor(Some(scenario));
        }
    }

    fn handle_job(&mut self, kind: JobKind, result: Result<ApiResponse, ApiError>) {
        match kind {
            JobKind::Identify => {
                self.boot_checked = true;
                match result {
                    Ok(_) => {
                        if let Some(session) = self.api.session() {
                            self.enter_session(session);
                        } else {
                            self.state.screen = Screen::Login;
                        }
                    }
                    Err(_) => {
                        self.api.clear_session();
                        self.state.screen = Screen::Login;
                    }
                }
            }
            JobKind::Login => {
                self.logging_in = false;
                match result {
                    Ok(_) => {
                        if let Some(session) = self.api.session() {
                            self.login_password_visible = false;
                            self.enter_session(session);
                            self.toast("Login berhasil.", ToastKind::Success);
                        }
                    }
                    Err(error) => {
                        self.login_error = Some(error.message.clone());
                    }
                }
            }
            JobKind::Logout => {}
            JobKind::Operations => {
                self.data.set_loading("operations", false);
                match result.and_then(|response| response.list::<Operation>()) {
                    Ok((mut operations, _)) => {
                        operations.sort_by(|left, right| left.name.cmp(&right.name));
                        self.data.operations = operations;
                        if self.scenario_operation.is_empty()
                            && let Some(first) = self.data.operations.first()
                        {
                            self.scenario_operation = first.id.to_string();
                        }
                    }
                    Err(error) => self.error(&error),
                }
            }
            JobKind::Scenarios => {
                self.data.set_loading("scenarios", false);
                match result.and_then(|response| response.list::<Scenario>()) {
                    Ok((scenarios, _)) => self.data.scenarios = scenarios,
                    Err(error) => self.error(&error),
                }
            }
            JobKind::Users => {
                self.data.set_loading("users", false);
                match result.and_then(|response| response.list::<User>()) {
                    Ok((users, _)) => self.data.users = users,
                    Err(error) => self.error(&error),
                }
            }
            JobKind::Satkers => {
                self.data.set_loading("satkers", false);
                match result.and_then(|response| response.list::<Satker>()) {
                    Ok((satkers, _)) => self.data.satkers = satkers,
                    Err(error) => self.error(&error),
                }
            }
            JobKind::Online => {
                self.data.set_loading("online", false);
                match result.and_then(|response| response.list::<OnlineUser>()) {
                    Ok((online, _)) => self.data.online = online,
                    Err(error) => self.error(&error),
                }
            }
            JobKind::Home => {
                self.data.set_loading("home", false);
                match result.and_then(|response| response.list::<HomeOperation>()) {
                    Ok((home, _)) => self.data.home = home,
                    Err(error) => self.error(&error),
                }
            }
            JobKind::Categories => {
                self.data.set_loading("categories", false);
                match result.and_then(|response| response.list::<Category>()) {
                    Ok((categories, _)) => self.data.categories = categories,
                    Err(error) => self.error(&error),
                }
            }
            JobKind::OperationSave => match result {
                Ok(_) => {
                    self.modal.close();
                    self.toast("Operasi disimpan.", ToastKind::Success);
                    self.refresh_operations();
                    self.refresh_home();
                }
                Err(error) => {
                    if let Modal::Operation(form) = &mut self.modal {
                        form.error = Some(error.message.clone());
                    }
                }
            },
            JobKind::OperationDelete => match result {
                Ok(_) => {
                    self.modal.close();
                    self.toast("Operasi dihapus.", ToastKind::Success);
                    self.refresh_operations();
                    self.refresh_home();
                }
                Err(error) => self.error(&error),
            },
            JobKind::ScenarioSave => match result {
                Ok(_) => {
                    self.modal.close();
                    self.toast("Skenario disimpan.", ToastKind::Success);
                    self.refresh_scenarios();
                    self.refresh_home();
                }
                Err(error) => {
                    if let Modal::Scenario(form) = &mut self.modal {
                        form.error = Some(error.message.clone());
                    }
                }
            },
            JobKind::ScenarioDelete => match result {
                Ok(_) => {
                    self.modal.close();
                    self.toast("Skenario dihapus.", ToastKind::Success);
                    self.refresh_scenarios();
                    self.refresh_home();
                }
                Err(error) => self.error(&error),
            },
            JobKind::ScenarioCopy => match result {
                Ok(_) => {
                    self.modal.close();
                    self.toast("Skenario disalin.", ToastKind::Success);
                    self.refresh_scenarios();
                    self.refresh_home();
                }
                Err(error) => {
                    if let Modal::Scenario(form) = &mut self.modal {
                        form.error = Some(error.message.clone());
                    }
                }
            },
            JobKind::UserSave => match result {
                Ok(_) => {
                    self.modal.close();
                    self.toast("Pengguna disimpan.", ToastKind::Success);
                    self.refresh_users();
                }
                Err(error) => {
                    if let Modal::User(form) = &mut self.modal {
                        form.error = Some(error.message.clone());
                    }
                }
            },
            JobKind::UserDelete => match result {
                Ok(_) => {
                    self.modal.close();
                    self.toast("Pengguna dihapus.", ToastKind::Success);
                    self.refresh_users();
                }
                Err(error) => self.error(&error),
            },
            JobKind::SatkerSave => match result {
                Ok(_) => {
                    self.modal.close();
                    self.toast("Satker disimpan.", ToastKind::Success);
                    self.refresh_satkers();
                }
                Err(error) => {
                    if let Modal::Satker(form) = &mut self.modal {
                        form.error = Some(error.message.clone());
                    }
                }
            },
            JobKind::SatkerDelete => match result {
                Ok(_) => {
                    self.modal.close();
                    self.toast("Satker dihapus.", ToastKind::Success);
                    self.refresh_satkers();
                }
                Err(error) => self.error(&error),
            },
            JobKind::ProfileSave => match result {
                Ok(_) => {
                    self.profile_message = Some((true, "Profil diperbarui.".to_owned()));
                    if let Some(session) = self.api.session() {
                        self.state.session = Some(session);
                    }
                    self.toast("Profil diperbarui.", ToastKind::Success);
                }
                Err(error) => {
                    self.profile_message = Some((false, error.message.clone()));
                }
            },
            JobKind::PasswordChange => match result {
                Ok(_) => {
                    self.old_password.clear();
                    self.new_password.clear();
                    self.confirm_password.clear();
                    self.old_password_visible = false;
                    self.new_password_visible = false;
                    self.confirm_password_visible = false;
                    self.profile_message = Some((true, "Password diganti.".to_owned()));
                    self.toast("Password diganti.", ToastKind::Success);
                }
                Err(error) => {
                    self.profile_message = Some((false, error.message.clone()));
                }
            },
            JobKind::SnapshotLoad => {
                self.data.set_loading("snapshot", false);
                match result.and_then(|response| response.data::<Option<Snapshot>>()) {
                    Ok(snapshot) => self.data.snapshot = snapshot,
                    Err(error) => {
                        self.data.snapshot = None;
                        self.error(&error);
                    }
                }
            }
            JobKind::SnapshotToggle | JobKind::SnapshotDelete => match result {
                Ok(_) => {
                    self.toast("Snapshot diperbarui.", ToastKind::Success);
                    if let Some(scenario) = self
                        .data
                        .snapshot
                        .as_ref()
                        .map(|snapshot| snapshot.skenario_id)
                    {
                        self.load_snapshot(scenario);
                    }
                }
                Err(error) => self.error(&error),
            },
            JobKind::WsTicket => match result
                .and_then(|response| response.data::<twg_contracts::realtime::WsTicketResponse>())
            {
                Ok(ticket) => self.open_ws(ticket),
                Err(error) => {
                    self.ws.state.status = RealtimeStatus::Error(error.message);
                    self.schedule_ws_retry();
                }
            },
            JobKind::Assets => {
                self.data.set_loading("assets", false);
                match result
                    .and_then(|response| response.list::<Asset>().map(|(assets, _total)| assets))
                {
                    Ok(assets) => {
                        if let Some(selected) = self.selected_asset
                            && !assets.iter().any(|asset| asset.id == selected)
                        {
                            self.selected_asset = None;
                            self.preview = PreviewUi::default();
                            self.preview_wanted = std::array::from_fn(|_| None);
                            self.preview_orbit = [PreviewOrbitInput::default(); PREVIEW_SLOTS];
                            self.preview_retry = 0;
                        }
                        self.assets = assets;
                        self.sync_editor_marker_models();
                    }
                    Err(error) => self.error(&error),
                }
            }
            JobKind::AssetGroups => {
                match result.and_then(|response| response.list::<AssetGroup>()) {
                    Ok((groups, _)) => self.data.asset_groups = groups,
                    Err(error) => self.error(&error),
                }
            }
            JobKind::Imports => {
                match result
                    .and_then(|response| response.list::<Asset>().map(|(assets, _total)| assets))
                {
                    Ok(imports) => {
                        self.imports = imports;
                        self.sync_editor_marker_models();
                    }
                    Err(error) => self.error(&error),
                }
            }
            JobKind::ImportUpload => {
                self.editor.import_busy = false;
                match result.and_then(|response| response.data::<Asset>()) {
                    Ok(asset) => {
                        self.toast(
                            format!("Aset '{}' diimpor ke operasi ini.", asset.name),
                            ToastKind::Success,
                        );
                        self.editor.import_draft.reset();
                        self.editor.import_error = None;
                        self.editor.import_dialog_open = false;
                        self.imports.insert(0, asset);
                        self.sync_editor_marker_models();
                        self.refresh_imports();
                        self.request_asset_check(None);
                    }
                    Err(error) => {
                        self.editor.import_error = Some(error.message.clone());
                        self.error(&error);
                    }
                }
            }
            JobKind::ImportDelete => match result {
                Ok(_) => {
                    self.toast("Aset impor dihapus.", ToastKind::Success);
                    self.refresh_imports();
                    self.request_asset_check(None);
                }
                Err(error) => self.error(&error),
            },
            JobKind::AssetGroupSave => match result {
                Ok(_) => {
                    let editing = self.asset_group_edit_id.is_some();
                    self.cancel_asset_group_edit();
                    self.asset_group_new_name.clear();
                    self.toast(
                        if editing {
                            "Kategori aset diperbarui."
                        } else {
                            "Kategori aset dibuat."
                        },
                        ToastKind::Success,
                    );
                    self.refresh_assets();
                }
                Err(error) => self.asset_group_error = Some(error.message),
            },
            JobKind::AssetGroupDelete => match result {
                Ok(_) => {
                    self.cancel_asset_group_edit();
                    self.toast("Kategori aset dihapus.", ToastKind::Success);
                    self.refresh_assets();
                }
                Err(error) => self.asset_group_error = Some(error.message),
            },
            JobKind::AssetManifest => {
                match result.and_then(|response| response.data::<AssetManifest>()) {
                    Ok(manifest) => {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let current = self.asset_cache.as_ref().is_some_and(|cache| {
                                cache
                                    .lock()
                                    .expect("asset cache poisoned")
                                    .is_current(&manifest)
                            });
                            if current {
                                self.asset_sync.state = AssetSyncState::Ready;
                                self.asset_sync.last_sync =
                                    Some(chrono::Utc::now().format("%H:%M:%S").to_string());
                                if let Some(nav) = self.pending_asset_nav.take() {
                                    self.enter_nav(nav);
                                }
                            } else {
                                self.spawn_asset_sync(manifest);
                            }
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            let _ = manifest;
                        }
                    }
                    Err(error) => self.finish_asset_check(Some(error.message)),
                }
            }
            JobKind::AssetUpload => {
                self.asset_upload.busy = false;
                match result.and_then(|response| response.data::<Asset>()) {
                    Ok(asset) => {
                        self.toast(
                            format!("Aset '{}' berhasil diunggah.", asset.name),
                            ToastKind::Success,
                        );
                        let selected = asset.id;
                        self.asset_upload.reset();
                        self.asset_edit_name = asset.name.clone();
                        self.asset_edit_group_id = asset.group_id;
                        self.asset_edit_move_mode = asset.move_mode;
                        self.asset_replace = None;
                        self.assets.insert(0, asset);
                        self.selected_asset = Some(selected);
                        self.preview = PreviewUi::default();
                        self.preview_wanted = std::array::from_fn(|_| None);
                        self.preview_orbit = [PreviewOrbitInput::default(); PREVIEW_SLOTS];
                        self.preview_retry = 0;
                        self.request_asset_check(None);
                    }
                    Err(error) => {
                        self.asset_upload.error = Some(error.message.clone());
                        self.error(&error);
                    }
                }
            }
            JobKind::AssetPreviewUpload => {
                match result.and_then(|response| response.data::<Asset>()) {
                    Ok(asset) => {
                        self.replace_asset(asset);
                        self.toast("Preview aset disimpan di server.", ToastKind::Success);
                        // Pull the manifest again so the preview is downloaded
                        // into the offline cache on this machine too.
                        self.sync_assets_now();
                    }
                    Err(error) => self.error(&error),
                }
            }
            JobKind::AssetConvert => {
                match result.and_then(|response| response.data::<ConvertResult>()) {
                    Ok(converted) => {
                        let kind = if converted.converted {
                            ToastKind::Success
                        } else {
                            ToastKind::Info
                        };
                        self.toast(converted.message, kind);
                        self.refresh_assets();
                        self.request_asset_check(None);
                    }
                    Err(error) => self.error(&error),
                }
            }
            JobKind::AssetRename => match result.and_then(|response| response.data::<Asset>()) {
                Ok(asset) => {
                    self.asset_edit_name = asset.name.clone();
                    self.asset_edit_group_id = asset.group_id;
                    self.asset_edit_move_mode = asset.move_mode;
                    let name = asset.name.clone();
                    self.replace_asset(asset);
                    self.toast(format!("Aset '{name}' diperbarui."), ToastKind::Success);
                }
                Err(error) => self.error(&error),
            },
            JobKind::AssetReplaceSource => {
                match result.and_then(|response| response.data::<Asset>()) {
                    Ok(asset) => {
                        let name = asset.name.clone();
                        let slot = self.asset_replace.take().map(|target| target.slot);
                        self.replace_asset(asset);
                        // A new file means a new preview; the version changed so
                        // the renderer re-fetches the model/texture.
                        self.preview = PreviewUi::default();
                        self.toast(
                            match slot {
                                Some(slot) => format!("File {slot} '{}' diperbarui.", name),
                                None => format!("File aset '{name}' diperbarui."),
                            },
                            ToastKind::Success,
                        );
                        self.request_asset_check(None);
                    }
                    Err(error) => self.error(&error),
                }
            }
            JobKind::AssetDelete => match result {
                Ok(_) => {
                    self.toast("Aset dihapus.", ToastKind::Success);
                    self.selected_asset = None;
                    self.preview = PreviewUi::default();
                    self.preview_wanted = std::array::from_fn(|_| None);
                    self.preview_orbit = [PreviewOrbitInput::default(); PREVIEW_SLOTS];
                    self.preview_retry = 0;
                    self.refresh_assets();
                    self.request_asset_check(None);
                }
                Err(error) => self.error(&error),
            },
        }
    }

    // ---------------------------------------------------------------- assets

    pub fn refresh_assets(&mut self) {
        self.refresh_asset_groups();
        self.data.set_loading("assets", true);
        let handle = self.api.request(HttpMethod::Get, "/private/assets", None);
        self.track(JobKind::Assets, handle);
    }

    pub fn refresh_asset_groups(&mut self) {
        let handle = self
            .api
            .request(HttpMethod::Get, "/private/asset-groups", None);
        self.track(JobKind::AssetGroups, handle);
    }

    pub fn begin_asset_group_edit(&mut self, id: Uuid, name: &str) {
        self.asset_group_edit_id = Some(id);
        self.asset_group_edit_name = name.to_owned();
        self.asset_group_error = None;
    }

    pub fn cancel_asset_group_edit(&mut self) {
        self.asset_group_edit_id = None;
        self.asset_group_edit_name.clear();
        self.asset_group_error = None;
    }

    pub fn begin_asset_group_save(&mut self) {
        if self.role() != Role::Admin {
            self.asset_group_error =
                Some("Hanya admin yang bisa membuat kategori aset.".to_owned());
            return;
        }
        let (path, body) = if let Some(id) = self.asset_group_edit_id {
            let name = self.asset_group_edit_name.trim().to_owned();
            if name.is_empty() {
                self.asset_group_error = Some("Nama kategori wajib diisi.".to_owned());
                return;
            }
            (format!("/admin/asset-groups/{id}"), json!({ "name": name }))
        } else {
            let name = self.asset_group_new_name.trim().to_owned();
            if name.is_empty() {
                self.asset_group_error = Some("Nama kategori wajib diisi.".to_owned());
                return;
            }
            ("/admin/asset-groups".to_owned(), json!({ "name": name }))
        };
        self.asset_group_error = None;
        let method = if self.asset_group_edit_id.is_some() {
            HttpMethod::Put
        } else {
            HttpMethod::Post
        };
        let handle = self.api.request(method, path, Some(body));
        self.track(JobKind::AssetGroupSave, handle);
    }

    pub fn begin_asset_group_delete(&mut self, id: Uuid) {
        if self.role() != Role::Admin {
            self.asset_group_error =
                Some("Hanya admin yang bisa menghapus kategori aset.".to_owned());
            return;
        }
        let handle = self.api.request(
            HttpMethod::Delete,
            format!("/admin/asset-groups/{id}"),
            None,
        );
        self.track(JobKind::AssetGroupDelete, handle);
    }

    /// Asks the server for the asset manifest and syncs the offline cache when
    /// something changed. When `nav` is set the navigation happens after the
    /// check (or after a failed check with a usable cache).
    pub fn request_asset_check(&mut self, nav: Option<NavItem>) {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = nav;
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.asset_cache.is_none() {
                return;
            }
            if self.asset_sync.state.is_busy() {
                if nav.is_some() {
                    self.pending_asset_nav = nav;
                } else {
                    self.pending_asset_check = true;
                }
                return;
            }
            self.pending_asset_nav = nav;
            self.asset_sync.state = AssetSyncState::Checking;
            let handle = self
                .api
                .request(HttpMethod::Get, "/private/assets/manifest", None);
            self.track(JobKind::AssetManifest, handle);
        }
    }

    pub fn sync_assets_now(&mut self) {
        self.request_asset_check(None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn spawn_asset_sync(&mut self, manifest: AssetManifest) {
        let Some(shared) = self.asset_cache.clone() else {
            self.finish_asset_check(Some("cache aset tidak tersedia".to_owned()));
            return;
        };
        let base_url = self.base_url.clone();
        let progress = Arc::new(Mutex::new(SyncProgress::default()));
        self.asset_sync.progress = Some(progress.clone());
        self.asset_sync.state = AssetSyncState::Syncing;
        // Work on a clone so the UI keeps reading the previous consistent
        // snapshot while files download, then swap it in atomically.
        let mut worker = shared.lock().expect("asset cache poisoned").clone();
        std::thread::spawn(move || {
            let result = worker.sync(&manifest, &base_url, &progress);
            *shared.lock().expect("asset cache poisoned") = worker;
            let mut guard = progress.lock().expect("sync progress poisoned");
            guard.finished = true;
            guard.error = result.err();
        });
    }

    fn tick_asset_sync(&mut self) {
        let Some(progress) = self.asset_sync.progress.clone() else {
            return;
        };
        let (finished, done) = {
            let guard = progress.lock().expect("sync progress poisoned");
            (guard.finished.then(|| guard.error.clone()), guard.done)
        };
        let Some(error) = finished else {
            return;
        };
        let background = self.pending_asset_nav.is_none();
        let failed = error.is_some();
        self.asset_sync.progress = None;
        self.asset_sync.last_sync = Some(chrono::Utc::now().format("%H:%M:%S").to_string());
        self.finish_asset_check(error);
        if background && !failed && done > 0 {
            self.toast(
                format!("{done} file aset diperbarui di cache offline."),
                ToastKind::Info,
            );
        }
        if !failed && !self.editor.preview {
            // Models that were missing before the sync are now in the cache;
            // the signature alone cannot notice that, so rebuild once.
            self.editor.marker_model_signature.clear();
            self.sync_editor_marker_models();
        }
        if self.pending_asset_check {
            self.pending_asset_check = false;
            self.request_asset_check(None);
        }
    }

    fn finish_asset_check(&mut self, error: Option<String>) {
        self.asset_sync.state = match &error {
            Some(message) => AssetSyncState::Offline(message.clone()),
            None => AssetSyncState::Ready,
        };
        // Preview files come from the server, so a finished cache sync does
        // not invalidate them.
        let pending = self.pending_asset_nav.take();
        if let Some(message) = &error {
            let offline_available = self.has_offline_manifest();
            if offline_available {
                self.toast(
                    format!("Server aset tidak bisa dicek ({message}); memakai cache offline."),
                    ToastKind::Info,
                );
            } else {
                self.toast(
                    format!("Sinkronisasi aset gagal: {message}"),
                    ToastKind::Error,
                );
            }
        } else if pending.is_some() {
            self.toast("Aset tersinkron ke cache offline.", ToastKind::Success);
        }
        if let Some(nav) = pending
            && (error.is_none() || self.has_offline_manifest())
        {
            self.enter_nav(nav);
        }
    }

    fn has_offline_manifest(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.asset_cache.as_ref().is_some_and(|cache| {
                cache
                    .lock()
                    .expect("asset cache poisoned")
                    .manifest()
                    .is_some()
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }

    fn enter_nav(&mut self, item: NavItem) {
        self.state.navigate(item);
        if item == NavItem::Preview {
            self.open_editor(self.selected_scenario);
        }
        if item == NavItem::Assets {
            self.refresh_assets();
        }
    }

    pub fn select_asset(&mut self, asset_id: Uuid) {
        if self.selected_asset == Some(asset_id) {
            return;
        }
        self.selected_asset = Some(asset_id);
        let asset = self.assets.iter().find(|asset| asset.id == asset_id);
        self.asset_edit_name = asset.map(|asset| asset.name.clone()).unwrap_or_default();
        self.asset_edit_group_id = asset.and_then(|asset| asset.group_id);
        self.asset_edit_move_mode = asset.map(|asset| asset.move_mode).unwrap_or_default();
        self.asset_replace = None;
        self.preview = PreviewUi::default();
        self.preview_wanted = std::array::from_fn(|_| None);
        self.preview_orbit = [PreviewOrbitInput::default(); PREVIEW_SLOTS];
        self.preview_retry = 0;
    }

    fn replace_asset(&mut self, asset: Asset) {
        let list = if asset.origin.is_import() {
            &mut self.imports
        } else {
            &mut self.assets
        };
        match list.iter_mut().find(|existing| existing.id == asset.id) {
            Some(existing) => *existing = asset,
            None => list.insert(0, asset),
        }
    }

    /// True when this asset (and all its files) is already mirrored offline.
    pub fn asset_is_cached(&self, asset: &Asset) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(cache) = self.asset_cache.as_ref() else {
                return false;
            };
            let cache = cache.lock().expect("asset cache poisoned");
            let Some(manifest) = cache.manifest() else {
                return false;
            };
            let same_version = manifest
                .assets
                .iter()
                .any(|remote| remote.id == asset.id && remote.version == asset.version);
            same_version
                && asset
                    .sources
                    .iter()
                    .all(|source| cache.resolve(asset, source.slot, true).is_some())
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = asset;
            false
        }
    }

    pub fn asset_cache_root(&self) -> Option<String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.asset_cache.as_ref().map(|cache| {
                cache
                    .lock()
                    .expect("asset cache poisoned")
                    .root()
                    .display()
                    .to_string()
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }

    /// Latest saved preview screenshot per asset id. Desktop only; the web
    /// build has no local filesystem cache.
    pub fn asset_preview_map(&self) -> BTreeMap<String, PathBuf> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(root) = self.asset_cache_root() else {
                return BTreeMap::new();
            };
            let dir = std::path::Path::new(&root).join("previews");
            let Ok(entries) = std::fs::read_dir(&dir) else {
                return BTreeMap::new();
            };
            let mut newest: BTreeMap<String, (u64, PathBuf)> = BTreeMap::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("png") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                    continue;
                };
                let Some((asset_id, stamp)) = stem.rsplit_once('_') else {
                    continue;
                };
                let Ok(stamp) = stamp.parse::<u64>() else {
                    continue;
                };
                match newest.get(asset_id) {
                    Some((current, _)) if *current >= stamp => {}
                    _ => {
                        newest.insert(asset_id.to_owned(), (stamp, path));
                    }
                }
            }
            newest
                .into_iter()
                .map(|(asset_id, (_, path))| (asset_id, path))
                .collect()
        }
        #[cfg(target_arch = "wasm32")]
        {
            BTreeMap::new()
        }
    }

    pub fn cached_asset_preview(&self, asset_id: Uuid) -> Option<PathBuf> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let cache = self.asset_cache.as_ref()?;
            let cache = cache.lock().expect("asset cache poisoned");
            let path = cache.preview_path(asset_id);
            path.is_file().then_some(path)
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = asset_id;
            None
        }
    }

    pub fn cached_asset_count(&self) -> usize {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.asset_cache.as_ref().map_or(0, |cache| {
                cache
                    .lock()
                    .expect("asset cache poisoned")
                    .manifest()
                    .map(|manifest| manifest.assets.len())
                    .unwrap_or(0)
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            0
        }
    }

    pub fn push_dropped_file(&mut self, path: PathBuf) {
        self.dropped_files.push(path.clone());
        if self.state.nav != NavItem::Assets {
            self.toast("Buka menu Aset untuk mengunggah file.", ToastKind::Info);
            return;
        }
        // A pending slot replacement takes precedence over the upload form.
        if self.asset_replace.is_some() {
            self.begin_asset_replace_file(path);
            return;
        }
        let draft = &mut self.asset_upload;
        let required = draft.required_files().len();
        if draft.filled_files() >= required {
            draft.files.clear();
            draft.name.clear();
        }
        if draft.name.trim().is_empty() {
            draft.name = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("aset")
                .to_owned();
        }
        if let Some(index) = draft
            .files
            .iter()
            .position(|file| file.as_os_str().is_empty())
        {
            draft.files[index] = path;
        } else {
            draft.files.push(path);
        }
        draft.error = None;
    }

    pub fn clear_asset_upload(&mut self) {
        self.asset_upload.files.clear();
        self.asset_upload.name.clear();
        self.asset_upload.error = None;
    }

    pub fn begin_asset_upload(&mut self) {
        self.asset_upload.error = None;
        if self.role() != Role::Admin {
            self.asset_upload.error = Some("Hanya admin yang bisa mengunggah aset.".to_owned());
            return;
        }
        let name = self.asset_upload.name.trim().to_owned();
        if name.is_empty() {
            self.asset_upload.error = Some("Nama aset wajib diisi.".to_owned());
            return;
        }
        let required = self.asset_upload.required_files().len();
        let filled = self.asset_upload.filled_files();
        if self.asset_upload.files.len() != required || filled != required {
            self.asset_upload.error = Some(format!(
                "Butuh {required} file untuk kategori ini ({filled} terisi).",
            ));
            return;
        }
        let Some(group_id) = self.asset_upload.group_id else {
            self.asset_upload.error = Some("Pilih kategori aset sebelum mengunggah.".to_owned());
            return;
        };
        let model_kind = self
            .asset_upload
            .category
            .is_model()
            .then_some(self.asset_upload.model_kind);
        let (content_type, body) = match multipart_asset_body(
            &name,
            self.asset_upload.category,
            model_kind,
            self.asset_upload.move_mode,
            Some(group_id),
            None,
            &self.asset_upload.files,
        ) {
            Ok(payload) => payload,
            Err(error) => {
                self.asset_upload.error = Some(error);
                return;
            }
        };
        self.asset_upload.busy = true;
        let handle = self.api.upload("/admin/assets", content_type, body);
        self.track(JobKind::AssetUpload, handle);
    }

    pub fn begin_asset_convert(
        &mut self,
        asset_id: Uuid,
        slot: AssetSourceSlot,
        target: ConvertTarget,
        size: Option<u32>,
    ) {
        let request = ConvertRequest {
            target,
            slot: Some(slot),
            size,
            force: false,
        };
        let handle = self.api.request(
            HttpMethod::Post,
            format!("/admin/assets/{asset_id}/convert"),
            Some(json!(request)),
        );
        self.track(JobKind::AssetConvert, handle);
    }

    pub fn begin_asset_delete(&mut self, asset_id: Uuid) {
        let handle = self.api.request(
            HttpMethod::Delete,
            format!("/admin/assets/{asset_id}"),
            None,
        );
        self.track(JobKind::AssetDelete, handle);
    }

    /// Renames the asset using the current `asset_edit_name` value.
    pub fn begin_asset_rename(&mut self, asset_id: Uuid) {
        if self.role() != Role::Admin {
            self.toast("Hanya admin yang bisa mengubah aset.", ToastKind::Error);
            return;
        }
        let name = self.asset_edit_name.trim().to_owned();
        if name.is_empty() {
            self.toast("Nama aset wajib diisi.", ToastKind::Error);
            return;
        }
        let move_mode = self
            .assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .and_then(|asset| {
                (asset.category.is_model()
                    && asset.model_kind_or_static() == AssetModelKind::Static)
                    .then_some(self.asset_edit_move_mode)
            });
        let handle = self.api.request(
            HttpMethod::Put,
            format!("/admin/assets/{asset_id}"),
            Some(json!({
                "name": name,
                "group_id": self.asset_edit_group_id,
                "move_mode": move_mode,
            })),
        );
        self.track(JobKind::AssetRename, handle);
    }

    /// Arms the next dropped file as a replacement for `slot`.
    pub fn begin_asset_replace(&mut self, asset_id: Uuid, slot: AssetSourceSlot) {
        if self.role() != Role::Admin {
            self.toast(
                "Hanya admin yang bisa mengganti file aset.",
                ToastKind::Error,
            );
            return;
        }
        self.asset_replace = Some(AssetReplaceTarget { asset_id, slot });
        self.toast(
            format!("Tarik file {slot} baru ke jendela aplikasi atau klik Pilih file."),
            ToastKind::Info,
        );
    }

    /// Replaces a source slot with a file chosen from the native picker.
    pub fn begin_asset_replace_pick(
        &mut self,
        asset_id: Uuid,
        slot: AssetSourceSlot,
        path: PathBuf,
    ) {
        if self.role() != Role::Admin {
            self.toast(
                "Hanya admin yang bisa mengganti file aset.",
                ToastKind::Error,
            );
            return;
        }
        self.asset_replace = Some(AssetReplaceTarget { asset_id, slot });
        self.begin_asset_replace_file(path);
    }

    pub fn cancel_asset_replace(&mut self) {
        self.asset_replace = None;
    }

    /// Uploads a dropped file into the armed replacement slot. Returns `false`
    /// when no replacement is pending.
    pub fn begin_asset_replace_file(&mut self, path: PathBuf) -> bool {
        let Some(target) = self.asset_replace.clone() else {
            return false;
        };
        let Some(asset) = self
            .assets
            .iter()
            .find(|asset| asset.id == target.asset_id)
            .cloned()
        else {
            self.asset_replace = None;
            self.toast(
                "Aset tidak ditemukan; muat ulang daftar aset.",
                ToastKind::Error,
            );
            return true;
        };
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if !asset.category.allows_extension(&extension) {
            self.toast(
                format!(
                    "{} harus memakai file: {}",
                    asset.category,
                    asset.category.upload_extensions().join(", ")
                ),
                ToastKind::Error,
            );
            return true;
        }
        let (content_type, body) = match multipart_asset_source_body(target.slot, &path) {
            Ok(payload) => payload,
            Err(error) => {
                self.toast(error, ToastKind::Error);
                return true;
            }
        };
        let handle = self.api.upload(
            format!("/admin/assets/{}/source", target.asset_id),
            content_type,
            body,
        );
        self.track(JobKind::AssetReplaceSource, handle);
        true
    }

    /// Marks an asset as the one used by the editor/game, resolving files from
    /// the offline cache (converted variant first, original as fallback).
    pub fn use_asset(&mut self, asset_id: Uuid, prefer_variants: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(asset) = self
                .assets
                .iter()
                .find(|asset| asset.id == asset_id)
                .cloned()
            else {
                return;
            };
            let Some(cache) = self.asset_cache.clone() else {
                self.toast("Cache aset tidak tersedia.", ToastKind::Error);
                return;
            };
            let mut files = Vec::new();
            {
                let cache = cache.lock().expect("asset cache poisoned");
                for source in &asset.sources {
                    match cache.resolve(&asset, source.slot, prefer_variants) {
                        Some((variant, path)) => files.push(UsedAssetFile {
                            slot: source.slot,
                            path,
                            variant,
                        }),
                        None => {
                            drop(cache);
                            self.toast(
                                format!(
                                    "File {} belum ada di cache. Jalankan sinkronisasi aset dulu.",
                                    source.slot
                                ),
                                ToastKind::Error,
                            );
                            return;
                        }
                    }
                }
            }
            self.used_asset = Some(UsedAsset {
                asset_id,
                name: asset.name.clone(),
                model_kind: asset.model_kind,
                files,
                prefer_variants,
            });
            self.toast(
                format!("Aset '{}' dipakai dari cache offline.", asset.name),
                ToastKind::Success,
            );
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (asset_id, prefer_variants);
            self.toast("Cache offline hanya tersedia di desktop.", ToastKind::Info);
        }
    }

    pub fn clear_used_asset(&mut self) {
        if self.used_asset.take().is_some() {
            self.toast(
                "Pilihan aset dilepas; kembali ke model bawaan.",
                ToastKind::Info,
            );
        }
    }

    /// Resolves the server preview of the selected asset. Previews always come
    /// from the server; the offline cache is only used by the editor/game.
    pub fn ensure_asset_preview(&mut self, _context: &egui::Context) {
        let Some(asset_id) = self.selected_asset else {
            self.preview_wanted = std::array::from_fn(|_| None);
            return;
        };

        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(asset) = self
                .assets
                .iter()
                .find(|asset| asset.id == asset_id)
                .cloned()
            else {
                return;
            };
            self.preview_wanted = self.preview_request(&asset);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = asset_id;
            self.preview_wanted = std::array::from_fn(|_| None);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn preview_request(&self, asset: &Asset) -> [Option<PreviewSource>; PREVIEW_SLOTS] {
        let mut request: [Option<PreviewSource>; PREVIEW_SLOTS] = std::array::from_fn(|_| None);
        if asset.category.is_model() {
            let animated = asset.model_kind == Some(AssetModelKind::Animated)
                || asset.source(AssetSourceSlot::Animated).is_some();
            if animated {
                for (index, (slot, label)) in [
                    (AssetSourceSlot::Idle, "Idle"),
                    (AssetSourceSlot::Animated, "Animasi"),
                ]
                .into_iter()
                .enumerate()
                {
                    if let Some(source) = asset.source(slot) {
                        request[index] = Some(self.model_preview_source(
                            asset,
                            source,
                            label,
                            slot == AssetSourceSlot::Animated,
                        ));
                    }
                }
            } else if let Some(source) = asset.sources.first() {
                request[0] = Some(self.model_preview_source(asset, source, "Model", false));
            }
        } else if let Some(source) = asset.sources.first() {
            request[0] = Some(PreviewSource::Image(ImagePreviewKey {
                asset_id: asset.id,
                version: asset.version,
                retry: self.preview_retry,
                label: "Preview".to_owned(),
                url: format!("{}{}", self.base_url.trim_end_matches('/'), source.file.url),
            }));
        }
        request
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn model_preview_source(
        &self,
        asset: &Asset,
        source: &AssetSource,
        label: &str,
        with_animation: bool,
    ) -> PreviewSource {
        let (url, draco) = self.preview_model_file(source);
        let animation_url = with_animation.then(|| url.clone());
        PreviewSource::Model(ModelPreviewKey {
            asset_id: asset.id,
            version: asset.version,
            retry: self.preview_retry,
            label: label.to_owned(),
            url,
            draco,
            animation_url,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn preview_model_file(&self, source: &AssetSource) -> (String, bool) {
        let base = self.base_url.trim_end_matches('/');
        if self.asset_prefer_variants {
            for kind in [
                AssetVariantKind::DracoLod2,
                AssetVariantKind::Lod2,
                AssetVariantKind::Draco,
            ] {
                if let Some(variant) = source.variant(kind) {
                    return (format!("{base}{}", variant.url), kind.is_draco());
                }
            }
        }
        (format!("{base}{}", source.file.url), source.has_draco)
    }

    fn tick_toasts(&mut self) {
        let delta = 1.0 / 60.0;
        for toast in &mut self.toasts {
            toast.remaining -= delta;
        }
        self.toasts.retain(|toast| toast.remaining > 0.0);
    }

    pub fn toast_color(&self, kind: ToastKind) -> egui::Color32 {
        match kind {
            ToastKind::Info => theme::color(twg_client::design::DARK.foreground),
            ToastKind::Success => theme::color(twg_client::design::DARK.success),
            ToastKind::Error => theme::color(twg_client::design::DARK.destructive),
        }
    }

    // ---------------------------------------------------------------- realtime

    pub fn ensure_ws(&mut self) {
        if self.state.session.is_none() {
            return;
        }
        if self.ws.connection.is_some() || self.ws.ticket_pending {
            return;
        }
        if let Some(next) = self.ws.next_attempt
            && Instant::now() < next
        {
            return;
        }
        self.ws.next_attempt = None;
        self.ws.ticket_pending = true;
        self.ws.state.status = RealtimeStatus::Connecting;
        let handle = self
            .api
            .request(HttpMethod::Post, "/private/ws-ticket", None);
        self.track(JobKind::WsTicket, handle);
    }

    pub fn open_ws(&mut self, ticket: twg_contracts::realtime::WsTicketResponse) {
        let url = format!(
            "{}{}?ticket={}",
            self.base_url.replace("http", "ws"),
            ticket.ws_path,
            ticket.ticket
        );
        let connection = self.ws_transport.connect(url);
        self.ws.connection = Some(Arc::new(connection));
        self.ws.ticket_pending = false;
        self.ws.subscribed = false;
        self.ws.state.status = RealtimeStatus::Connecting;
    }

    pub fn close_ws(&mut self) {
        if let Some(connection) = self.ws.connection.take() {
            connection.close();
        }
        self.ws.state.status = RealtimeStatus::Disabled;
        self.ws.subscribed = false;
        self.ws.ticket_pending = false;
    }

    pub fn tick_ws(&mut self) {
        let Some(connection) = self.ws.connection.clone() else {
            return;
        };
        let status = connection.status();
        self.ws.state.status = status.clone();
        if status.is_connected() {
            self.ws.state.reset_reconnect();
        }
        let mut hello = false;
        let mut resync = false;
        let mut error_message = None;
        while let Some(payload) = connection.poll_inbound() {
            if let Ok(value) = serde_json::from_str::<Value>(&payload) {
                match value.get("type").and_then(Value::as_str) {
                    Some("hello") => hello = true,
                    Some("event") => {
                        self.ws.state.note_event();
                        if let (Some(skenario), Some(sequence)) = (
                            value
                                .get("skenario_id")
                                .and_then(Value::as_str)
                                .and_then(|value| Uuid::parse_str(value).ok()),
                            value.get("sequence").and_then(Value::as_u64),
                        ) {
                            self.ws.state.note_sequence(skenario, sequence);
                        }
                    }
                    Some("resync") => resync = true,
                    Some("error") => {
                        error_message = Some(
                            value
                                .get("message")
                                .and_then(Value::as_str)
                                .unwrap_or("kesalahan realtime")
                                .to_owned(),
                        );
                    }
                    _ => {}
                }
            }
        }
        if hello {
            self.ws.state.status = RealtimeStatus::Connected;
            self.subscribe_ws();
        }
        if resync {
            self.toast("Realtime meminta resync state.", ToastKind::Info);
        }
        if let Some(message) = error_message {
            self.toast(message, ToastKind::Error);
        }
        if connection.is_closed() {
            self.ws.connection = None;
            self.ws.subscribed = false;
            self.ws.state.schedule_reconnect();
            self.ws.next_attempt = Some(Instant::now() + self.ws.state.retry_delay());
        }
    }

    pub fn subscribe_ws(&mut self) {
        let Some(scenario) = self.ws.scenario else {
            return;
        };
        let Some(connection) = &self.ws.connection else {
            return;
        };
        let last = self.ws.state.last_sequence.get(&scenario).copied();
        let message = json!({
            "type": "subscribe",
            "request_id": "ui-subscribe",
            "skenario_id": scenario,
            "last_sequence": last,
        });
        if connection.send_text(message.to_string()) {
            self.ws.subscribed = true;
        }
    }

    fn schedule_ws_retry(&mut self) {
        self.ws.state.schedule_reconnect();
        self.ws.next_attempt = Some(Instant::now() + Duration::from_millis(500));
    }

    pub fn reconnect_ws_now(&mut self) {
        self.ws.next_attempt = None;
        self.ws.ticket_pending = false;
        self.ws.state.reset_reconnect();
        self.ensure_ws();
    }

    pub fn go(&mut self, item: NavItem) {
        if item == NavItem::Preview {
            if self.has_offline_manifest() {
                // A consistent cache snapshot already exists: open the map
                // immediately and refresh the manifest in the background. This
                // keeps startup fast on slow networks; newly downloaded files
                // are used the next time assets are loaded.
                self.enter_nav(item);
                self.request_asset_check(None);
            } else {
                // First run: wait for the initial cache fill so the map never
                // starts with missing assets.
                self.request_asset_check(Some(item));
                if !self.asset_sync.state.is_busy() {
                    self.enter_nav(item);
                }
            }
            return;
        }
        self.state.navigate(item);
        if item == NavItem::Dashboard {
            self.refresh_all();
        } else if item == NavItem::Operations {
            self.refresh_operations();
        } else if item == NavItem::Scenarios {
            self.refresh_operations();
            self.refresh_scenarios();
        } else if matches!(item, NavItem::Assets | NavItem::AssetGroups) {
            self.refresh_assets();
        } else if item == NavItem::Games {
            self.refresh_home();
        } else if item == NavItem::Users {
            self.refresh_users();
        } else if item == NavItem::Satkers {
            self.refresh_satkers();
        } else if item == NavItem::Online {
            self.refresh_online();
        } else if item == NavItem::Snapshots {
            self.refresh_operations();
            self.refresh_scenarios();
            self.refresh_categories();
        } else if item == NavItem::Profile {
            self.sync_profile_fields();
        }
    }

    pub fn refresh_current_screen(&mut self) {
        match self.state.nav {
            NavItem::Dashboard => self.refresh_all(),
            NavItem::Operations => self.refresh_operations(),
            NavItem::Scenarios => {
                self.refresh_operations();
                self.refresh_scenarios();
            }
            NavItem::Games => self.refresh_home(),
            NavItem::Assets | NavItem::AssetGroups => self.refresh_assets(),
            NavItem::Users => self.refresh_users(),
            NavItem::Satkers => self.refresh_satkers(),
            NavItem::Online => self.refresh_online(),
            NavItem::Snapshots => {
                self.refresh_operations();
                self.refresh_scenarios();
            }
            NavItem::Profile => self.sync_profile_fields(),
            NavItem::Preview => self.reload_editor(),
        }
    }

    pub fn snapshot_matches_filter(&self, snapshot: &Snapshot) -> bool {
        let Some(filter) = self.snapshot_date else {
            return true;
        };
        snapshot.updated_at.date() == filter
    }
}

fn encode_query(value: &str) -> String {
    value
        .trim()
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('#', "%23")
}

// ---------------------------------------------------------------- API actions

impl ClientUi {
    pub fn submit_operation(&mut self) {
        let Modal::Operation(form) = &mut self.modal else {
            return;
        };
        if form.name.trim().is_empty() {
            form.error = Some("Nama operasi wajib diisi.".to_owned());
            return;
        }
        let name = form.name.trim().to_owned();
        let satker_id = if form.satker_id.trim().is_empty() {
            None
        } else {
            Uuid::parse_str(form.satker_id.trim()).ok()
        };
        let body = Some(json!({ "name": name, "satker_id": satker_id }));
        let handle = match form.id {
            Some(id) => self
                .api
                .request(HttpMethod::Put, format!("/admin/operasis/{id}"), body),
            None => self.api.request(HttpMethod::Post, "/admin/operasis", body),
        };
        self.track(JobKind::OperationSave, handle);
    }

    pub fn submit_scenario(&mut self) {
        let Modal::Scenario(form) = &mut self.modal else {
            return;
        };
        let operasi_id = match Uuid::parse_str(form.operasi_id.trim()) {
            Ok(id) => id,
            Err(_) => {
                form.error = Some("Operasi wajib dipilih.".to_owned());
                return;
            }
        };
        let name = form.name.trim().to_owned();
        if name.is_empty() {
            form.error = Some("Nama skenario wajib diisi.".to_owned());
            return;
        }
        let mut indoor_config: Value =
            serde_json::from_str(if form.indoor_config.trim().is_empty() {
                "{}"
            } else {
                form.indoor_config.trim()
            })
            .unwrap_or(Value::Object(serde_json::Map::new()));
        if form.map_mode == "indoor"
            && let Value::Object(map) = &mut indoor_config
        {
            map.insert("grid_size_m".to_owned(), json!(form.grid_size_m));
        }

        if form.copy {
            let target = match Uuid::parse_str(form.target_operasi_id.trim()) {
                Ok(id) => id,
                Err(_) => {
                    form.error = Some("Operasi tujuan wajib dipilih.".to_owned());
                    return;
                }
            };
            let Some(source) = form.id else {
                form.error = Some("Skenario sumber tidak ditemukan.".to_owned());
                return;
            };
            let handle = self.api.request(
                HttpMethod::Post,
                "/admin/skenarios/copy",
                Some(json!({
                    "name": name,
                    "src_skenario_id": source,
                    "tgr_operasi_id": target,
                })),
            );
            self.track(JobKind::ScenarioCopy, handle);
            return;
        }

        let body = Some(json!({
            "name": name,
            "operasi_id": operasi_id,
            "center_x": form.center_x,
            "center_y": form.center_y,
            "zoom": form.zoom,
            "min_zoom": form.min_zoom,
            "max_zoom": form.max_zoom,
            "pitch": form.pitch,
            "bearing": form.bearing,
            "map_mode": form.map_mode,
            "base_layer": form.base_layer,
            "sky_mode": "",
            "indoor_config": indoor_config,
        }));
        let handle = match form.id {
            Some(id) => self
                .api
                .request(HttpMethod::Put, format!("/admin/skenarios/{id}"), body),
            None => self.api.request(HttpMethod::Post, "/admin/skenarios", body),
        };
        self.track(JobKind::ScenarioSave, handle);
    }

    pub fn submit_user(&mut self) {
        let Modal::User(form) = &mut self.modal else {
            return;
        };
        if form.name.trim().is_empty() || form.username.trim().is_empty() {
            form.error = Some("Nama dan username wajib diisi.".to_owned());
            return;
        }
        let satker_id = if form.satker_id.trim().is_empty() {
            None
        } else {
            Some(form.satker_id.trim().to_owned())
        };
        let handle = match form.id {
            Some(id) => {
                let body = json!({
                    "name": form.name,
                    "units": form.units,
                    "satker_id": satker_id,
                    "profile_img": form.profile_img,
                });
                self.api
                    .request(HttpMethod::Put, format!("/admin/users/{id}"), Some(body))
            }
            None => {
                if form.password.len() < 8 {
                    form.error = Some("Password minimal 8 karakter.".to_owned());
                    return;
                }
                if form.password != form.confirm_password {
                    form.error = Some("Konfirmasi password tidak cocok.".to_owned());
                    return;
                }
                let body = json!({
                    "name": form.name,
                    "username": form.username,
                    "password": form.password,
                    "units": form.units,
                    "role": form.role,
                    "satker_id": satker_id,
                    "profile_img": form.profile_img,
                });
                self.api
                    .request(HttpMethod::Post, "/admin/users", Some(body))
            }
        };
        self.track(JobKind::UserSave, handle);
    }

    pub fn submit_satker(&mut self) {
        let Modal::Satker(form) = &mut self.modal else {
            return;
        };
        if form.code.trim().is_empty() || form.name.trim().is_empty() {
            form.error = Some("Kode dan nama satker wajib diisi.".to_owned());
            return;
        }
        let body = json!({
            "code": form.code,
            "name": form.name,
            "is_active": form.is_active,
        });
        let handle = match form.id {
            Some(id) => {
                self.api
                    .request(HttpMethod::Put, format!("/admin/satkers/{id}"), Some(body))
            }
            None => self
                .api
                .request(HttpMethod::Post, "/admin/satkers", Some(body)),
        };
        self.track(JobKind::SatkerSave, handle);
    }

    pub fn save_profile(&mut self) {
        let body = json!({ "name": self.profile_name, "profile_img": self.profile_img });
        let handle = self
            .api
            .request(HttpMethod::Put, "/private/profile", Some(body));
        self.track(JobKind::ProfileSave, handle);
    }

    pub fn change_password(&mut self) {
        if self.old_password.len() < 8 || self.new_password.len() < 8 {
            self.profile_message = Some((
                false,
                "Password lama dan baru minimal 8 karakter.".to_owned(),
            ));
            return;
        }
        if self.new_password != self.confirm_password {
            self.profile_message =
                Some((false, "Konfirmasi password baru tidak cocok.".to_owned()));
            return;
        }
        let body = json!({
            "old_password": self.old_password,
            "new_password": self.new_password,
        });
        let handle = self
            .api
            .request(HttpMethod::Put, "/private/profile/password", Some(body));
        self.track(JobKind::PasswordChange, handle);
    }

    pub fn delete_confirmed(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::DeleteOperation(id) => {
                let handle =
                    self.api
                        .request(HttpMethod::Delete, format!("/admin/operasis/{id}"), None);
                self.track(JobKind::OperationDelete, handle);
            }
            ConfirmAction::DeleteScenario(id) => {
                let handle =
                    self.api
                        .request(HttpMethod::Delete, format!("/admin/skenarios/{id}"), None);
                self.track(JobKind::ScenarioDelete, handle);
            }
            ConfirmAction::DeleteUser(id) => {
                let handle =
                    self.api
                        .request(HttpMethod::Delete, format!("/admin/users/{id}"), None);
                self.track(JobKind::UserDelete, handle);
            }
            ConfirmAction::DeleteSatker(id) => {
                let handle =
                    self.api
                        .request(HttpMethod::Delete, format!("/admin/satkers/{id}"), None);
                self.track(JobKind::SatkerDelete, handle);
            }
            ConfirmAction::DeleteSnapshot(skenario_id) => {
                let handle = self.api.request(
                    HttpMethod::Delete,
                    format!("/private/game-sessions/snapshot/{skenario_id}"),
                    None,
                );
                self.track(JobKind::SnapshotDelete, handle);
            }
            ConfirmAction::DeleteAsset(id) => {
                self.begin_asset_delete(id);
            }
            ConfirmAction::DeleteAssetGroup(id) => {
                self.begin_asset_group_delete(id);
            }
            ConfirmAction::Logout => self.logout(),
        }
        self.modal.close();
    }

    pub fn toggle_snapshot(&mut self, enabled: bool) {
        let Some(snapshot) = &self.data.snapshot else {
            return;
        };
        let skenario_id = snapshot.skenario_id;
        let handle = self.api.request(
            HttpMethod::Patch,
            format!("/private/game-sessions/snapshot/{skenario_id}/enabled"),
            Some(json!({ "snapshot_enabled": enabled })),
        );
        self.track(JobKind::SnapshotToggle, handle);
    }
}

/// Reads `?scenario=<uuid>` so the web client can deep-link straight into the
/// editor. Desktop builds ignore it.
#[cfg(target_arch = "wasm32")]
fn initial_editor_scenario() -> Option<Uuid> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    for pair in search.trim_start_matches('?').split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next() != Some("scenario") {
            continue;
        }
        return Uuid::parse_str(&percent_decode(parts.next().unwrap_or_default())).ok();
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn initial_editor_scenario() -> Option<Uuid> {
    None
}

/// Reads `TWG_ACTIVE_ASSET=<uuid>` and resolves it from the local manifest.
#[cfg(not(target_arch = "wasm32"))]
fn env_selected_asset(cache: &Arc<Mutex<AssetCache>>) -> Option<UsedAsset> {
    let value = std::env::var("TWG_ACTIVE_ASSET").ok()?;
    let asset_id = Uuid::parse_str(value.trim()).ok()?;
    let cache = cache.lock().ok()?;
    let asset = cache.asset(asset_id)?.clone();
    let mut files = Vec::new();
    for source in &asset.sources {
        let (variant, path) = cache.resolve(&asset, source.slot, true)?;
        files.push(UsedAssetFile {
            slot: source.slot,
            path,
            variant,
        });
    }
    Some(UsedAsset {
        asset_id,
        name: asset.name.clone(),
        model_kind: asset.model_kind,
        files,
        prefer_variants: true,
    })
}

#[cfg(target_arch = "wasm32")]
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            output.push(high * 16 + low);
            index += 3;
            continue;
        }
        output.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

#[cfg(target_arch = "wasm32")]
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

// Keep this import used even when no screens reference it directly yet.

// ---------------------------------------------------------------- multipart

/// Builds a `multipart/form-data` body for `POST /admin/assets`.
pub(crate) fn multipart_asset_body(
    name: &str,
    category: AssetCategory,
    model_kind: Option<AssetModelKind>,
    move_mode: AssetMoveMode,
    group_id: Option<Uuid>,
    operasi_id: Option<Uuid>,
    files: &[PathBuf],
) -> Result<(String, Vec<u8>), String> {
    let boundary = format!("----twg{}", Uuid::new_v4().simple());
    let mut body = Vec::new();
    push_form_field(&mut body, &boundary, "name", name);
    push_form_field(&mut body, &boundary, "category", &category.to_string());
    if let Some(group_id) = group_id {
        push_form_field(&mut body, &boundary, "group_id", &group_id.to_string());
    }
    if let Some(operasi_id) = operasi_id {
        push_form_field(&mut body, &boundary, "operasi_id", &operasi_id.to_string());
    }
    if category.is_model() {
        push_form_field(&mut body, &boundary, "move_mode", &move_mode.to_string());
    }
    if let Some(kind) = model_kind {
        push_form_field(&mut body, &boundary, "model_kind", &kind.to_string());
    }
    let kind = model_kind.unwrap_or(AssetModelKind::Static);
    let slots = if category.is_model() {
        kind.slots().to_vec()
    } else {
        vec![AssetSourceSlot::Static]
    };
    for (index, path) in files.iter().enumerate() {
        let slot = slots.get(index).copied().unwrap_or(AssetSourceSlot::Static);
        let field = kind.upload_field(slot);
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("asset");
        let mime = mime_for_path(path);
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(
            &std::fs::read(path).map_err(|error| format!("gagal membaca {path:?}: {error}"))?,
        );
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Ok((format!("multipart/form-data; boundary={boundary}"), body))
}

/// Builds a `multipart/form-data` body for `POST /admin/assets/{id}/source`,
/// which replaces the file of one source slot.
fn multipart_asset_source_body(
    slot: AssetSourceSlot,
    path: &std::path::Path,
) -> Result<(String, Vec<u8>), String> {
    let boundary = format!("----twg{}", Uuid::new_v4().simple());
    let mut body = Vec::new();
    push_form_field(&mut body, &boundary, "slot", &slot.to_string());
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    let mime = mime_for_path(path);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        &std::fs::read(path).map_err(|error| format!("gagal membaca {path:?}: {error}"))?,
    );
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Ok((format!("multipart/form-data; boundary={boundary}"), body))
}

fn push_form_field(body: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        )
        .as_bytes(),
    );
}

fn mime_for_path(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("glb") => "model/gltf-binary",
        Some("gltf") => "model/gltf+json",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod asset_selection_tests {
    use super::{UsedAsset, env_selected_asset};
    use std::sync::{Arc, Mutex};
    use twg_client::asset_cache::AssetCache;

    #[test]
    fn env_selection_resolves_from_the_cache_manifest() {
        let root = std::env::temp_dir().join(format!(
            "twg-ui-env-asset-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let asset_dir = root
            .join("00000000-0000-4000-8000-000000000001")
            .join("static");
        std::fs::create_dir_all(&asset_dir).unwrap();
        std::fs::write(asset_dir.join("original.glb"), b"glb").unwrap();
        std::fs::write(
            root.join("manifest.json"),
            serde_json::json!({
                "saved_at": "now",
                "manifest": {
                    "generated_at": "2026-01-01T00:00:00+08:00",
                    "assets": [{
                        "id": "00000000-0000-4000-8000-000000000001",
                        "name": "cached model",
                        "category": "model",
                        "model_kind": "static",
                        "sources": [{
                            "slot": "static",
                            "file": {
                                "filename": "original.glb",
                                "url": "/assets/00000000-0000-4000-8000-000000000001/static/original.glb",
                                "mime": "model/gltf-binary",
                                "size_bytes": 3,
                                "sha256": ""
                            },
                            "has_draco": false,
                            "variants": []
                        }],
                        "version": 1,
                        "created_at": "2026-01-01T00:00:00+08:00",
                        "updated_at": "2026-01-01T00:00:00+08:00"
                    }]
                }
            })
            .to_string()
            .as_bytes(),
        )
        .unwrap();

        // SAFETY: single-threaded test setup before reading the variable.
        unsafe {
            std::env::set_var("TWG_ACTIVE_ASSET", "00000000-0000-4000-8000-000000000001");
        }
        let cache = Arc::new(Mutex::new(AssetCache::with_root(root.clone())));
        let selected: Option<UsedAsset> = env_selected_asset(&cache);
        unsafe {
            std::env::remove_var("TWG_ACTIVE_ASSET");
        }
        let selected = selected.expect("asset selected from cache");
        assert_eq!(selected.name, "cached model");
        assert_eq!(selected.files.len(), 1);
        assert!(selected.files[0].path.ends_with("original.glb"));
        let _ = std::fs::remove_dir_all(root);
    }
}
