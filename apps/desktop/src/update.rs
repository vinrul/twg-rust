//! Startup update gate for `twg-desktop`.
//!
//! The updater checks the signed manifest from the public server before the
//! login/boot UI is allowed to draw. A newer build is mandatory: the app
//! downloads, verifies, installs, and restarts. If the check itself fails, the
//! operator may retry or continue offline.

use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use twg_client::design::{DARK, SPACE_2, SPACE_3};
use twg_ui::ClientUiGate;
use twg_ui::theme;
use twg_ui::widgets;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Checking,
    Downloading,
    Verifying,
    Installing,
    CheckFailed,
    Failed,
    Done,
}

enum UpdateEvent {
    Available {
        version: String,
        build: u64,
        notes: String,
        size: u64,
    },
    Progress {
        downloaded: u64,
        total: Option<u64>,
    },
    Verifying,
    Installing,
    InstallReady(std::path::PathBuf),
    CheckFailed(String),
    InstallFailed(String),
    Done,
}

#[derive(Resource)]
struct UpdateUi {
    phase: Phase,
    version: String,
    build: u64,
    notes: String,
    downloaded: u64,
    total: Option<u64>,
    error: Option<String>,
    receiver: Option<Mutex<Receiver<UpdateEvent>>>,
}

impl UpdateUi {
    fn new() -> Self {
        Self {
            phase: Phase::Checking,
            version: String::new(),
            build: 0,
            notes: String::new(),
            downloaded: 0,
            total: None,
            error: None,
            receiver: None,
        }
    }

    fn start(&mut self) {
        self.phase = Phase::Checking;
        self.version.clear();
        self.build = 0;
        self.notes.clear();
        self.downloaded = 0;
        self.total = None;
        self.error = None;
        let (sender, receiver) = channel();
        self.receiver = Some(Mutex::new(receiver));
        let manifest_url = manifest_url();
        let current_build = current_build();
        bevy::log::info!("update: checking {manifest_url} (current build {current_build})");
        std::thread::spawn(move || run_update(manifest_url, current_build, sender));
    }

    fn drain_events(&self) -> Vec<UpdateEvent> {
        let Some(receiver) = &self.receiver else {
            return Vec::new();
        };
        let receiver = receiver.lock().expect("update receiver poisoned");
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        events
    }

    fn fraction(&self) -> f32 {
        match self.total {
            Some(total) if total > 0 => (self.downloaded as f32 / total as f32).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

pub struct UpdatePlugin;

impl Plugin for UpdatePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClientUiGate { open: false });
        app.insert_resource(UpdateUi::new());
        app.add_systems(Startup, start_check);
        app.add_systems(Update, poll_events);
        app.add_systems(EguiPrimaryContextPass, draw);
    }
}

fn start_check(mut update: ResMut<UpdateUi>) {
    update.start();
}

fn poll_events(
    mut update: ResMut<UpdateUi>,
    mut gate: ResMut<ClientUiGate>,
    mut exit: MessageWriter<AppExit>,
) {
    if gate.open || update.receiver.is_none() {
        return;
    }
    for event in update.drain_events() {
        match event {
            UpdateEvent::Available {
                version,
                build,
                notes,
                size,
            } => {
                update.phase = Phase::Downloading;
                update.version = version;
                update.build = build;
                update.notes = notes;
                update.total = (size > 0).then_some(size);
            }
            UpdateEvent::Progress { downloaded, total } => {
                update.phase = Phase::Downloading;
                update.downloaded = downloaded;
                update.total = total;
            }
            UpdateEvent::Verifying => update.phase = Phase::Verifying,
            UpdateEvent::Installing => update.phase = Phase::Installing,
            UpdateEvent::InstallReady(path) => {
                update.receiver = None;
                bevy::log::info!("update: installed, relaunching from {}", path.display());
                match twg_update::launch(&path) {
                    Ok(()) => {
                        update.phase = Phase::Done;
                        exit.write(AppExit::Success);
                    }
                    Err(error) => {
                        update.phase = Phase::Failed;
                        update.error = Some(error.to_string());
                    }
                }
            }
            UpdateEvent::CheckFailed(error) => {
                update.receiver = None;
                bevy::log::warn!("update: check failed: {error}");
                update.phase = Phase::CheckFailed;
                update.error = Some(error);
            }
            UpdateEvent::InstallFailed(error) => {
                update.receiver = None;
                bevy::log::error!("update: install failed: {error}");
                update.phase = Phase::Failed;
                update.error = Some(error);
            }
            UpdateEvent::Done => {
                update.receiver = None;
                update.phase = Phase::Done;
                gate.open = true;
                bevy::log::info!("update: up to date, opening client shell");
            }
        }
    }
}

