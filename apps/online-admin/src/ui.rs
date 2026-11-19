//! Online-user list + session revocation UI.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy_egui::{EguiContexts, egui};
use twg_client::ApiClient;
use twg_client::api::{ApiError, ApiHandle, ApiResponse, HttpMethod};
use twg_client::connection::{ConnectionMode, ConnectionSettings, normalize_host};
use twg_client::design::{DARK, Rgb};
use twg_client::session::ClientSession;
use twg_contracts::users::{KickUserResponse, OnlineUser};
use twg_domain::Role;
use uuid::Uuid;

use bevy::app::AppExit;
use bevy::prelude::*;

const AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

pub fn session_path() -> PathBuf {
    if let Ok(path) = std::env::var("TWG_ADMIN_SESSION_FILE") {
        return path.into();
    }
    twg_client::native::FileTokenStore::default_path().with_file_name("online-admin-session.json")
}

/// Device id used by this app. Admins are single-device; set
/// `TWG_ADMIN_DEVICE_ID` to reuse the id of an existing admin session.
pub fn device_id() -> String {
    if let Ok(value) = std::env::var("TWG_ADMIN_DEVICE_ID") {
        let value = value.trim();
        if !value.is_empty() {
            return value.to_owned();
        }
    }
    let path = session_path().with_file_name("online-admin-device-id");
    if let Ok(value) = std::fs::read_to_string(&path) {
        let value = value.trim();
        if !value.is_empty() {
            return value.to_owned();
        }
    }
    let id = twg_client::store::generate_device_id();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, &id);
    id
}

#[derive(Debug, Clone, Copy)]
enum JobKind {
    Login,
    Logout,
    /// Reachability check for the selected connection.
    Health,
    /// Reachability check for the connection form before saving it.
    ConnectionTest,
    Online,
    Kick(Uuid),
}

struct UiJob {
    kind: JobKind,
    handle: ApiHandle,
}

#[derive(Resource)]
pub struct OnlineAdminUi {
    api: ApiClient,
    device_id: String,
    connection: ConnectionSettings,
    connection_mode: ConnectionMode,
    connection_host: String,
    connection_port: String,
    connection_message: Option<(bool, String)>,
    connection_testing: bool,
    connection_probe: Option<ApiClient>,
    exit_requested: bool,
    exit_confirm: bool,
    restored_session: bool,
    username: String,
    password: String,
    show_password: bool,
    session: Option<ClientSession>,
    users: Vec<OnlineUser>,
    jobs: Vec<UiJob>,
    logging_in: bool,
    loading: bool,
    error: Option<String>,
    notice: Option<String>,
    auto_refresh: bool,
    next_refresh: Option<Instant>,
    confirm: Option<OnlineUser>,
}

impl OnlineAdminUi {
    pub fn new(api: ApiClient, connection: ConnectionSettings) -> Self {
        let device_id = api.device_id();
        let session = api.restore();
        let restored_session = session.is_some();
        let connection_mode = connection.mode;
        let connection_host = connection.local_host.clone();
        let connection_port = connection.local_port.to_string();
        let mut ui = Self {
            api,
            device_id,
            connection,
            connection_mode,
            connection_host,
            connection_port,
            connection_message: None,
            connection_testing: false,
            connection_probe: None,
            exit_requested: false,
            exit_confirm: false,
            restored_session,
            username: String::new(),
            password: String::new(),
            show_password: false,
            session,
            users: Vec::new(),
            jobs: Vec::new(),
            logging_in: false,
            loading: false,
            error: None,
            notice: None,
            auto_refresh: true,
            next_refresh: None,
            confirm: None,
        };
        ui.begin_health_check();
        ui
    }

    fn begin_health_check(&mut self) {
        self.loading = true;
        let handle = self.api.health();
        self.track(JobKind::Health, handle);
    }

    /// Connection form values, or `None` when the local address is invalid.
    fn connection_draft(&self) -> Option<ConnectionSettings> {
        let host = normalize_host(&self.connection_host);
        let port = self.connection_port.trim().parse::<u16>().ok();
        if self.connection_mode == ConnectionMode::Local
            && (host.is_empty() || port.is_none_or(|port| port == 0))
        {
            return None;
        }
        Some(
            ConnectionSettings {
                mode: self.connection_mode,
                local_host: host,
                local_port: port.unwrap_or(self.connection.local_port),
            }
            .normalized(),
        )
    }

