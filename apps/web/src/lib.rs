#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

use bevy::asset::{AssetMetaCheck, AssetPlugin};
use bevy::prelude::*;
use bevy::window::WindowPlugin;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use twg_client::TokenStore;
use twg_render::TacticalRenderPlugin;
#[cfg(target_arch = "wasm32")]
use twg_ui::ClientBackend;
use twg_ui::TacticalUiPlugin;

pub fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..default()
            }),
    );

    #[cfg(target_arch = "wasm32")]
    let ui = {
        use twg_client::ApiClient;
        use twg_client::web::{
            BrowserWsTransport, FetchTransport, LocalStorageTokenStore, api_base_url,
            persisted_device_id, stress_requested,
        };
        if stress_requested() {
            TacticalUiPlugin::map_demo()
        } else {
            let base_url = api_base_url();
            let api = ApiClient::new(
                base_url.clone(),
                persisted_device_id(),
                Arc::new(FetchTransport),
                Arc::new(LocalStorageTokenStore::new()) as Arc<dyn TokenStore>,
            );
            TacticalUiPlugin::new(ClientBackend {
                api,
                ws_transport: Arc::new(BrowserWsTransport),
                base_url,
            })
        }
    };

    #[cfg(not(target_arch = "wasm32"))]
    let ui = TacticalUiPlugin::map_demo();

    app.add_plugins((TacticalRenderPlugin, ui));
    app
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    build_app().run();
}
