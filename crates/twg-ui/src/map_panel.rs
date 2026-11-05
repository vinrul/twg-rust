use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use twg_map::{BaseLayer, MAX_ZOOM};
use twg_render::{BaseLayerResource, MapCameraResource, MapCursorResource};

pub fn draw_map_panel(
    mut contexts: EguiContexts,
    mut layer: ResMut<BaseLayerResource>,
    camera: Res<MapCameraResource>,
    cursor: Res<MapCursorResource>,
    diagnostics: Res<DiagnosticsStore>,
) -> Result {
    let context = contexts.ctx_mut()?;
    let camera = camera.0;
    let cursor = cursor.0;
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or_default();

    egui::Window::new("Tactical Map · Gate G1")
        .default_pos(egui::pos2(16.0, 16.0))
        .resizable(false)
        .show(context, |ui| {
            ui.heading("Outdoor raster + overzoom");
            ui.separator();
            ui.monospace(format!(
                "center  {:.6}, {:.6}",
                camera.center.latitude, camera.center.longitude
            ));
            ui.monospace(format!(
                "zoom    {:.2} / max {:.0}   tile z{}",
                camera.zoom,
                MAX_ZOOM,
                camera.tile_zoom()
            ));
            ui.monospace(format!(
                "scale   {:.2} m/px   tile {:.1} m",
                camera.meters_per_pixel(),
                camera.tile_world_size()
            ));
            ui.monospace(format!(
                "pitch   {:.1}°   bearing {:.1}°",
                camera.pitch_degrees, camera.bearing_degrees
            ));
            ui.monospace(format!("fps     {fps:.1}"));
            match cursor.position {
                Some(point) => ui.monospace(format!(
                    "cursor  {:.6}, {:.6}",
                    point.latitude, point.longitude
                )),
                None => ui.monospace("cursor  —"),
            };
            ui.separator();
            ui.label("Base layer");
            ui.horizontal(|ui| {
                for candidate in BaseLayer::ALL {
                    if ui
                        .selectable_label(layer.0 == candidate, candidate.label())
                        .clicked()
                    {
                        layer.0 = candidate;
                    }
                }
            });
            ui.small(layer.0.attribution());
            ui.separator();
            ui.label("Drag: pan  ·  Right-drag: orbit  ·  Wheel: zoom");
            ui.label("WASD/arrows: pan  ·  Q/E: rotate  ·  R/F: pitch  ·  H: reset");
            ui.label("L: switch base layer  ·  +/-: zoom step");
        });

    Ok(())
}