    fn begin_connection_test(&mut self) {
        let Some(settings) = self.connection_draft() else {
            self.connection_message =
                Some((false, "IP dan port server lokal tidak valid.".to_owned()));
            return;
        };
        self.connection_message = None;
        self.connection_testing = true;
        let probe = self.api.with_base_url(settings.base_url());
        let handle = probe.health();
        self.connection_probe = Some(probe);
        self.track(JobKind::ConnectionTest, handle);
    }

    fn save_connection(&mut self) {
        let Some(settings) = self.connection_draft() else {
            self.connection_message =
                Some((false, "IP dan port server lokal tidak valid.".to_owned()));
            return;
        };
        if let Err(error) = settings.save() {
            self.connection_message = Some((false, error));
            return;
        }
        self.apply_connection(settings);
    }

    /// Rebuilds the API client against the selected server and returns to the
    /// login screen; the old session belongs to the previous server.
    fn apply_connection(&mut self, settings: ConnectionSettings) {
        let base_url = settings.base_url();
        let _ = self.api.logout();
        self.api.clear_session();
        self.api = self.api.with_base_url(base_url);
        self.connection = settings.clone();
        self.connection_mode = settings.mode;
        self.connection_host = settings.local_host.clone();
        self.connection_port = settings.local_port.to_string();
        self.connection_probe = None;
        self.connection_testing = false;
        self.restored_session = false;
        self.session = None;
        self.users.clear();
        self.jobs.clear();
        self.logging_in = false;
        self.password.clear();
        self.show_password = false;
        self.error = None;
        self.notice = None;
        self.confirm = None;
        self.next_refresh = None;
        self.connection_message = None;
        self.begin_health_check();
    }

    fn track(&mut self, kind: JobKind, handle: ApiHandle) {
        self.jobs.push(UiJob { kind, handle });
    }

    fn begin_login(&mut self) {
        self.error = None;
        self.notice = None;
        if self.username.trim().is_empty() || self.password.is_empty() {
            self.error = Some("Username dan password wajib diisi.".to_owned());
            return;
        }
        self.logging_in = true;
        let handle = self
            .api
            .login_admin_tool(self.username.trim(), &self.password);
        self.track(JobKind::Login, handle);
    }

    fn logout(&mut self) {
        let handle = self.api.logout();
        self.track(JobKind::Logout, handle);
        self.session = None;
        self.users.clear();
        self.password.clear();
        self.show_password = false;
        self.notice = Some("Logout berhasil.".to_owned());
    }

    fn refresh_online(&mut self) {
        if self.session.is_none() || self.loading {
            return;
        }
        self.loading = true;
        self.next_refresh = Some(Instant::now() + AUTO_REFRESH_INTERVAL);
        let handle = self
            .api
            .request(HttpMethod::Get, "/private/users/online", None);
        self.track(JobKind::Online, handle);
    }

    fn kick(&mut self, user_id: Uuid) {
        self.error = None;
        self.notice = None;
        let handle = self.api.request(
            HttpMethod::Post,
            format!("/admin/users/{user_id}/kick-session"),
            None,
        );
        self.track(JobKind::Kick(user_id), handle);
    }

    fn poll(&mut self) {
        self.api.tick();
        if let Some(probe) = &self.connection_probe {
            probe.tick();
        }
        let jobs = std::mem::take(&mut self.jobs);
        for job in jobs {
            let Some(result) = job.handle.try_take() else {
                self.jobs.push(job);
                continue;
            };
            self.handle_job(job.kind, result);
        }
        if self.auto_refresh
            && self.session.is_some()
            && !self.loading
            && self
                .next_refresh
                .is_none_or(|deadline| Instant::now() >= deadline)
        {
            self.refresh_online();
        }
    }

