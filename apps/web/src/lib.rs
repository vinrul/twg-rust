use bevy::asset::{AssetMetaCheck, AssetPlugin};
use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use twg_render::TacticalRenderPlugin;
use twg_ui::TacticalUiPlugin;

pub fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        meta_check: AssetMetaCheck::Never,
        ..default()
    }))
    .add_plugins((TacticalRenderPlugin, TacticalUiPlugin));
    app
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    build_app().run();
}
