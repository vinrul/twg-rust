use bevy_egui::egui;
use twg_client::design::{
    DARK, MOBILE_BREAKPOINT, SIDEBAR_WIDTH, SPACE_2, SPACE_3, SPACE_4, SPACE_6,
};
use twg_client::realtime::RealtimeStatus;
use twg_client::session::{NavItem, Screen};
use twg_contracts::auth::UserIdentity;
use twg_contracts::operations::Operation;
use twg_contracts::satkers::Satker;
use twg_contracts::scenarios::Scenario;
use twg_contracts::users::{OnlineUser, User};
use twg_domain::Role;
use uuid::Uuid;

use crate::state::{
    ClientUi, ConfirmAction, ConfirmDialog, JobKind, Modal, OperationForm, SatkerForm,
    ScenarioForm, ToastKind, UserForm,
};
use crate::theme::{self, color, color_alpha};
use crate::widgets;

pub fn draw(context: &egui::Context, client: &mut ClientUi) {
    match client.state.screen {
        Screen::Boot => draw_boot(context, client),
        Screen::Login => draw_login(context, client),
        Screen::Preview | Screen::MapEditor => crate::editor::panels::draw(context, client),
        Screen::Workspace => draw_shell(context, client),
    }
    draw_modal(context, client);
    draw_toasts(context, client);
}

fn root_ui<R>(context: &egui::Context, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut ui = egui::Ui::new(
        context.clone(),
        egui::Id::new("twg_client_root"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(context.viewport_rect()),
    );
    add(&mut ui)
}

fn draw_boot(context: &egui::Context, client: &mut ClientUi) {
    if !client.boot_checked && client.jobs.is_empty() {
        client.begin_identify();
    }
    root_ui(context, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(140.0);
                ui.spinner();
                ui.label(theme::muted("Memuat sesi..."));
            });
        });
    });
}

fn draw_login(context: &egui::Context, client: &mut ClientUi) {
    root_ui(context, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(90.0);
                widgets::card(ui, |ui| {
                    ui.set_width(360.0);
                    ui.label(
                        egui::RichText::new("TACTICAL GAME 3D")
                            .size(11.0)
                            .color(color(DARK.muted_foreground)),
                    );
                    ui.label(
                        egui::RichText::new("Masuk ke workspace")
                            .size(26.0)
                            .strong(),
                    );
                    ui.add_space(SPACE_2);
                    ui.label(theme::muted(
                        "Gunakan akun operasi. Device ID dipakai untuk session per perangkat.",
                    ));
                    ui.add_space(SPACE_4);
                    widgets::labeled_input(ui, "Username", &mut client.login_username);
                    widgets::password_input(ui, "Password", &mut client.login_password);
                    ui.label(theme::muted(&format!("Device: {}", client.api.device_id())));
                    if let Some(error) = &client.login_error {
                        ui.add_space(SPACE_2);
                        widgets::error_banner(ui, error);
                    }
                    ui.add_space(SPACE_3);
                    let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
                    ui.horizontal(|ui| {
                        if client.logging_in {
                            widgets::loading(ui, "Memeriksa kredensial...");
                        } else if widgets::primary_button(ui, "Masuk").clicked() || enter {
                            client.begin_login();
                        }
                    });
                    ui.add_space(SPACE_2);
                    widgets::key_hint(ui, "Enter", "Masuk");
                });
            });
        });
    });
}

fn draw_shell(context: &egui::Context, client: &mut ClientUi) {
    let width = context.viewport_rect().width();
    let compact = width < MOBILE_BREAKPOINT;

    root_ui(context, |ui| {
        egui::Panel::top("topbar")
            .frame(
                egui::Frame::new()
                    .fill(color(DARK.sidebar))
                    .inner_margin(egui::Margin::symmetric(SPACE_3 as i8, SPACE_2 as i8))
                    .stroke(egui::Stroke::new(1.0, color(DARK.border))),
            )
            .show(ui, |ui| draw_topbar(ui, client, compact));

        if compact {
            egui::Panel::top("mobile_nav")
                .frame(egui::Frame::new().fill(color(DARK.background)))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for item in NavItem::visible_items(client.role()) {
                            if ui
                                .selectable_label(client.state.nav == item, item.label())
                                .clicked()
                            {
                                client.go(item);
                            }
                        }
                    });
                });
        } else {
            egui::Panel::left("sidebar")
                .exact_size(SIDEBAR_WIDTH)
                .frame(
                    egui::Frame::new()
                        .fill(color(DARK.sidebar))
                        .inner_margin(egui::Margin::symmetric(SPACE_2 as i8, SPACE_3 as i8)),
                )
                .show(ui, |ui| draw_sidebar(ui, client));
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(color(DARK.background))
                    .inner_margin(egui::Margin::same(SPACE_4 as i8)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    draw_screen(ui, client);
                });
            });
    });
}

fn draw_topbar(ui: &mut egui::Ui, client: &mut ClientUi, compact: bool) {
    ui.horizontal(|ui| {
        if !compact {
            ui.label(
                egui::RichText::new("TWG")
                    .size(20.0)
                    .strong()
                    .color(color(DARK.foreground)),
            );
            ui.separator();
        }
        ui.label(
            egui::RichText::new(client.state.nav.label())
                .size(15.0)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::ghost_button(ui, "Logout").clicked() {
                client.modal = Modal::Confirm(ConfirmDialog {
                    title: "Keluar".to_owned(),
                    message: "Keluar dari sesi ini?".to_owned(),
                    action: ConfirmAction::Logout,
                });
            }
            if let Some(session) = &client.state.session {
                ui.label(theme::muted(&session.name));
                widgets::badge(ui, &role_label(session.role), DARK.success);
            }
            let status = client.ws.state.status.clone();
            let status_color = match status {
                RealtimeStatus::Connected => DARK.success,
                RealtimeStatus::Connecting | RealtimeStatus::Reconnecting { .. } => DARK.warning,
                _ => DARK.muted_foreground,
            };
            widgets::badge(
                ui,
                if status.is_connected() {
                    "realtime"
                } else {
                    "offline"
                },
                status_color,
            );
        });
    });
}

