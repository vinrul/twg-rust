#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

//! Desktop-only administration app: lists online users and revokes their
//! sessions so a kicked account can log in again (the server enforces
//! single-device sessions for `admin`/`admin_satker`).

mod ui;

use std::sync::Arc;

use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow, WindowPlugin};
use bevy::winit::WinitSettings;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use twg_client::ApiClient;
use twg_client::connection::ConnectionSettings;
use twg_client::native::{FileTokenStore, UreqTransport};

use ui::OnlineAdminUi;

fn main() {
    let connection = ConnectionSettings::load();
    let base_url = std::env::var("TWG_API_URL")
        .map(|url| url.trim_end_matches('/').to_owned())
        .unwrap_or_else(|_| connection.base_url());
    let store = Arc::new(FileTokenStore::new(ui::session_path()));
    let api = ApiClient::new(
        base_url.clone(),
        ui::device_id(),
        Arc::new(UreqTransport::new()),
        store,
    );

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "TWG Online Admin".to_owned(),
                resolution: (860, 620).into(),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(WinitSettings::continuous())
        .add_plugins(EguiPlugin::default())
        .insert_resource(OnlineAdminUi::new(api, connection))
        .insert_non_send(window_icon_state())
        .add_systems(Startup, spawn_camera)
        .add_systems(Update, apply_window_icon)
        .add_systems(EguiPrimaryContextPass, ui::draw)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Live window icon for the title bar/taskbar. The embedded `.ico`/`.icns` cover
/// Explorer, Finder, and Linux launchers; this covers the window itself on
/// Windows and X11. Winit queues the call onto the main thread.
struct WindowIconState {
    icon: winit::window::Icon,
    applied: bool,
}

fn window_icon_state() -> WindowIconState {
    const SIZE: u32 = 256;
    let bytes = include_bytes!("../../../assets/icons/twg-admin-ui.rgba").to_vec();
    let icon = winit::window::Icon::from_rgba(bytes, SIZE, SIZE)
        .expect("embedded admin window icon is valid RGBA");
    WindowIconState {
        icon,
        applied: false,
    }
}

fn apply_window_icon(
    windows: Query<Entity, With<PrimaryWindow>>,
    mut state: NonSendMut<WindowIconState>,
) {
    if state.applied {
        return;
    }
    let Ok(entity) = windows.single() else {
        return;
    };
    bevy::winit::WINIT_WINDOWS.with_borrow(|winit_windows| {
        let Some(window) = winit_windows.get_window(entity) else {
            return;
        };
        window.set_window_icon(Some(state.icon.clone()));
        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowExtWindows;
            window.set_taskbar_icon(Some(state.icon.clone()));
        }
        state.applied = true;
    });
}