    fn handle_job(&mut self, kind: JobKind, result: Result<ApiResponse, ApiError>) {
        match kind {
            JobKind::Login => {
                self.logging_in = false;
                match result {
                    Ok(_) => {
                        self.session = self.api.session();
                        self.password.clear();
                        self.show_password = false;
                        self.error = None;
                        self.notice = Some("Login berhasil.".to_owned());
                        self.refresh_online();
                    }
                    Err(error) => self.error = Some(error.message),
                }
            }
            JobKind::Logout => {
                self.session = None;
                self.users.clear();
            }
            JobKind::Health => {
                self.loading = false;
                match result {
                    Ok(_) => {
                        self.connection_message = Some((
                            true,
                            format!("Terhubung ke server {}.", self.connection.label()),
                        ));
                        if self.restored_session {
                            self.session = self.api.session();
                            self.refresh_online();
                        }
                    }
                    Err(error) => {
                        self.session = None;
                        self.users.clear();
                        self.connection_message = Some((
                            false,
                            format!(
                                "Koneksi ke server gagal. Coba pindah koneksi. ({})",
                                error.message
                            ),
                        ));
                    }
                }
            }
            JobKind::ConnectionTest => {
                self.connection_testing = false;
                self.connection_probe = None;
                let target = self.connection_draft().map_or_else(
                    || self.connection_mode.label().to_owned(),
                    |settings| settings.label(),
                );
                match result {
                    Ok(_) => {
                        self.connection_message =
                            Some((true, format!("Koneksi ke server {target} berhasil.")));
                    }
                    Err(error) => {
                        self.connection_message = Some((
                            false,
                            format!(
                                "Koneksi ke server {target} gagal. Periksa IP/port dan pastikan server berjalan. ({})",
                                error.message
                            ),
                        ));
                    }
                }
            }
            JobKind::Online => {
                self.loading = false;
                match result
                    .and_then(|response| response.list::<OnlineUser>().map(|(users, _total)| users))
                {
                    Ok(users) => {
                        self.users = users;
                        self.error = None;
                    }
                    Err(error) => self.handle_error(error),
                }
            }
            JobKind::Kick(user_id) => {
                match result.and_then(|response| response.data::<KickUserResponse>()) {
                    Ok(kicked) => {
                        self.notice = Some(format!(
                            "Sesi '{}' dicabut ({} sesi aktif). User bisa login lagi.",
                            kicked.username, kicked.revoked_sessions
                        ));
                        if self
                            .session
                            .as_ref()
                            .is_some_and(|session| session.user_id == user_id)
                        {
                            self.session = None;
                            self.users.clear();
                            self.error =
                                Some("Anda mencabut sesi sendiri; login ulang.".to_owned());
                        } else {
                            self.refresh_online();
                        }
                    }
                    Err(error) => self.handle_error(error),
                }
            }
        }
    }

    fn handle_error(&mut self, error: ApiError) {
        if error.status == Some(401) {
            self.api.clear_session();
            self.session = None;
            self.users.clear();
            self.error = Some("Sesi berakhir. Silakan login lagi.".to_owned());
        } else if error.status == Some(403) {
            self.error = Some("Role Anda tidak berhak mencabut sesi user.".to_owned());
        } else {
            self.error = Some(error.message);
        }
    }
}

// ---------------------------------------------------------------- drawing

pub fn draw(
    mut contexts: EguiContexts,
    mut admin: ResMut<OnlineAdminUi>,
    mut exit: MessageWriter<AppExit>,
) -> Result {
    let context = contexts.ctx_mut()?;
    apply_theme(context);
    admin.poll();

    if admin.session.is_none() {
        draw_login(context, &mut admin);
    } else {
        draw_online(context, &mut admin);
    }
    draw_confirm(context, &mut admin);
    if admin.exit_requested {
        admin.exit_requested = false;
        exit.write(AppExit::Success);
    }
    Ok(())
}