fn draw_sidebar(ui: &mut egui::Ui, client: &mut ClientUi) {
    ui.add_space(SPACE_2);
    ui.label(
        egui::RichText::new("TACTICAL GAME 3D")
            .size(10.0)
            .color(color(DARK.muted_foreground)),
    );
    ui.add_space(SPACE_4);
    for item in NavItem::visible_items(client.role()) {
        let selected = client.state.nav == item;
        let response = ui.add(
            egui::Button::new(
                egui::RichText::new(format!("{}   {}", item.icon(), item.label()))
                    .size(13.0)
                    .color(if selected {
                        color(DARK.foreground)
                    } else {
                        color(DARK.muted_foreground)
                    }),
            )
            .fill(if selected {
                color(DARK.sidebar_accent)
            } else {
                egui::Color32::TRANSPARENT
            })
            .min_size(egui::vec2(SIDEBAR_WIDTH - 24.0, 32.0)),
        );
        if response.clicked() {
            client.go(item);
        }
        if item == NavItem::Preview {
            ui.add_space(SPACE_2);
            ui.separator();
            ui.add_space(SPACE_2);
        }
    }
    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        ui.label(theme::muted(env!("CARGO_PKG_VERSION")));
        if let Some(session) = &client.state.session {
            ui.label(theme::muted(&session.username));
            if let Some(satker_id) = session.satker_id {
                ui.label(theme::muted(&format!("satker {}", short_id(satker_id))));
            }
        }
    });
}

fn draw_screen(ui: &mut egui::Ui, client: &mut ClientUi) {
    match client.state.nav {
        NavItem::Dashboard => screen_dashboard(ui, client),
        NavItem::Operations => screen_operations(ui, client),
        NavItem::Scenarios => screen_scenarios(ui, client),
        NavItem::Games => screen_games(ui, client),
        NavItem::Users => screen_users(ui, client),
        NavItem::Satkers => screen_satkers(ui, client),
        NavItem::Online => screen_online(ui, client),
        NavItem::Snapshots => screen_snapshots(ui, client),
        NavItem::Profile => screen_profile(ui, client),
        NavItem::Preview => {}
    }
}

// ---------------------------------------------------------------- screens

fn screen_dashboard(ui: &mut egui::Ui, client: &mut ClientUi) {
    widgets::section(
        ui,
        "Dashboard",
        Some("Ringkasan operasi, skenario, pengguna, dan koneksi aktif."),
    );
    ui.horizontal_wrapped(|ui| {
        widgets::stat_card(
            ui,
            "Operasi",
            &client.data.operations.len().to_string(),
            "Total terlihat",
        );
        widgets::stat_card(
            ui,
            "Skenario",
            &client.data.scenarios.len().to_string(),
            "Operasi terpilih",
        );
        widgets::stat_card(
            ui,
            "Pengguna",
            &client.data.users.len().to_string(),
            "Tanpa admin",
        );
        widgets::stat_card(
            ui,
            "Satker",
            &client.data.satkers.len().to_string(),
            "Unit organisasi",
        );
        widgets::stat_card(
            ui,
            "Online",
            &client.data.online.len().to_string(),
            "Session aktif",
        );
    });
    ui.add_space(SPACE_6);
    ui.horizontal(|ui| {
        ui.label(theme::title("Aksi cepat"));
        if widgets::ghost_button(ui, "Muat ulang").clicked() {
            client.refresh_all();
        }
    });
    ui.add_space(SPACE_2);
    ui.horizontal_wrapped(|ui| {
        if widgets::primary_button(ui, "Operasi baru").clicked() {
            client.modal = Modal::Operation(new_operation_form(client));
        }
        if widgets::secondary_button(ui, "Skenario").clicked() {
            client.go(NavItem::Scenarios);
        }
        if widgets::secondary_button(ui, "Latihan").clicked() {
            client.go(NavItem::Games);
        }
        if widgets::secondary_button(ui, "Buka peta").clicked() {
            if let Some(scenario) = client.data.scenarios.first() {
                client.selected_scenario = Some(scenario.id);
                client.ws.scenario = Some(scenario.id);
            }
            client.go(NavItem::Preview);
        }
    });
    ui.add_space(SPACE_6);
    widgets::card(ui, |ui| {
        ui.label(theme::title("Operasi terbaru"));
        if client.data.operations.is_empty() {
            widgets::empty(ui, "Belum ada operasi yang dapat ditampilkan.");
        } else {
            let operations = client
                .data
                .operations
                .iter()
                .take(6)
                .cloned()
                .collect::<Vec<_>>();
            widgets::table(ui, "dashboard_ops", &["Nama", "Satker", "Dibuat"], |ui| {
                for operation in operations {
                    ui.label(&operation.name);
                    ui.label(
                        operation
                            .satker_id
                            .map(short_id)
                            .unwrap_or_else(|| "—".to_owned()),
                    );
                    ui.label(operation.created_at.format("%d %b %Y").to_string());
                    ui.end_row();
                }
            });
        }
    });
}