fn draw(
    mut contexts: EguiContexts,
    mut update: ResMut<UpdateUi>,
    mut gate: ResMut<ClientUiGate>,
    mut exit: MessageWriter<AppExit>,
    mut themed: Local<bool>,
) -> Result {
    if gate.open {
        return Ok(());
    }
    let context = contexts.ctx_mut()?;
    if !*themed {
        theme::apply(context);
        *themed = true;
    }
    let mut root = egui::Ui::new(
        context.clone(),
        egui::Id::new("twg_update_root"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(context.viewport_rect()),
    );
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::color(DARK.background)))
        .show(&mut root, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(90.0);
                ui.label(
                    egui::RichText::new("TACTICAL GAME 3D")
                        .size(11.0)
                        .color(theme::color(DARK.muted_foreground)),
                );
                ui.label(
                    egui::RichText::new("Pembaruan aplikasi")
                        .size(26.0)
                        .strong(),
                );
                ui.add_space(SPACE_3);
                widgets::card(ui, |ui| {
                    ui.set_width(520.0);
                    match update.phase {
                        Phase::Checking => {
                            widgets::loading(ui, "Memeriksa pembaruan...");
                        }
                        Phase::Downloading | Phase::Verifying | Phase::Installing => {
                            ui.label(theme::title(&format!(
                                "Versi {} (build {}) → {} (build {})",
                                CURRENT_VERSION,
                                current_build(),
                                update.version,
                                update.build
                            )));
                            if !update.notes.is_empty() {
                                ui.add_space(SPACE_2);
                                ui.label(theme::muted(&update.notes));
                            }
                            ui.add_space(SPACE_3);
                            match update.phase {
                                Phase::Downloading => {
                                    ui.add(
                                        egui::ProgressBar::new(update.fraction())
                                            .fill(theme::color(DARK.primary))
                                            .show_percentage(),
                                    );
                                    let total = update
                                        .total
                                        .map(format_bytes)
                                        .unwrap_or_else(|| "?".to_owned());
                                    ui.label(theme::muted(&format!(
                                        "{} / {}",
                                        format_bytes(update.downloaded),
                                        total
                                    )));
                                }
                                Phase::Verifying => {
                                    widgets::loading(ui, "Memverifikasi pembaruan...");
                                }
                                _ => {
                                    widgets::loading(ui, "Memasang pembaruan...");
                                }
                            }
                            ui.add_space(SPACE_2);
                            ui.label(theme::muted(
                                "Pembaruan wajib. Jangan tutup aplikasi sampai proses selesai.",
                            ));
                        }
                        Phase::CheckFailed => {
                            ui.label(theme::title("Tidak dapat memeriksa pembaruan"));
                            ui.label(theme::muted(
                                "Server update tidak terjangkau. Anda tetap bisa melanjutkan \
                                 memakai aplikasi, tetapi pembaruan belum terpasang.",
                            ));
                            if let Some(error) = &update.error {
                                ui.add_space(SPACE_2);
                                widgets::error_banner(ui, error);
                            }
                            ui.add_space(SPACE_3);
                            ui.horizontal(|ui| {
                                if widgets::primary_button(ui, "Coba lagi").clicked() {
                                    update.start();
                                }
                                if widgets::secondary_button(ui, "Tetap lanjut").clicked() {
                                    gate.open = true;
                                }
                                if widgets::ghost_button(ui, "Keluar").clicked() {
                                    exit.write(AppExit::Success);
                                }
                            });
                        }
                        Phase::Failed => {
                            ui.label(theme::title("Pembaruan gagal"));
                            if let Some(error) = &update.error {
                                ui.add_space(SPACE_2);
                                widgets::error_banner(ui, error);
                            }
                            ui.add_space(SPACE_3);
                            ui.horizontal(|ui| {
                                if widgets::primary_button(ui, "Coba lagi").clicked() {
                                    update.start();
                                }
                                if widgets::ghost_button(ui, "Keluar").clicked() {
                                    exit.write(AppExit::Success);
                                }
                            });
                        }
                        Phase::Done => {}
                    }
                });
            });
        });
    Ok(())
}

fn run_update(manifest_url: String, current_build: u64, sender: Sender<UpdateEvent>) {
    twg_update::cleanup_old_versions();
    match twg_update::check(&manifest_url, current_build) {
        Ok(twg_update::CheckOutcome::UpToDate) => {
            bevy::log::info!("update: server reports no newer build");
            let _ = sender.send(UpdateEvent::Done);
        }
        Ok(twg_update::CheckOutcome::Available(plan)) => {
            bevy::log::info!(
                "update: available version {} build {}",
                plan.version(),
                plan.build()
            );
            let _ = sender.send(UpdateEvent::Available {
                version: plan.version().to_owned(),
                build: plan.build(),
                notes: plan.notes().to_owned(),
                size: plan.size(),
            });
            let progress_sender = sender.clone();
            match twg_update::download(&plan, move |downloaded, total| {
                let _ = progress_sender.send(UpdateEvent::Progress { downloaded, total });
            }) {
                Ok(path) => {
                    let _ = sender.send(UpdateEvent::Verifying);
                    let _ = sender.send(UpdateEvent::Installing);
                    match twg_update::install(&path, current_build) {
                        Ok(launch) => {
                            let _ = sender.send(UpdateEvent::InstallReady(launch));
                        }
                        Err(error) => {
                            let _ = sender.send(UpdateEvent::InstallFailed(error.to_string()));
                        }
                    }
                }
                Err(error) => {
                    let _ = sender.send(UpdateEvent::InstallFailed(error.to_string()));
                }
            }
        }
        Err(error) => {
            bevy::log::warn!("update: check error: {error}");
            let _ = sender.send(UpdateEvent::CheckFailed(error.to_string()));
        }
    }
}

fn manifest_url() -> String {
    std::env::var("TWG_UPDATE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| twg_update::DEFAULT_MANIFEST_URL.to_owned())
}

fn current_build() -> u64 {
    env!("TWG_BUILD_NUMBER").parse().unwrap_or(0)
}

fn format_bytes(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    }
}
