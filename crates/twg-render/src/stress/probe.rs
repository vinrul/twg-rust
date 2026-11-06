use bevy::app::AppExit;
use bevy::asset::LoadState;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};

use super::gltf::load_gltf;

struct ProbeModel {
    file: &'static str,
    draco: bool,
}

const PROBE_MODELS: [ProbeModel; 8] = [
    ProbeModel {
        file: "polwan_airud_idle_non_draco.glb",
        draco: false,
    },
    ProbeModel {
        file: "polwan_airud_walk_non_draco.glb",
        draco: false,
    },
    ProbeModel {
        file: "polwan_airud_idle_lod2.glb",
        draco: false,
    },
    ProbeModel {
        file: "polwan_airud_walk_lod2.glb",
        draco: false,
    },
    ProbeModel {
        file: "polwan_airud_idle_draco.glb",
        draco: true,
    },
    ProbeModel {
        file: "polwan_airud_walk_draco.glb",
        draco: true,
    },
    ProbeModel {
        file: "polwan_airud_idle_lod2_draco.glb",
        draco: true,
    },
    ProbeModel {
        file: "polwan_airud_walk_lod2_draco.glb",
        draco: true,
    },
];
const PROBE_TIMEOUT_SECS: f32 = 15.0;

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
        let path = format!("models/{}", model.file);
        let handle: Handle<WorldAsset> =
            load_gltf(&asset_server, GltfAssetLabel::Scene(0), path, model.draco);
        commands.spawn((
            WorldAssetRoot(handle.clone()),
            Transform::from_xyz(index as f32 * 25.0, 0.0, 0.0).with_scale(Vec3::splat(0.02)),
            Name::new(model.file),
        ));
        entries.push((model.file.to_string(), handle));
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