fn screen_operations(ui: &mut egui::Ui, client: &mut ClientUi) {
    widgets::section(ui, "Operasi", Some("Kelola operasi dan penugasan satker."));
    let can_write = client.role().is_admin_like();
    ui.horizontal_wrapped(|ui| {
        let response = widgets::search_input(ui, &mut client.operation_search, "Cari operasi...");
        if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            client.refresh_operations();
        }
        if widgets::secondary_button(ui, "Cari").clicked() {
            client.refresh_operations();
        }
        if widgets::ghost_button(ui, "Reset").clicked() {
            client.operation_search.clear();
            client.refresh_operations();
        }
        if can_write && widgets::primary_button(ui, "Operasi baru").clicked() {
            client.modal = Modal::Operation(new_operation_form(client));
        }
    });
    ui.add_space(SPACE_3);
    if client.data.is_loading("operations") {
        widgets::loading(ui, "Memuat operasi...");
    }
    if client.data.operations.is_empty() {
        widgets::empty(ui, "Belum ada operasi.");
        return;
    }
    let operations = client.data.operations.clone();
    let satkers = client.data.satkers.clone();
    let mut edit = None;
    let mut delete = None;
    widgets::table(
        ui,
        "operations",
        &["Nama", "Satker", "Dibuat", "Diperbarui", ""],
        |ui| {
            for operation in &operations {
                ui.label(&operation.name);
                ui.label(
                    operation
                        .satker_id
                        .and_then(|id| satkers.iter().find(|satker| satker.id == id))
                        .map(|satker| satker.name.clone())
                        .unwrap_or_else(|| "—".to_owned()),
                );
                ui.label(operation.created_at.format("%d %b %Y %H:%M").to_string());
                ui.label(operation.updated_at.format("%d %b %Y %H:%M").to_string());
                ui.horizontal(|ui| {
                    if can_write {
                        if widgets::ghost_button(ui, "Ubah").clicked() {
                            edit = Some(operation.clone());
                        }
                        if widgets::ghost_button(ui, "Hapus").clicked() {
                            delete = Some(operation.clone());
                        }
                    }
                });
                ui.end_row();
            }
        },
    );
    if let Some(operation) = edit {
        client.modal = Modal::Operation(operation_form(operation));
    }
    if let Some(operation) = delete {
        client.modal = Modal::Confirm(ConfirmDialog {
            title: "Hapus operasi".to_owned(),
            message: format!(
                "Hapus operasi '{}'? Semua skenario di dalamnya ikut terhapus.",
                operation.name
            ),
            action: ConfirmAction::DeleteOperation(operation.id),
        });
    }
}

fn screen_scenarios(ui: &mut egui::Ui, client: &mut ClientUi) {
    widgets::section(ui, "Skenario", Some("Daftar skenario per operasi."));
    let can_write = client.role().is_admin_like();
    let operations = client.data.operations.clone();
    ui.horizontal_wrapped(|ui| {
        ui.label("Operasi");
        let mut selected = client.scenario_operation.clone();
        let options = operations
            .iter()
            .map(|operation| (operation.id.to_string(), operation.name.clone()))
            .collect::<Vec<_>>();
        widgets::combo(
            ui,
            egui::Id::new("scenario_operation"),
            &mut selected,
            &options,
        );
        if selected != client.scenario_operation {
            client.scenario_operation = selected;
            client.refresh_scenarios();
        }
        widgets::search_input(ui, &mut client.scenario_search, "Cari skenario...");
        if widgets::secondary_button(ui, "Muat").clicked() {
            client.refresh_scenarios();
        }
        if can_write && widgets::primary_button(ui, "Skenario baru").clicked() {
            client.modal = Modal::Scenario(new_scenario_form(client));
        }
    });
    ui.add_space(SPACE_3);
    if client.data.is_loading("scenarios") {
        widgets::loading(ui, "Memuat skenario...");
    }
    let needle = client.scenario_search.trim().to_lowercase();
    let scenarios = client
        .data
        .scenarios
        .iter()
        .filter(|scenario| needle.is_empty() || scenario.name.to_lowercase().contains(&needle))
        .cloned()
        .collect::<Vec<_>>();
    if scenarios.is_empty() {
        widgets::empty(ui, "Belum ada skenario pada operasi ini.");
        return;
    }
    let mut edit = None;
    let mut copy = None;
    let mut delete = None;
    let mut preview = None;
    widgets::table(
        ui,
        "scenarios",
        &["Nama", "Zoom", "Mode", "Layer", "Diperbarui", ""],
        |ui| {
            for scenario in &scenarios {
                ui.label(&scenario.name);
                ui.label(format!("{:.1} / {:.1}", scenario.zoom, scenario.max_zoom));
                ui.label(format!("{:?}", scenario.map_mode).to_lowercase());
                ui.label(format!("{:?}", scenario.base_layer).to_lowercase());
                ui.label(scenario.updated_at.format("%d %b %Y %H:%M").to_string());
                ui.horizontal(|ui| {
                    if widgets::ghost_button(ui, "Peta").clicked() {
                        preview = Some(scenario.clone());
                    }
                    if can_write {
                        if widgets::ghost_button(ui, "Ubah").clicked() {
                            edit = Some(scenario.clone());
                        }
                        if widgets::ghost_button(ui, "Salin").clicked() {
                            copy = Some(scenario.clone());
                        }
                        if widgets::ghost_button(ui, "Hapus").clicked() {
                            delete = Some(scenario.clone());
                        }
                    }
                });
                ui.end_row();
            }
        },
    );
    if let Some(scenario) = preview {
        client.selected_scenario = Some(scenario.id);
        client.ws.scenario = Some(scenario.id);
        client.go(NavItem::Preview);
    }
    if let Some(scenario) = edit {
        client.modal = Modal::Scenario(scenario_form(scenario));
    }
    if let Some(scenario) = copy {
        client.modal = Modal::Scenario(copy_scenario_form(client, scenario));
    }
    if let Some(scenario) = delete {
        client.modal = Modal::Confirm(ConfirmDialog {
            title: "Hapus skenario".to_owned(),
            message: format!("Hapus skenario '{}' beserta seluruh isinya?", scenario.name),
            action: ConfirmAction::DeleteScenario(scenario.id),
        });
    }
}

