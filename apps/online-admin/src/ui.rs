//! Online-user list + session revocation UI.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy_egui::{EguiContexts, egui};
use twg_client::ApiClient;
use twg_client::api::{ApiError, ApiHandle, ApiResponse, HttpMethod};
use twg_client::design::{DARK, Rgb};
use twg_client::session::ClientSession;
use twg_contracts::users::{KickUserResponse, OnlineUser};
use twg_domain::Role;
use uuid::Uuid;

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
    base_url: String,
    device_id: String,
    username: String,
    password: String,
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
    pub fn new(api: ApiClient, base_url: String) -> Self {
        let device_id = api.device_id();
        let session = api.restore();
        let mut ui = Self {
            api,
            base_url,
            device_id,
            username: String::new(),
            password: String::new(),
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
        if ui.session.is_some() {
            ui.refresh_online();
        }
        ui
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

pub fn draw(mut contexts: EguiContexts, mut admin: ResMut<OnlineAdminUi>) -> Result {
    let context = contexts.ctx_mut()?;
    apply_theme(context);
    admin.poll();

    if admin.session.is_none() {
        draw_login(context, &mut admin);
    } else {
        draw_online(context, &mut admin);
    }
    draw_confirm(context, &mut admin);
    Ok(())
}

fn draw_login(context: &egui::Context, admin: &mut OnlineAdminUi) {
    root_ui(context, |ui| {
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
                        let password = ui.add(
                            egui::TextEdit::singleline(&mut admin.password)
                                .password(true)
                                .desired_width(320.0),
                        );
                        let submit = password.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter));
                        ui.add_space(12.0);
                        let label = if admin.logging_in {
                            "Masuk..."
                        } else {
                            "Masuk"
                        };
                        if primary(ui, label).clicked() || submit {
                            admin.begin_login();
                        }
                    });

                ui.add_space(10.0);
                if let Some(error) = &admin.error {
                    banner(ui, error, DARK.destructive);
                }
                if let Some(notice) = &admin.notice {
                    banner(ui, notice, DARK.success);
                }
                ui.add_space(8.0);
                ui.label(muted(format!("Server: {}", admin.base_url)));
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
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary(ui, "Logout").clicked() {
                            admin.logout();
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

fn draw_confirm(context: &egui::Context, admin: &mut OnlineAdminUi) {
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
    ui.add(egui::TextEdit::singleline(value).desired_width(320.0));
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
