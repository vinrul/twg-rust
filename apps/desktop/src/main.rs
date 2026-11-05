use bevy::asset::{AssetMetaCheck, AssetPlugin};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowPlugin};
use bevy::winit::WinitSettings;
use twg_map::{BaseLayer, MapCameraState};
use twg_render::{
    BaseLayerResource, DEFAULT_STRESS_ZOOM, DracoProbe, MapCameraResource, StressMode, StressRun,
    TacticalRenderPlugin,
};
use twg_ui::TacticalUiPlugin;

fn main() {
    let asset_path = std::env::var("TWG_ASSET_PATH").unwrap_or_else(|_| "../../assets".to_owned());
    let cli = CliArgs::parse();

    let mut app = App::new();
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

    if let Some(run) = cli.stress {
        app.insert_resource(run);
    }
    if cli.probe_draco {
        app.insert_resource(DracoProbe {
            exit_when_done: true,
        });
    }

    app.add_plugins((TacticalRenderPlugin, TacticalUiPlugin));

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

struct CliArgs {
    stress: Option<StressRun>,
    probe_draco: bool,
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
        });

        Self {
            stress,
            probe_draco,
            zoom: view_zoom,
            pitch,
            bearing,
            layer,
        }
    }
}