fn screen_games(ui: &mut egui::Ui, client: &mut ClientUi) {
    widgets::section(
        ui,
        "Latihan",
        Some("Operasi dan skenario yang siap dijalankan."),
    );
    if client.data.is_loading("home") {
        widgets::loading(ui, "Memuat latihan...");
    }
    if client.data.home.is_empty() {
        widgets::empty(ui, "Belum ada operasi dengan skenario.");
        return;
    }
    let home = client.data.home.clone();
    let mut preview = None;
    let mut open_operation = None;
    for operation in home {
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::title(&operation.name));
                if let Some(satker) = operation.satker_id {
                    widgets::badge(ui, &short_id(satker), DARK.warning);
                }
            });
            ui.add_space(SPACE_2);
            ui.horizontal_wrapped(|ui| {
                for scenario in &operation.skenario {
                    let response = ui.add(
                        egui::Button::new(egui::RichText::new(&scenario.name).size(13.0))
                            .fill(color(DARK.secondary))
                            .corner_radius(egui::CornerRadius::same(8)),
                    );
                    if response.clicked() {
                        preview = Some(scenario.scenario_id);
                    }
                }
            });
            ui.add_space(SPACE_2);
            ui.horizontal(|ui| {
                if widgets::ghost_button(ui, "Kelola skenario").clicked() {
                    open_operation = Some(operation.operation_id);
                }
            });
        });
        ui.add_space(SPACE_3);
    }
    if let Some(operation_id) = open_operation {
        client.scenario_operation = operation_id.to_string();
        client.go(NavItem::Scenarios);
    }
    if let Some(scenario_id) = preview {
        client.selected_scenario = Some(scenario_id);
        client.ws.scenario = Some(scenario_id);
        client.go(NavItem::Preview);
    }
}

fn screen_users(ui: &mut egui::Ui, client: &mut ClientUi) {
    widgets::section(ui, "Pengguna", Some("Kelola akun dan penugasan satker."));
    if !client.role().is_admin_like() {
        widgets::empty(ui, "Role Anda tidak memiliki akses ke manajemen pengguna.");
        return;
    }
    let can_write = client
        .session()
        .map(|session| session.can_write_users())
        .unwrap_or(false);
    ui.horizontal_wrapped(|ui| {
        if widgets::secondary_button(ui, "Muat ulang").clicked() {
            client.refresh_users();
        }
        if can_write && widgets::primary_button(ui, "Pengguna baru").clicked() {
            client.modal = Modal::User(new_user_form(client));
        }
    });
    ui.add_space(SPACE_3);
    if client.data.is_loading("users") {
        widgets::loading(ui, "Memuat pengguna...");
    }
    let users = client.data.users.clone();
    let satkers = client.data.satkers.clone();
    let mut edit = None;
    let mut delete = None;
    if users.is_empty() {
        widgets::empty(ui, "Belum ada pengguna.");
        return;
    }
    widgets::table(
        ui,
        "users",
        &["Nama", "Username", "Role", "Satker", ""],
        |ui| {
            for user in &users {
                ui.label(&user.name);
                ui.label(&user.username);
                widgets::badge(ui, &role_label(user.role), DARK.success);
                ui.label(
                    user.satker_id
                        .and_then(|id| satkers.iter().find(|satker| satker.id == id))
                        .map(|satker| satker.name.clone())
                        .unwrap_or_else(|| "—".to_owned()),
                );
                ui.horizontal(|ui| {
                    if can_write {
                        if widgets::ghost_button(ui, "Ubah").clicked() {
                            edit = Some(user.clone());
                        }
                        if widgets::ghost_button(ui, "Hapus").clicked() {
                            delete = Some(user.clone());
                        }
                    }
                });
                ui.end_row();
            }
        },
    );
    if let Some(user) = edit {
        client.modal = Modal::User(user_form(&user));
    }
    if let Some(user) = delete {
        client.modal = Modal::Confirm(ConfirmDialog {
            title: "Hapus pengguna".to_owned(),
            message: format!("Hapus pengguna '{}'?", user.username),
            action: ConfirmAction::DeleteUser(user.id),
        });
    }
}

