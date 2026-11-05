use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use twg_render::{CameraFlight, CameraFlightState, MapCameraResource, MapCameraState};

pub fn draw_compass(
    mut contexts: EguiContexts,
    mut camera: ResMut<MapCameraResource>,
    mut flight: ResMut<CameraFlightState>,
) -> Result {
    let context = contexts.ctx_mut()?;
    egui::Window::new("Compass")
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
        .resizable(false)
        .show(context, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(104.0, 104.0), egui::Sense::click());
            let painter = ui.painter_at(rect);
            let center = rect.center();
            let radius = 44.0;
            let bearing = camera.0.bearing_degrees.to_radians();

            painter.circle_stroke(
                center,
                radius,
                egui::Stroke::new(1.5, egui::Color32::from_gray(140)),
            );
            painter.circle_filled(center, 3.0, egui::Color32::from_gray(200));

            for (label, dx, dy) in [
                ("N", 0.0, -radius - 9.0),
                ("E", radius + 10.0, 0.0),
                ("S", 0.0, radius + 9.0),
                ("W", -radius - 10.0, 0.0),
            ] {
                painter.text(
                    center + egui::vec2(dx, dy),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(12.0),
                    if label == "N" {
                        egui::Color32::from_rgb(255, 190, 60)
                    } else {
                        egui::Color32::from_gray(180)
                    },
                );
            }

            // The needle points to north in the rotated map view.
            let direction = egui::vec2(bearing.sin(), -bearing.cos());
            let side = egui::vec2(-direction.y, direction.x);
            let tip = center + direction * (radius * 0.82);
            let back = center - direction * (radius * 0.25);
            painter.add(egui::Shape::convex_polygon(
                vec![tip, center + side * 8.0, back, center - side * 8.0],
                egui::Color32::from_rgb(255, 170, 40),
                egui::Stroke::NONE,
            ));
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + side * 8.0,
                    center - direction * (radius * 0.25),
                    center - side * 8.0,
                ],
                egui::Color32::from_gray(120),
                egui::Stroke::NONE,
            ));

            if response.clicked() {
                let mut target = camera.0;
                target.bearing_degrees = 0.0;
                flight.0 = Some(CameraFlight::new(camera.0, target, 0.5));
            }
            response.on_hover_text("Klik kompas untuk reset ke utara");

            ui.horizontal(|ui| {
                if ui.button("−").clicked() {
                    camera.0.zoom_by(-0.5);
                }
                if ui.button("N").clicked() {
                    let mut target = camera.0;
                    target.bearing_degrees = 0.0;
                    flight.0 = Some(CameraFlight::new(camera.0, target, 0.5));
                }
                if ui.button("+").clicked() {
                    camera.0.zoom_by(0.5);
                }
                if ui.button("Reset").clicked() {
                    flight.0 = Some(CameraFlight::new(camera.0, MapCameraState::default(), 0.8));
                }
            });
            ui.small(format!(
                "bearing {:.0}°  zoom {:.2}",
                camera.0.bearing_degrees, camera.0.zoom
            ));
        });

    Ok(())
}
