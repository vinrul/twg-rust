use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_egui::egui;
use chrono::NaiveDate;
use serde_json::{Value, json};
use twg_client::ApiClient;
use twg_client::api::{ApiError, ApiHandle, ApiResponse, HttpMethod};
use twg_client::realtime::{RealtimeState, RealtimeStatus, WsConnection, WsTransport};
use twg_client::session::{ClientSession, ClientState, NavItem, Screen};
use twg_contracts::categories::Category;
use twg_contracts::operations::{HomeOperation, Operation};
use twg_contracts::satkers::Satker;
use twg_contracts::scenarios::Scenario;
use twg_contracts::snapshots::Snapshot;
use twg_contracts::users::{OnlineUser, User};
use twg_domain::Role;
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
    pub center_x: String,
    pub center_y: String,
    pub zoom: String,
    pub min_zoom: String,
    pub max_zoom: String,
    pub pitch: String,
    pub bearing: String,
    pub map_mode: String,
    pub base_layer: String,
    pub indoor_config: String,
    pub copy: bool,
    pub error: Option<String>,
}

pub struct UserForm {
    pub id: Option<Uuid>,
    pub name: String,
    pub username: String,
    pub password: String,
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
    Logout,
}

pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub action: ConfirmAction,
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
    pub profile_message: Option<(bool, String)>,
    pub ws: WsUi,
    pub layer_order: Vec<&'static str>,
    pub selected_layer: &'static str,
    pub boot_checked: bool,
    pub last_screen: Screen,
    pub editor: EditorState,
    pending_editor_scenario: Option<Uuid>,
}

impl ClientUi {
    pub fn new(api: ApiClient, ws_transport: Arc<dyn WsTransport>, base_url: String) -> Self {
        let pending_editor_scenario = initial_editor_scenario();
        let mut state = ClientState::default();
        let session = api.restore();
        if session.is_some() {
            state.screen = Screen::Boot;
        } else {
            state.screen = Screen::Login;
        }
        Self {
            state,
            api,
            ws_transport,
            base_url,
            login_username: String::new(),
            login_password: String::new(),
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
            profile_message: None,
            ws: WsUi::default(),
            layer_order: vec!["OpenStreetMap", "Satelit"],
            selected_layer: "OpenStreetMap",
            boot_checked: false,
            last_screen: Screen::Boot,
            editor: EditorState::new(),
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
        self.tick_editor_jobs();
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
        }
    }

    fn enter_session(&mut self, session: ClientSession) {
        self.state.login(session);
        self.sync_profile_fields();
        self.refresh_all();
        if let Some(scenario) = self.pending_editor_scenario.take() {
            self.open_editor(Some(scenario));
        }
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
        self.state.navigate(item);
        if item == NavItem::Preview {
            self.open_editor(self.selected_scenario);
        } else if item == NavItem::Dashboard {
            self.refresh_all();
        } else if item == NavItem::Operations {
            self.refresh_operations();
        } else if item == NavItem::Scenarios {
            self.refresh_operations();
            self.refresh_scenarios();
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
        let number = |value: &str, fallback: f64| -> Result<f64, String> {
            if value.trim().is_empty() {
                Ok(fallback)
            } else {
                value
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| format!("Nilai '{value}' bukan angka."))
            }
        };
        let center_x = match number(&form.center_x, 0.0) {
            Ok(value) => value,
            Err(error) => {
                form.error = Some(error);
                return;
            }
        };
        let center_y = match number(&form.center_y, 0.0) {
            Ok(value) => value,
            Err(error) => {
                form.error = Some(error);
                return;
            }
        };
        let zoom = match number(&form.zoom, 2.0) {
            Ok(value) => value,
            Err(error) => {
                form.error = Some(error);
                return;
            }
        };
        let min_zoom = match number(&form.min_zoom, 2.0) {
            Ok(value) => value,
            Err(error) => {
                form.error = Some(error);
                return;
            }
        };
        let max_zoom = match number(&form.max_zoom, 26.0) {
            Ok(value) => value,
            Err(error) => {
                form.error = Some(error);
                return;
            }
        };
        let pitch = match number(&form.pitch, 0.0) {
            Ok(value) => value,
            Err(error) => {
                form.error = Some(error);
                return;
            }
        };
        let bearing = match number(&form.bearing, 0.0) {
            Ok(value) => value,
            Err(error) => {
                form.error = Some(error);
                return;
            }
        };
        let indoor_config: Value = serde_json::from_str(if form.indoor_config.trim().is_empty() {
            "{}"
        } else {
            form.indoor_config.trim()
        })
        .unwrap_or(Value::Object(serde_json::Map::new()));

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
            "center_x": center_x,
            "center_y": center_y,
            "zoom": zoom,
            "min_zoom": min_zoom,
            "max_zoom": max_zoom,
            "pitch": pitch,
            "bearing": bearing,
            "map_mode": form.map_mode,
            "base_layer": form.base_layer,
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