fn screen_satkers(ui: &mut egui::Ui, client: &mut ClientUi) {
    widgets::section(ui, "Satker", Some("Unit organisasi dan status aktifnya."));
    let can_write = client
        .session()
        .map(|session| session.can_write_satkers())
        .unwrap_or(false);
    ui.horizontal_wrapped(|ui| {
        if widgets::secondary_button(ui, "Muat ulang").clicked() {
            client.refresh_satkers();
        }
        if can_write && widgets::primary_button(ui, "Satker baru").clicked() {
            client.modal = Modal::Satker(SatkerForm {
                id: None,
                code: String::new(),
                name: String::new(),
                is_active: true,
                error: None,
            });
        }
    });
    ui.add_space(SPACE_3);
    if client.data.is_loading("satkers") {
        widgets::loading(ui, "Memuat satker...");
    }
    let satkers = client.data.satkers.clone();
    let mut edit = None;
    let mut delete = None;
    if satkers.is_empty() {
        widgets::empty(ui, "Belum ada satker.");
        return;
    }
    widgets::table(ui, "satkers", &["Kode", "Nama", "Aktif", ""], |ui| {
        for satker in &satkers {
            ui.label(&satker.code);
            ui.label(&satker.name);
            widgets::badge(
                ui,
                if satker.is_active {
                    "aktif"
                } else {
                    "nonaktif"
                },
                if satker.is_active {
                    DARK.success
                } else {
                    DARK.muted_foreground
                },
            );
            ui.horizontal(|ui| {
                if can_write {
                    if widgets::ghost_button(ui, "Ubah").clicked() {
                        edit = Some(satker.clone());
                    }
                    if widgets::ghost_button(ui, "Hapus").clicked() {
                        delete = Some(satker.clone());
                    }
                }
            });
            ui.end_row();
        }
    });
    if let Some(satker) = edit {
        client.modal = Modal::Satker(SatkerForm {
            id: Some(satker.id),
            code: satker.code,
            name: satker.name,
            is_active: satker.is_active,
            error: None,
        });
    }
    if let Some(satker) = delete {
        client.modal = Modal::Confirm(ConfirmDialog {
            title: "Hapus satker".to_owned(),
            message: format!("Hapus satker '{}'?", satker.name),
            action: ConfirmAction::DeleteSatker(satker.id),
        });
    }
}

fn screen_online(ui: &mut egui::Ui, client: &mut ClientUi) {
    widgets::section(ui, "Online", Some("Pengguna dengan session aktif."));
    if widgets::secondary_button(ui, "Muat ulang").clicked() {
        client.refresh_online();
    }
    ui.add_space(SPACE_3);
    if client.data.is_loading("online") {
        widgets::loading(ui, "Memuat session...");
    }
    let online = client.data.online.clone();
    if online.is_empty() {
        widgets::empty(ui, "Tidak ada pengguna online.");
        return;
    }
    widgets::table(
        ui,
        "online",
        &["Nama", "Username", "Role", "Session", "Terakhir"],
        |ui| {
            for user in &online {
                ui.label(&user.name);
                ui.label(&user.username);
                widgets::badge(ui, &role_label(user.role), DARK.success);
                ui.label(user.active_session_count.to_string());
                ui.label(
                    user.last_used_at
                        .map(|value| value.format("%d %b %Y %H:%M").to_string())
                        .unwrap_or_else(|| "—".to_owned()),
                );
                ui.end_row();
            }
        },
    );
}

fn screen_snapshots(ui: &mut egui::Ui, client: &mut ClientUi) {
    widgets::section(
        ui,
        "Snapshot",
        Some("Status snapshot game session per skenario."),
    );
    let operations = client.data.operations.clone();
    let scenarios = client.data.scenarios.clone();
    ui.horizontal_wrapped(|ui| {
        ui.label("Operasi");
        let mut selected = client.scenario_operation.clone();
        let options = operations
            .iter()
            .map(|operation| (operation.id.to_string(), operation.name.clone()))
            .collect::<Vec<_>>();
        widgets::combo(
            ui,
            egui::Id::new("snapshot_operation"),
            &mut selected,
            &options,
        );
        if selected != client.scenario_operation {
            client.scenario_operation = selected;
            client.refresh_scenarios();
        }
        ui.label("Skenario");
        let mut scenario_text = client
            .selected_scenario
            .map(|id| id.to_string())
            .unwrap_or_default();
        let scenario_options = scenarios
            .iter()
            .map(|scenario| (scenario.id.to_string(), scenario.name.clone()))
            .collect::<Vec<_>>();
        widgets::combo(
            ui,
            egui::Id::new("snapshot_scenario"),
            &mut scenario_text,
            &scenario_options,
        );
        if let Ok(id) = Uuid::parse_str(&scenario_text)
            && Some(id) != client.selected_scenario
        {
            client.selected_scenario = Some(id);
            client.ws.scenario = Some(id);
            client.load_snapshot(id);
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Filter tanggal");
        widgets::date_picker(
            ui,
            egui::Id::new("snapshot_date"),
            &mut client.snapshot_date,
        );
        if widgets::secondary_button(ui, "Muat snapshot").clicked()
            && let Some(id) = client.selected_scenario
        {
            client.load_snapshot(id);
        }
    });
    ui.add_space(SPACE_3);
    if client.data.is_loading("snapshot") {
        widgets::loading(ui, "Memuat snapshot...");
    }
    let Some(snapshot) = client.data.snapshot.clone() else {
        widgets::empty(ui, "Skenario ini belum memiliki snapshot.");
        return;
    };
    if !client.snapshot_matches_filter(&snapshot) {
        widgets::empty(ui, "Snapshot tidak cocok dengan filter tanggal.");
        return;
    }
    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::title("Snapshot game session"));
            widgets::badge(
                ui,
                if snapshot.snapshot_enabled {
                    "enabled"
                } else {
                    "disabled"
                },
                if snapshot.snapshot_enabled {
                    DARK.success
                } else {
                    DARK.warning
                },
            );
        });
        ui.label(theme::muted(&format!("Versi {}", snapshot.version)));
        ui.label(theme::muted(&format!(
            "Diperbarui {}",
            snapshot.updated_at.format("%d %b %Y %H:%M")
        )));
        ui.add_space(SPACE_2);
        ui.horizontal(|ui| {
            let mut enabled = snapshot.snapshot_enabled;
            if widgets::switch(ui, &mut enabled, "Aktifkan snapshot").changed() {
                client.toggle_snapshot(enabled);
            }
            if widgets::danger_button(ui, "Hapus snapshot").clicked() {
                client.modal = Modal::Confirm(ConfirmDialog {
                    title: "Hapus snapshot".to_owned(),
                    message: "Snapshot akan dihapus dan klien harus resync.".to_owned(),
                    action: ConfirmAction::DeleteSnapshot(snapshot.skenario_id),
                });
            }
        });
        ui.add_space(SPACE_2);
        ui.collapsing("Lihat state", |ui| {
            ui.monospace(format!("map_state: {}", snapshot.map_state));
            ui.monospace(format!("markers_state: {}", snapshot.markers_state));
            ui.monospace(format!("anims_state: {}", snapshot.anims_state));
        });
    });
}