fn draw_login(context: &egui::Context, admin: &mut OnlineAdminUi) {
    root_ui(context, |ui| {
        egui::Area::new(egui::Id::new("online_admin_exit"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 12.0))
            .order(egui::Order::Foreground)
            .show(context, |ui| {
                if secondary(ui, "Keluar Aplikasi").clicked() {
                    admin.exit_confirm = true;
                }
            });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(380.0);
                ui.label(title("TWG Online Admin"));
                ui.label(muted(
                    "Masuk dengan akun admin untuk melihat user yang sedang online \
                 dan mencabut sesinya.",
                ));
                ui.add_space(16.0);

                egui::Frame::new()
                    .fill(color(DARK.card))
                    .stroke(egui::Stroke::new(1.0, color(DARK.border)))
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_width(340.0);
                        field(ui, "Username", &mut admin.username);
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(muted("Password"));
                        });
                        let mut submit = false;
                        ui.horizontal(|ui| {
                            let button_width = 28.0;
                            let input_width =
                                (ui.available_width() - button_width - ui.spacing().item_spacing.x)
                                    .max(120.0);
                            let password = ui.add(
                                egui::TextEdit::singleline(&mut admin.password)
                                    .password(!admin.show_password)
                                    .desired_width(input_width),
                            );
                            if password_toggle(ui, admin.show_password).clicked() {
                                admin.show_password = !admin.show_password;
                                password.request_focus();
                            }
                            submit = password.lost_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter));
                        });
                        ui.add_space(12.0);
                        let label = if admin.logging_in {
                            "Masuk..."
                        } else {
                            "Masuk"
                        };
                        if primary(ui, label).clicked() || submit {
                            admin.begin_login();
                        }
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(8.0);
                        connection_section(ui, admin);
                    });

                ui.add_space(10.0);
                if let Some(error) = &admin.error {
                    banner(ui, error, DARK.destructive);
                }
                if let Some(notice) = &admin.notice {
                    banner(ui, notice, DARK.success);
                }
                ui.add_space(8.0);
                ui.label(muted(format!(
                    "Koneksi aktif: {}",
                    admin.connection.label()
                )));
                ui.label(muted(format!("Device id: {}", admin.device_id)));
                ui.label(muted(
                    "Role admin bersifat single-device; gunakan TWG_ADMIN_DEVICE_ID \
                 bila ingin memakai device id yang sama dengan sesi lain.",
                ));
            });
        });
    });
}

fn draw_online(context: &egui::Context, admin: &mut OnlineAdminUi) {
    root_ui(context, |ui| {
        egui::Panel::top("topbar")
            .frame(
                egui::Frame::new()
                    .fill(color(DARK.sidebar))
                    .inner_margin(egui::Margin::symmetric(16, 10))
                    .stroke(egui::Stroke::new(1.0, color(DARK.border))),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(title("TWG Online Admin"));
                    ui.add_space(8.0);
                    if let Some(session) = &admin.session {
                        ui.label(muted(format!("{} · @{}", session.name, session.username)));
                        role_badge(ui, session.role);
                        if !session.role.is_admin() {
                            ui.label(
                                egui::RichText::new("butuh role admin").color(color(DARK.warning)),
                            );
                        }
                    }
                    ui.label(muted(format!("Koneksi: {}", admin.connection.label())));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary(ui, "Logout").clicked() {
                            admin.logout();
                        }
                        if secondary(ui, "Keluar Aplikasi").clicked() {
                            admin.exit_confirm = true;
                        }
                        if secondary(ui, "Muat ulang").clicked() {
                            admin.refresh_online();
                        }
                        ui.checkbox(&mut admin.auto_refresh, "Auto 5s");
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(color(DARK.background))
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(heading(format!("{} user online", admin.users.len())));
                    if admin.loading {
                        ui.spinner();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(muted(
                            "Sesi mencabut akses user; mereka dapat login lagi setelahnya.",
                        ));
                    });
                });
                ui.add_space(6.0);

                if let Some(error) = &admin.error {
                    banner(ui, error, DARK.destructive);
                    ui.add_space(6.0);
                }
                if let Some(notice) = &admin.notice {
                    banner(ui, notice, DARK.success);
                    ui.add_space(6.0);
                }

                if admin.users.is_empty() {
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| {
                        ui.label(muted("Tidak ada user dengan sesi aktif."));
                    });
                    return;
                }

                let users = admin.users.clone();
                let mut kick_target: Option<OnlineUser> = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("online-users")
                        .num_columns(7)
                        .striped(true)
                        .spacing(egui::vec2(16.0, 10.0))
                        .min_col_width(70.0)
                        .show(ui, |ui| {
                            for header in
                                ["Nama", "Username", "Role", "Sesi", "Device", "Terakhir", ""]
                            {
                                ui.label(
                                    egui::RichText::new(header)
                                        .size(11.0)
                                        .strong()
                                        .color(color(DARK.muted_foreground)),
                                );
                            }
                            ui.end_row();

                            for user in &users {
                                ui.label(user.name.as_str());
                                ui.label(format!("@{}", user.username));
                                role_badge(ui, user.role);
                                ui.label(user.active_session_count.to_string());
                                ui.label(device_summary(user));
                                ui.label(last_used(user));
                                if danger(ui, "Kick").clicked() {
                                    kick_target = Some(user.clone());
                                }
                                ui.end_row();
                            }
                        });
                });
                if let Some(user) = kick_target {
                    admin.confirm = Some(user);
                }
            });
    });
}

