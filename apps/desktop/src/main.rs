use bevy::asset::io::file::FileAssetReader;
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::asset::{AssetApp, AssetMetaCheck, AssetPlugin};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowPlugin};
use bevy::winit::WinitSettings;
use std::sync::Arc;
use twg_client::ApiClient;
use twg_client::TokenStore;
use twg_client::native::{
    FileTokenStore, TungsteniteTransport, UreqTransport, persisted_device_id,
};
use twg_map::{BaseLayer, MapCameraState};
use twg_render::{
    BaseLayerResource, CACHE_ASSET_SOURCE, DEFAULT_STRESS_ZOOM, DracoProbe, MapCameraResource,
    MarkerModelSet, MemoryProbe, StressMode, StressRun, TacticalRenderPlugin, TileProxyConfig,
};
use twg_ui::{ClientBackend, TacticalUiPlugin};

fn main() {
    let asset_path = std::env::var("TWG_ASSET_PATH").unwrap_or_else(|_| "../../assets".to_owned());
    let cli = CliArgs::parse();

    let mut app = App::new();
    // Expose the offline asset cache to Bevy as `twg-cache://...`. The source
    // must exist before `DefaultPlugins` builds the asset server.
    let cache_root = twg_client::asset_cache::default_root();
    app.register_asset_source(
        AssetSourceId::from(CACHE_ASSET_SOURCE),
        AssetSourceBuilder::new(move || Box::new(FileAssetReader::new(cache_root.clone()))),
    );
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Tactical Game 3D".to_owned(),
                    present_mode: PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: asset_path,
                meta_check: AssetMetaCheck::Never,
                ..default()
            }),
    )
    .insert_resource(WinitSettings::continuous());

    if let Ok(proxy) = std::env::var("TWG_TILE_PROXY") {
        app.insert_resource(TileProxyConfig(Some(proxy)));
    }

    if let Some(run) = cli.stress {
        app.insert_resource(run);
    }
    if cli.probe_draco {
        app.insert_resource(DracoProbe {
            exit_when_done: true,
        });
    }
    if cli.memory_probe {
        app.insert_resource(MemoryProbe::new(true));
    }
    app.insert_resource(cli.models);

    let demo = cli.stress.is_some()
        || cli.probe_draco
        || cli.memory_probe
        || std::env::var_os("TWG_DEMO").is_some();
    let ui = if demo {
        TacticalUiPlugin::map_demo()
    } else {
        TacticalUiPlugin::new(client_backend())
    };
    app.add_plugins((TacticalRenderPlugin, ui));

    if cli.zoom.is_some() || cli.pitch.is_some() || cli.bearing.is_some() {
        let mut camera = MapCameraState::default();
        if let Some(zoom) = cli.zoom {
            camera.set_zoom(zoom);
        }
        if let Some(pitch) = cli.pitch {
            camera.pitch_degrees = pitch.clamp(0.0, 85.0);
        }
        if let Some(bearing) = cli.bearing {
            camera.bearing_degrees = bearing.rem_euclid(360.0);
        }
        app.insert_resource(MapCameraResource(camera));
    }
    if let Some(layer) = cli.layer {
        app.insert_resource(BaseLayerResource(layer));
    }

    app.run();
}

fn client_backend() -> ClientBackend {
    let base_url = std::env::var("TWG_API_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8010".to_owned())
        .trim_end_matches('/')
        .to_owned();
    let store = Arc::new(FileTokenStore::default()) as Arc<dyn TokenStore>;
    let api = ApiClient::new(
        base_url.clone(),
        persisted_device_id(),
        Arc::new(UreqTransport::new()),
        store,
    );
    ClientBackend {
        api,
        ws_transport: Arc::new(TungsteniteTransport),
        base_url,
    }
}

struct CliArgs {
    stress: Option<StressRun>,
    models: MarkerModelSet,
    probe_draco: bool,
    memory_probe: bool,
    zoom: Option<f32>,
    pitch: Option<f32>,
    bearing: Option<f32>,
    layer: Option<BaseLayer>,
}

impl CliArgs {
    fn parse() -> Self {
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        let mut count = 1_000usize;
        let mut mode = StressMode::Mixed;
        let mut benchmark = false;
        let mut exit = false;
        let mut probe_draco = false;
        let mut memory_probe = false;
        let mut lod = cfg!(target_arch = "wasm32");
        let mut draco = false;
        let mut zoom = DEFAULT_STRESS_ZOOM;
        let mut warmup_secs = 3.0f32;
        let mut sample_secs = 8.0f32;
        let mut requested = false;
        let mut view_zoom = None;
        let mut pitch = None;
        let mut bearing = None;
        let mut layer = None;
        let mut index = 0;

        while index < args.len() {
            let value = args.get(index + 1);
            match args[index].as_str() {
                "--stress" | "--stress-mode" => {
                    if let Some(parsed) = value.and_then(|value| StressMode::parse(value)) {
                        mode = parsed;
                        requested = true;
                    }
                    index += 1;
                }
                "--stress-count" | "--count" => {
                    if let Some(parsed) = value.and_then(|value| value.parse().ok()) {
                        count = parsed;
                        requested = true;
                    }
                    index += 1;
                }
                "--zoom" => {
                    if let Some(parsed) = value.and_then(|value| value.parse().ok()) {
                        zoom = parsed;
                        view_zoom = Some(parsed);
                    }
                    index += 1;
                }
                "--pitch" => {
                    if let Some(parsed) = value.and_then(|value| value.parse().ok()) {
                        pitch = Some(parsed);
                    }
                    index += 1;
                }
                "--bearing" => {
                    if let Some(parsed) = value.and_then(|value| value.parse().ok()) {
                        bearing = Some(parsed);
                    }
                    index += 1;
                }
                "--layer" => {
                    layer = value.and_then(|value| match value.as_str() {
                        "satellite" | "esri" => Some(BaseLayer::ArcGisImagery),
                        "osm" | "street" => Some(BaseLayer::OpenStreetMap),
                        _ => None,
                    });
                    index += 1;
                }
                "--warmup" => {
                    if let Some(parsed) = value.and_then(|value| value.parse().ok()) {
                        warmup_secs = parsed;
                    }
                    index += 1;
                }
                "--sample" => {
                    if let Some(parsed) = value.and_then(|value| value.parse().ok()) {
                        sample_secs = parsed;
                    }
                    index += 1;
                }
                "--benchmark" => {
                    benchmark = true;
                    requested = true;
                }
                "--exit" => exit = true,
                "--probe-draco" => probe_draco = true,
                "--memory-probe" => memory_probe = true,
                "--stress-lod" | "--lod" => lod = true,
                "--stress-full" | "--full" => lod = false,
                "--draco" => draco = true,
                "--no-draco" => draco = false,
                _ => {}
            }
            index += 1;
        }

        let stress = requested.then_some(StressRun {
            count,
            mode,
            benchmark,
            warmup_secs,
            sample_secs,
            exit_when_done: exit || benchmark,
            zoom,
            pitch_degrees: pitch,
            bearing_degrees: bearing,
        });
        let stress = stress.or_else(|| {
            memory_probe.then_some(StressRun {
                count: 10_000,
                mode: StressMode::Static,
                benchmark: false,
                warmup_secs: 0.0,
                sample_secs: 0.0,
                exit_when_done: false,
                zoom,
                pitch_degrees: pitch,
                bearing_degrees: bearing,
            })
        });

        Self {
            stress,
            models: MarkerModelSet::for_variant(lod, draco),
            probe_draco,
            memory_probe,
            zoom: view_zoom,
            pitch,
            bearing,
            layer,
        }
    }
}