fn screen_profile(ui: &mut egui::Ui, client: &mut ClientUi) {
    widgets::section(ui, "Profil", Some("Perbarui identitas dan password akun."));
    let Some(session) = client.state.session.clone() else {
        return;
    };
    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::title(&session.name));
            widgets::badge(ui, &role_label(session.role), DARK.success);
        });
        ui.label(theme::muted(&format!("@{}", session.username)));
        ui.label(theme::muted(&format!("User ID {}", session.user_id)));
        if let Some(satker_id) = session.satker_id {
            ui.label(theme::muted(&format!("Satker {}", satker_id)));
        }
    });
    ui.add_space(SPACE_4);
    widgets::card(ui, |ui| {
        ui.label(theme::title("Identitas"));
        widgets::labeled_input(ui, "Nama", &mut client.profile_name);
        widgets::labeled_input(ui, "URL foto profil", &mut client.profile_img);
        if widgets::primary_button(ui, "Simpan profil").clicked() {
            client.save_profile();
        }
    });
    ui.add_space(SPACE_4);
    widgets::card(ui, |ui| {
        ui.label(theme::title("Ganti password"));
        widgets::password_input(ui, "Password lama", &mut client.old_password);
        widgets::password_input(ui, "Password baru", &mut client.new_password);
        if widgets::primary_button(ui, "Ganti password").clicked() {
            client.change_password();
        }
        if let Some((ok, message)) = &client.profile_message {
            ui.add_space(SPACE_2);
            if *ok {
                widgets::success_banner(ui, message);
            } else {
                widgets::error_banner(ui, message);
            }
        }
    });
    ui.add_space(SPACE_4);
    let identity = IdentitySummary::from_session(&session);
    widgets::card(ui, |ui| {
        ui.label(theme::title("Session"));
        ui.label(theme::muted(&format!("Device {}", client.api.device_id())));
        ui.label(theme::muted(&format!("Role {}", identity.role_label)));
        if widgets::secondary_button(ui, "Logout").clicked() {
            client.modal = Modal::Confirm(ConfirmDialog {
                title: "Keluar".to_owned(),
                message: "Keluar dari sesi ini?".to_owned(),
                action: ConfirmAction::Logout,
            });
        }
    });
}

// ---------------------------------------------------------------- modals

enum ModalAction {
    None,
    SaveOperation,
    SaveScenario,
    SaveUser,
    SaveSatker,
    Confirm(ConfirmAction),
}