fn connection_section(ui: &mut egui::Ui, admin: &mut OnlineAdminUi) {
    ui.label(egui::RichText::new("Koneksi server").size(12.0).strong());
    let mut mode = admin.connection_mode;
    ui.horizontal(|ui| {
        ui.radio_value(&mut mode, ConnectionMode::Public, "Publik");
        ui.radio_value(&mut mode, ConnectionMode::Local, "Lokal");
    });
    if mode != admin.connection_mode {
        admin.connection_mode = mode;
        admin.connection_message = None;
    }
    if admin.connection_mode == ConnectionMode::Local {
        let host = &mut admin.connection_host;
        let port = &mut admin.connection_port;
        ui.columns(2, |columns| {
            field(&mut columns[0], "IP Address", host);
            field(&mut columns[1], "Port", port);
        });
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if secondary(ui, "Simpan").clicked() {
            admin.save_connection();
        }
        if secondary(ui, "Tes").clicked() {
            admin.begin_connection_test();
        }
        if admin.connection_testing {
            ui.spinner();
        }
    });
    if let Some((ok, message)) = &admin.connection_message {
        let rgb = if *ok { DARK.success } else { DARK.destructive };
        banner(ui, message, rgb);
    }
}

fn root_ui<R>(context: &egui::Context, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut ui = egui::Ui::new(
        context.clone(),
        egui::Id::new("twg_online_admin_root"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(context.viewport_rect()),
    );
    add(&mut ui)
}

fn draw_exit_confirm(context: &egui::Context, admin: &mut OnlineAdminUi) {
    if !admin.exit_confirm {
        return;
    }
    let mut close = false;
    egui::Window::new("Keluar aplikasi")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(context, |ui| {
            ui.set_min_width(320.0);
            ui.label("Tutup TWG Online Admin?");
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if danger(ui, "Keluar").clicked() {
                    admin.exit_requested = true;
                    close = true;
                }
                if secondary(ui, "Batal").clicked() {
                    close = true;
                }
            });
        });
    if close {
        admin.exit_confirm = false;
    }
}

fn draw_confirm(context: &egui::Context, admin: &mut OnlineAdminUi) {
    draw_exit_confirm(context, admin);
    let Some(user) = admin.confirm.clone() else {
        return;
    };
    let mut kick = false;
    let mut cancel = false;
    egui::Window::new("Cabut sesi user")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(context, |ui| {
            ui.set_min_width(360.0);
            ui.label(format!(
                "Cabut semua sesi aktif milik '{}' ({})?",
                user.name, user.username
            ));
            ui.add_space(4.0);
            ui.label(muted(
                "User akan ter-logout dan dapat login lagi dari perangkatnya.",
            ));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if danger(ui, "Kick sekarang").clicked() {
                    kick = true;
                }
                if secondary(ui, "Batal").clicked() {
                    cancel = true;
                }
            });
        });
    if kick {
        admin.confirm = None;
        match Uuid::parse_str(&user.id) {
            Ok(user_id) => admin.kick(user_id),
            Err(_) => admin.error = Some("User id tidak valid.".to_owned()),
        }
    } else if cancel {
        admin.confirm = None;
    }
}

// ---------------------------------------------------------------- helpers

fn apply_theme(context: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = color(DARK.background);
    visuals.window_fill = color(DARK.card);
    visuals.extreme_bg_color = color(DARK.input);
    visuals.widgets.inactive.bg_fill = color(DARK.secondary);
    visuals.widgets.inactive.weak_bg_fill = color(DARK.secondary);
    visuals.widgets.hovered.bg_fill = color(DARK.accent);
    visuals.widgets.active.bg_fill = color(DARK.accent);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, color(DARK.border));
    visuals.window_stroke = egui::Stroke::new(1.0, color(DARK.border));
    context.set_visuals(visuals);
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(muted(label));
    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
}

