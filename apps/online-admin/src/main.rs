//! Desktop-only administration app: lists online users and revokes their
//! sessions so a kicked account can log in again (the server enforces
//! single-device sessions for `admin`/`admin_satker`).

mod ui;

use std::sync::Arc;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowPlugin};
use bevy::winit::WinitSettings;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use twg_client::ApiClient;
use twg_client::native::{FileTokenStore, UreqTransport};

use ui::OnlineAdminUi;

fn main() {
    let base_url = std::env::var("TWG_API_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8010".to_owned())
        .trim_end_matches('/')
        .to_owned();
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
        .insert_resource(OnlineAdminUi::new(api, base_url))
        .add_systems(Startup, spawn_camera)
        .add_systems(EguiPrimaryContextPass, ui::draw)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