fn draw_modal(context: &egui::Context, client: &mut ClientUi) {
    if !client.modal.is_open() {
        return;
    }
    let mut modal = std::mem::replace(&mut client.modal, Modal::None);
    let mut open = true;
    let mut request_close = false;
    let mut action = ModalAction::None;
    match &mut modal {
        Modal::Operation(form) => {
            let title = if form.id.is_some() {
                "Ubah operasi"
            } else {
                "Operasi baru"
            };
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .min_width(360.0)
                .show(context, |ui| {
                    widgets::labeled_input(ui, "Nama operasi", &mut form.name);
                    if client.role().sees_all_satkers() {
                        let mut satker = form.satker_id.clone();
                        let options = satker_options(client);
                        widgets::combo(ui, egui::Id::new("op_satker"), &mut satker, &options);
                        form.satker_id = satker;
                    }
                    if let Some(error) = &form.error {
                        widgets::error_banner(ui, error);
                    }
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, "Simpan").clicked() {
                            action = ModalAction::SaveOperation;
                        }
                        if widgets::ghost_button(ui, "Batal").clicked() {
                            request_close = true;
                        }
                    });
                });
        }
        Modal::Scenario(form) => {
            let title = if form.copy {
                "Salin skenario"
            } else if form.id.is_some() {
                "Ubah skenario"
            } else {
                "Skenario baru"
            };
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(460.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(context, |ui| {
                    widgets::labeled_input(ui, "Nama skenario", &mut form.name);
                    if form.copy {
                        let mut target = form.target_operasi_id.clone();
                        let options = client
                            .data
                            .operations
                            .iter()
                            .map(|operation| (operation.id.to_string(), operation.name.clone()))
                            .collect::<Vec<_>>();
                        widgets::combo(ui, egui::Id::new("copy_target"), &mut target, &options);
                        form.target_operasi_id = target;
                    } else {
                        if form.id.is_none() {
                            let mut operasi = form.operasi_id.clone();
                            let options = client
                                .data
                                .operations
                                .iter()
                                .map(|operation| (operation.id.to_string(), operation.name.clone()))
                                .collect::<Vec<_>>();
                            widgets::combo(
                                ui,
                                egui::Id::new("scenario_op"),
                                &mut operasi,
                                &options,
                            );
                            form.operasi_id = operasi;
                        }
                        ui.horizontal(|ui| {
                            widgets::labeled_input(ui, "Center X", &mut form.center_x);
                            widgets::labeled_input(ui, "Center Y", &mut form.center_y);
                        });
                        ui.horizontal(|ui| {
                            widgets::labeled_input(ui, "Zoom", &mut form.zoom);
                            widgets::labeled_input(ui, "Min zoom", &mut form.min_zoom);
                            widgets::labeled_input(ui, "Max zoom", &mut form.max_zoom);
                        });
                        ui.horizontal(|ui| {
                            widgets::labeled_input(ui, "Pitch", &mut form.pitch);
                            widgets::labeled_input(ui, "Bearing", &mut form.bearing);
                        });
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Map mode").size(11.0));
                                let mut mode = form.map_mode.clone();
                                widgets::combo(
                                    ui,
                                    egui::Id::new("map_mode"),
                                    &mut mode,
                                    &[
                                        ("outdoor".to_owned(), "Outdoor".to_owned()),
                                        ("indoor".to_owned(), "Indoor".to_owned()),
                                    ],
                                );
                                form.map_mode = mode;
                            });
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Base layer").size(11.0));
                                let mut layer = form.base_layer.clone();
                                widgets::combo(
                                    ui,
                                    egui::Id::new("base_layer"),
                                    &mut layer,
                                    &[
                                        ("osm".to_owned(), "OpenStreetMap".to_owned()),
                                        ("satellite".to_owned(), "Satelit".to_owned()),
                                        ("grid_dark".to_owned(), "Grid dark".to_owned()),
                                        ("grid_light".to_owned(), "Grid light".to_owned()),
                                        ("blank_dark".to_owned(), "Blank dark".to_owned()),
                                        ("blank_light".to_owned(), "Blank light".to_owned()),
                                        ("floor_plan".to_owned(), "Floor plan".to_owned()),
                                    ],
                                );
                                form.base_layer = layer;
                            });
                        });
                        ui.label(egui::RichText::new("Indoor config (JSON)").size(11.0));
                        ui.add(
                            egui::TextEdit::multiline(&mut form.indoor_config)
                                .desired_rows(3)
                                .desired_width(f32::INFINITY),
                        );
                    }
                    if let Some(error) = &form.error {
                        widgets::error_banner(ui, error);
                    }
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, "Simpan").clicked() {
                            action = ModalAction::SaveScenario;
                        }
                        if widgets::ghost_button(ui, "Batal").clicked() {
                            request_close = true;
                        }
                    });
                });
        }
        Modal::User(form) => {
            let title = if form.id.is_some() {
                "Ubah pengguna"
            } else {
                "Pengguna baru"
            };
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .min_width(380.0)
                .show(context, |ui| {
                    widgets::labeled_input(ui, "Nama", &mut form.name);
                    ui.add_enabled_ui(form.id.is_none(), |ui| {
                        widgets::labeled_input(ui, "Username", &mut form.username);
                    });
                    if form.id.is_none() {
                        widgets::password_input(ui, "Password", &mut form.password);
                    }
                    let mut role = form.role.clone();
                    widgets::combo(
                        ui,
                        egui::Id::new("user_role"),
                        &mut role,
                        &[
                            ("admin".to_owned(), "Admin".to_owned()),
                            ("admin_monitor".to_owned(), "Admin monitor".to_owned()),
                            ("admin_satker".to_owned(), "Admin satker".to_owned()),
                            ("user_satker".to_owned(), "User satker".to_owned()),
                            ("user".to_owned(), "User".to_owned()),
                        ],
                    );
                    form.role = role;
                    let mut satker = form.satker_id.clone();
                    let options = satker_options(client);
                    widgets::combo(ui, egui::Id::new("user_satker"), &mut satker, &options);
                    form.satker_id = satker;
                    widgets::labeled_input(ui, "Units (JSON)", &mut form.units);
                    if let Some(error) = &form.error {
                        widgets::error_banner(ui, error);
                    }
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, "Simpan").clicked() {
                            action = ModalAction::SaveUser;
                        }
                        if widgets::ghost_button(ui, "Batal").clicked() {
                            request_close = true;
                        }
                    });
                });
        }
        Modal::Satker(form) => {
            let title = if form.id.is_some() {
                "Ubah satker"
            } else {
                "Satker baru"
            };
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .min_width(340.0)
                .show(context, |ui| {
                    widgets::labeled_input(ui, "Kode", &mut form.code);
                    widgets::labeled_input(ui, "Nama", &mut form.name);
                    widgets::switch(ui, &mut form.is_active, "Aktif");
                    if let Some(error) = &form.error {
                        widgets::error_banner(ui, error);
                    }
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, "Simpan").clicked() {
                            action = ModalAction::SaveSatker;
                        }
                        if widgets::ghost_button(ui, "Batal").clicked() {
                            request_close = true;
                        }
                    });
                });
        }
        Modal::Confirm(confirm) => {
            egui::Window::new(&confirm.title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(context, |ui| {
                    ui.label(&confirm.message);
                    ui.add_space(SPACE_2);
                    ui.horizontal(|ui| {
                        if widgets::danger_button(ui, "Ya, lanjutkan").clicked() {
                            action = ModalAction::Confirm(confirm.action);
                        }
                        if widgets::ghost_button(ui, "Batal").clicked() {
                            request_close = true;
                        }
                    });
                });
        }
        Modal::None => {}
    }

    if !open || request_close {
        client.modal = Modal::None;
        return;
    }
    match action {
        ModalAction::SaveOperation => {
            client.modal = modal;
            client.submit_operation();
        }
        ModalAction::SaveScenario => {
            client.modal = modal;
            client.submit_scenario();
        }
        ModalAction::SaveUser => {
            client.modal = modal;
            client.submit_user();
        }
        ModalAction::SaveSatker => {
            client.modal = modal;
            client.submit_satker();
        }
        ModalAction::Confirm(action) => {
            client.modal = Modal::None;
            client.delete_confirmed(action);
        }
        ModalAction::None => {
            client.modal = modal;
        }
    }
}

