use bevy::app::AppExit;
use bevy::asset::LoadState;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};

const PROBE_MODELS: [&str; 4] = [
    "polwan_airud_idle_non_draco.glb",
    "polwan_airud_walk_non_draco.glb",
    "polwan_airud_idle_draco.glb",
    "polwan_airud_walk_draco.glb",
];
const PROBE_TIMEOUT_SECS: f32 = 8.0;

#[derive(Resource, Clone, Copy, Debug)]
pub struct DracoProbe {
    pub exit_when_done: bool,
}

#[derive(Resource)]
pub struct DracoProbeState {
    entries: Vec<(String, Handle<WorldAsset>)>,
    elapsed: f32,
    reported: bool,
    exit_when_done: bool,
}

pub fn setup_probe(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    probe: Option<Res<DracoProbe>>,
) {
    let Some(probe) = probe else {
        return;
    };

    let mut entries = Vec::with_capacity(PROBE_MODELS.len());
    for (index, model) in PROBE_MODELS.iter().enumerate() {
        let path = format!("models/{model}");
        let handle: Handle<WorldAsset> =
            asset_server.load(GltfAssetLabel::Scene(0).from_asset(path));
        commands.spawn((
            WorldAssetRoot(handle.clone()),
            Transform::from_xyz(index as f32 * 25.0, 0.0, 0.0)
                .with_scale(Vec3::splat(0.02)),
            Name::new(*model),
        ));
        entries.push(((*model).to_string(), handle));
    }

    commands.insert_resource(DracoProbeState {
        entries,
        elapsed: 0.0,
        reported: false,
        exit_when_done: probe.exit_when_done,
    });
}

pub fn report_probe(
    time: Res<Time>,
    mut probe: Option<ResMut<DracoProbeState>>,
    asset_server: Res<AssetServer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(probe) = probe.as_mut() else {
        return;
    };
    if probe.reported {
        return;
    }
    probe.elapsed += time.delta_secs();
    if probe.elapsed < PROBE_TIMEOUT_SECS {
        return;
    }
    probe.reported = true;

    for (model, handle) in &probe.entries {
        let status = match asset_server.get_load_state(handle.id()) {
            Some(LoadState::Loaded) => "PASS".to_string(),
            Some(LoadState::Failed(error)) => format!("FAIL: {error}"),
            Some(LoadState::Loading) => "FAIL: still loading after timeout".to_string(),
            Some(LoadState::NotLoaded) => "FAIL: not loaded".to_string(),
            None => "FAIL: load state unavailable".to_string(),
        };
        println!("DRACO_PROBE {model}: {status}");
    }

    if probe.exit_when_done {
        exit.write(AppExit::Success);
    }
}