/// Icon-only show/hide button. Painted directly because the bundled fonts do
/// not contain a reliable eye glyph.
fn password_toggle(ui: &mut egui::Ui, visible: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect(
            rect,
            egui::CornerRadius::same(6),
            color(DARK.secondary),
            egui::Stroke::NONE,
            egui::StrokeKind::Inside,
        );
    }
    let tint = if response.hovered() {
        color(DARK.foreground)
    } else {
        color(DARK.muted_foreground)
    };
    paint_eye(painter, rect, visible, tint);
    response.on_hover_text(if visible {
        "Sembunyikan password"
    } else {
        "Tampilkan password"
    })
}

fn paint_eye(painter: &egui::Painter, rect: egui::Rect, open: bool, tint: egui::Color32) {
    let center = rect.center();
    let width = 15.0;
    let height = 9.0;
    let stroke = egui::Stroke::new(1.4, tint);
    let steps = 14;
    let mut points = Vec::with_capacity(steps * 2);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = center.x - width * 0.5 + width * t;
        let y = center.y - (std::f32::consts::PI * t).sin() * height * 0.5;
        points.push(egui::pos2(x, y));
    }
    for step in (1..steps).rev() {
        let t = step as f32 / steps as f32;
        let x = center.x - width * 0.5 + width * t;
        let y = center.y + (std::f32::consts::PI * t).sin() * height * 0.5;
        points.push(egui::pos2(x, y));
    }
    painter.add(egui::Shape::closed_line(points, stroke));
    if open {
        painter.circle_filled(center, 2.6, tint);
    } else {
        painter.line_segment(
            [
                center + egui::vec2(-width * 0.6, -height * 0.9),
                center + egui::vec2(width * 0.6, height * 0.9),
            ],
            stroke,
        );
    }
}

fn color(rgb: Rgb) -> egui::Color32 {
    egui::Color32::from_rgb(rgb.0, rgb.1, rgb.2)
}

fn heading(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(20.0).strong()
}

fn title(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(22.0).strong()
}

fn muted(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text)
        .size(12.0)
        .color(color(DARK.muted_foreground))
}

fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .size(13.0)
                .color(color(DARK.primary_foreground)),
        )
        .fill(color(DARK.primary))
        .min_size(egui::vec2(100.0, 30.0)),
    )
}

fn secondary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).size(13.0))
            .fill(color(DARK.secondary))
            .min_size(egui::vec2(84.0, 28.0)),
    )
}

fn danger(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .size(12.0)
                .color(color(DARK.destructive)),
        )
        .fill(color(DARK.secondary))
        .min_size(egui::vec2(64.0, 24.0)),
    )
}

fn banner(ui: &mut egui::Ui, text: &str, rgb: Rgb) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(
            rgb.0, rgb.1, rgb.2, 24,
        ))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(rgb.0, rgb.1, rgb.2, 140),
        ))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(12.0).color(color(rgb)));
        });
}

fn role_badge(ui: &mut egui::Ui, role: Role) {
    let (label, rgb) = match role {
        Role::Admin => ("admin", DARK.success),
        Role::AdminMonitor => ("admin_monitor", DARK.warning),
        Role::AdminSatker => ("admin_satker", DARK.warning),
        Role::UserSatker => ("user_satker", DARK.muted_foreground),
        Role::User => ("user", DARK.muted_foreground),
        Role::BigScreen => ("big_screen", DARK.muted_foreground),
    };
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(
            rgb.0, rgb.1, rgb.2, 32,
        ))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).size(11.0).color(color(rgb)));
        });
}

fn device_summary(user: &OnlineUser) -> String {
    let Some(first) = user.sessions.first() else {
        return "—".to_owned();
    };
    let device = short_id(&first.device_id);
    if user.sessions.len() > 1 {
        format!("{device} +{}", user.sessions.len() - 1)
    } else {
        device
    }
}

fn short_id(value: &str) -> String {
    if value.chars().count() <= 10 {
        value.to_owned()
    } else {
        format!("{}…", value.chars().take(10).collect::<String>())
    }
}

fn last_used(user: &OnlineUser) -> String {
    user.last_used_at
        .map(|value| value.format("%d %b %H:%M").to_string())
        .unwrap_or_else(|| "—".to_owned())
}