fn draw_toasts(context: &egui::Context, client: &mut ClientUi) {
    if client.toasts.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::new("toasts"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
        .show(context, |ui| {
            for toast in &client.toasts {
                let fill = match toast.kind {
                    ToastKind::Info => color(DARK.card),
                    ToastKind::Success => color_alpha(DARK.success, 30),
                    ToastKind::Error => color_alpha(DARK.destructive, 30),
                };
                egui::Frame::new()
                    .fill(fill)
                    .stroke(egui::Stroke::new(1.0, client.toast_color(toast.kind)))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&toast.text)
                                .size(12.0)
                                .color(client.toast_color(toast.kind)),
                        );
                    });
                ui.add_space(4.0);
            }
        });
}

// ---------------------------------------------------------------- helpers

fn new_operation_form(client: &ClientUi) -> OperationForm {
    OperationForm {
        id: None,
        name: String::new(),
        satker_id: client
            .state
            .session
            .as_ref()
            .and_then(|session| session.satker_id)
            .map(|id| id.to_string())
            .unwrap_or_default(),
        error: None,
    }
}

fn operation_form(operation: Operation) -> OperationForm {
    OperationForm {
        id: Some(operation.id),
        name: operation.name,
        satker_id: operation
            .satker_id
            .map(|id| id.to_string())
            .unwrap_or_default(),
        error: None,
    }
}

fn new_scenario_form(client: &ClientUi) -> ScenarioForm {
    ScenarioForm {
        id: None,
        operasi_id: client
            .effective_operation()
            .map(|id| id.to_string())
            .unwrap_or_default(),
        target_operasi_id: String::new(),
        name: String::new(),
        center_x: "-8.648393".to_owned(),
        center_y: "115.225287".to_owned(),
        zoom: "15".to_owned(),
        min_zoom: "2".to_owned(),
        max_zoom: "26".to_owned(),
        pitch: "0".to_owned(),
        bearing: "0".to_owned(),
        map_mode: "outdoor".to_owned(),
        base_layer: "osm".to_owned(),
        indoor_config: "{}".to_owned(),
        copy: false,
        error: None,
    }
}

fn scenario_form(scenario: Scenario) -> ScenarioForm {
    ScenarioForm {
        id: Some(scenario.id),
        operasi_id: scenario.operasi.id.to_string(),
        target_operasi_id: String::new(),
        name: scenario.name,
        center_x: scenario.center_x.to_string(),
        center_y: scenario.center_y.to_string(),
        zoom: scenario.zoom.to_string(),
        min_zoom: scenario.min_zoom.to_string(),
        max_zoom: scenario.max_zoom.to_string(),
        pitch: scenario.pitch.to_string(),
        bearing: scenario.bearing.to_string(),
        map_mode: format!("{:?}", scenario.map_mode).to_lowercase(),
        base_layer: format!("{:?}", scenario.base_layer).to_lowercase(),
        indoor_config: scenario.indoor_config.to_string(),
        copy: false,
        error: None,
    }
}

fn copy_scenario_form(client: &ClientUi, scenario: Scenario) -> ScenarioForm {
    let target = client
        .data
        .operations
        .iter()
        .find(|operation| operation.id != scenario.operasi.id)
        .map(|operation| operation.id.to_string())
        .unwrap_or_else(|| scenario.operasi.id.to_string());
    ScenarioForm {
        copy: true,
        target_operasi_id: target,
        name: format!("Salinan {}", scenario.name),
        ..scenario_form(scenario)
    }
}

fn new_user_form(client: &ClientUi) -> UserForm {
    UserForm {
        id: None,
        name: String::new(),
        username: String::new(),
        password: String::new(),
        units: "[]".to_owned(),
        role: if client.role() == Role::AdminSatker {
            "user_satker".to_owned()
        } else {
            "user".to_owned()
        },
        satker_id: client
            .state
            .session
            .as_ref()
            .and_then(|session| session.satker_id)
            .map(|id| id.to_string())
            .unwrap_or_default(),
        profile_img: String::new(),
        error: None,
    }
}

fn user_form(user: &User) -> UserForm {
    UserForm {
        id: Some(user.id),
        name: user.name.clone(),
        username: user.username.clone(),
        password: String::new(),
        units: user.units.clone(),
        role: match user.role {
            Role::Admin => "admin",
            Role::AdminMonitor => "admin_monitor",
            Role::AdminSatker => "admin_satker",
            Role::UserSatker => "user_satker",
            Role::User => "user",
        }
        .to_owned(),
        satker_id: user.satker_id.map(|id| id.to_string()).unwrap_or_default(),
        profile_img: user.profile_img.clone(),
        error: None,
    }
}

fn satker_options(client: &ClientUi) -> Vec<(String, String)> {
    let mut options = vec![(String::new(), "(tanpa satker)".to_owned())];
    for satker in &client.data.satkers {
        options.push((satker.id.to_string(), satker.name.clone()));
    }
    options
}

fn role_label(role: Role) -> String {
    match role {
        Role::Admin => "admin",
        Role::AdminMonitor => "admin monitor",
        Role::AdminSatker => "admin satker",
        Role::UserSatker => "user satker",
        Role::User => "user",
    }
    .to_owned()
}

fn short_id(id: Uuid) -> String {
    let text = id.to_string();
    text.chars().take(8).collect()
}

struct IdentitySummary {
    role_label: String,
}

impl IdentitySummary {
    fn from_session(session: &twg_client::session::ClientSession) -> Self {
        Self {
            role_label: role_label(session.role),
        }
    }
}

// Keep imported contract types available to future screens.
const _: Option<UserIdentity> = None;
const _: Option<OnlineUser> = None;
const _: Option<Satker> = None;
const _: Option<JobKind> = None;
