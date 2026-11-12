//! Interactive map preview for the scenario form.
//!
//! Outdoor layers fetch raster tiles from the same providers as the 3D map;
//! indoor grid/blank layers are drawn procedurally. Pan, pitch, zoom and
//! bearing all write back into the form, so the numeric fields and the preview
//! stay in sync.

use bevy_egui::egui;
#[cfg(not(target_arch = "wasm32"))]
use twg_map::{BaseLayer, TileId, TileSource};
use twg_map::{EQUATOR_CIRCUMFERENCE_METERS, GeoPoint};

use crate::state::ScenarioForm;

const TILE_PX: f32 = 256.0;
const MAX_TILE_ZOOM: u8 = 19;
const MIN_ZOOM: f64 = 2.0;
const MAX_ZOOM: f64 = 26.0;
const MIN_PITCH: f64 = 0.0;
const MAX_PITCH: f64 = 85.0;

/// Tile identity used by the cache. The layer slug is part of the key so
/// switching between OSM and satellite does not show stale tiles.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TileKey {
    layer: &'static str,
    z: u8,
    x: u32,
    y: u32,
}

#[cfg(not(target_arch = "wasm32"))]
type TileResults = std::sync::Arc<std::sync::Mutex<Vec<(TileKey, Option<egui::ColorImage>)>>>;

/// Map preview state kept in `ClientUi` so the tile cache survives closing and
/// reopening the scenario dialog.
#[derive(Default)]
pub struct ScenarioPreview {
    #[cfg(not(target_arch = "wasm32"))]
    tiles: std::collections::HashMap<TileKey, egui::TextureHandle>,
    #[cfg(not(target_arch = "wasm32"))]
    loading: std::collections::HashSet<TileKey>,
    #[cfg(not(target_arch = "wasm32"))]
    failed: std::collections::HashSet<TileKey>,
    #[cfg(not(target_arch = "wasm32"))]
    results: TileResults,
    last_layer: String,
}

impl ScenarioPreview {
    /// Drops cached tiles when the tile proxy changes.
    pub fn clear_tile_cache(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.tiles.clear();
            self.loading.clear();
            self.failed.clear();
        }
    }

    /// Draws the preview and applies drag/scroll input to `form`.
    pub fn show(&mut self, ui: &mut egui::Ui, form: &mut ScenarioForm, _tile_proxy: Option<&str>) {
        if self.last_layer != form.base_layer {
            self.last_layer.clone_from(&form.base_layer);
            #[cfg(not(target_arch = "wasm32"))]
            self.failed.clear();
        }
        let outdoor = form.map_mode != "indoor";
        let width = ui.available_width().max(220.0);
        let height = (width * 0.55).clamp(180.0, 300.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());
        let dark = form.base_layer.ends_with("dark");
        let background = if outdoor {
            egui::Color32::from_rgb(18, 22, 26)
        } else if dark {
            egui::Color32::from_rgb(24, 26, 30)
        } else {
            egui::Color32::from_rgb(240, 240, 234)
        };
        ui.painter().rect_filled(rect, 6.0, background);

        handle_input(ui, &response, form);

        if outdoor {
            #[cfg(not(target_arch = "wasm32"))]
            self.draw_tiles(ui, rect, form, _tile_proxy);
            #[cfg(target_arch = "wasm32")]
            draw_outdoor_placeholder(ui, rect);
        } else if form.base_layer.starts_with("grid_") {
            draw_grid(ui, rect, form, dark);
        }

        draw_overlay(ui, rect, form, outdoor);
        if response.hovered() {
            ui.ctx().set_cursor_icon(if response.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn draw_tiles(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        form: &ScenarioForm,
        tile_proxy: Option<&str>,
    ) {
        self.poll(ui.ctx());
        let Some(layer) = base_layer(form) else {
            return;
        };
        let z = tile_zoom(form.zoom);
        let scale = preview_scale(form.zoom) as f32;
        let center = GeoPoint::new(form.center_x, form.center_y);
        let Ok((fx, fy)) = center.tile_fraction(z) else {
            return;
        };
        let center_px = egui::vec2(
            (fx * f64::from(TILE_PX)) as f32,
            (fy * f64::from(TILE_PX)) as f32,
        );
        let cos_pitch = form.pitch.to_radians().cos().max(0.12) as f32;
        let bearing = form.bearing.to_radians() as f32;
        let half_w = rect.width() * 0.5 / scale;
        let half_h = rect.height() * 0.5 / (scale * cos_pitch);
        let radius = ((half_w.hypot(half_h) / TILE_PX).ceil() as i32 + 1).clamp(1, 6);
        let tx = fx.floor() as i64;
        let ty = fy.floor() as i64;
        let world = 1i64 << z;
        let painter = ui.painter().with_clip_rect(rect);
        let mut requests = 0u32;
        let mut any = false;
        for dy in -radius..=radius {
            let draw_y = ty + i64::from(dy);
            if draw_y < 0 || draw_y >= world {
                continue;
            }
            for dx in -radius..=radius {
                let draw_x = tx + i64::from(dx);
                let wrapped_x = draw_x.rem_euclid(world) as u32;
                let key = TileKey {
                    layer: layer.slug(),
                    z,
                    x: wrapped_x,
                    y: draw_y as u32,
                };
                if !self.tiles.contains_key(&key) {
                    self.request_tile(key, layer, tile_proxy, &mut requests);
                }
                let Some(texture) = self.tiles.get(&key) else {
                    continue;
                };
                any = true;
                let map_origin = egui::vec2(draw_x as f32 * TILE_PX, draw_y as f32 * TILE_PX);
                let mesh = tile_mesh(
                    rect,
                    center_px,
                    map_origin,
                    scale,
                    bearing,
                    cos_pitch,
                    texture.id(),
                );
                painter.add(egui::Shape::mesh(mesh));
            }
        }
        if !any {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Memuat peta...",
                egui::FontId::proportional(12.0),
                egui::Color32::from_white_alpha(190),
            );
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn request_tile(
        &mut self,
        key: TileKey,
        layer: BaseLayer,
        tile_proxy: Option<&str>,
        requests: &mut u32,
    ) {
        if *requests >= 4 || self.loading.contains(&key) || self.failed.contains(&key) {
            return;
        }
        self.loading.insert(key);
        *requests += 1;
        let url = layer.tile_source_with_proxy(tile_proxy).url(TileId {
            zoom: key.z,
            x: key.x,
            y: key.y,
        });
        let results = self.results.clone();
        std::thread::spawn(move || {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .http_status_as_error(false)
                .user_agent("twg-desktop/0.1")
                .timeout_global(Some(std::time::Duration::from_secs(8)))
                .build()
                .into();
            let image = agent
                .get(&url)
                .call()
                .ok()
                .filter(|response| (200..300).contains(&response.status().as_u16()))
                .and_then(|mut response| response.body_mut().read_to_vec().ok())
                .and_then(|bytes| image::load_from_memory(&bytes).ok())
                .map(|image| {
                    let image = image.to_rgba8();
                    egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    )
                });
            if let Ok(mut guard) = results.lock() {
                guard.push((key, image));
            }
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll(&mut self, ctx: &egui::Context) {
        let results = self
            .results
            .lock()
            .map(|mut guard| std::mem::take(&mut *guard))
            .unwrap_or_default();
        for (key, image) in results {
            self.loading.remove(&key);
            match image {
                Some(image) => {
                    let name = format!("scenario-tile:{}:{}/{}/{}", key.layer, key.z, key.x, key.y);
                    let texture = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
                    self.tiles.insert(key, texture);
                }
                None => {
                    self.failed.insert(key);
                }
            }
        }
        if self.tiles.len() > 160 {
            self.tiles.clear();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn base_layer(form: &ScenarioForm) -> Option<BaseLayer> {
    match form.base_layer.as_str() {
        "osm" => Some(BaseLayer::OpenStreetMap),
        "satellite" => Some(BaseLayer::ArcGisImagery),
        _ => None,
    }
}

fn tile_zoom(zoom: f64) -> u8 {
    zoom.floor().clamp(2.0, f64::from(MAX_TILE_ZOOM)) as u8
}

fn preview_scale(zoom: f64) -> f64 {
    2f64.powf(zoom - f64::from(tile_zoom(zoom)))
}

fn handle_input(ui: &egui::Ui, response: &egui::Response, form: &mut ScenarioForm) {
    let shift = ui.input(|input| input.modifiers.shift);
    if response.dragged_by(egui::PointerButton::Primary) {
        let delta = response.drag_delta();
        if shift {
            orbit(form, delta);
        } else {
            pan(form, delta);
        }
    }
    if response.dragged_by(egui::PointerButton::Secondary) {
        orbit(form, response.drag_delta());
    }
    if response.hovered() {
        let scroll = ui.input(|input| input.smooth_scroll_delta.y);
        if scroll != 0.0 {
            form.zoom = (form.zoom + f64::from(scroll) * 0.005).clamp(MIN_ZOOM, MAX_ZOOM);
            // Keep the wheel on the map instead of scrolling the form behind it.
            ui.ctx()
                .input_mut(|input| input.smooth_scroll_delta = egui::Vec2::ZERO);
        }
    }
}

fn orbit(form: &mut ScenarioForm, delta: egui::Vec2) {
    form.bearing = (form.bearing + f64::from(delta.x) * 0.4).rem_euclid(360.0);
    form.pitch = (form.pitch + f64::from(delta.y) * 0.3).clamp(MIN_PITCH, MAX_PITCH);
}

fn pan(form: &mut ScenarioForm, delta: egui::Vec2) {
    let z = tile_zoom(form.zoom);
    let scale = preview_scale(form.zoom);
    let center = GeoPoint::new(form.center_x, form.center_y);
    let Ok((fx, fy)) = center.tile_fraction(z) else {
        return;
    };
    let cos_pitch = form.pitch.to_radians().cos().max(0.12);
    let bearing = form.bearing.to_radians();
    let (sin, cos) = bearing.sin_cos();
    let dx = f64::from(delta.x);
    let dy = f64::from(delta.y);
    let map_dx = (dx * cos + dy * sin) / scale;
    let map_dy = (-dx * sin + dy * cos) / (scale * cos_pitch);
    let nx = fx * f64::from(TILE_PX) - map_dx;
    let ny = fy * f64::from(TILE_PX) - map_dy;
    if let Ok(point) =
        GeoPoint::from_tile_fraction(z, nx / f64::from(TILE_PX), ny / f64::from(TILE_PX))
    {
        form.center_x = point.longitude;
        form.center_y = point.latitude;
    }
}

fn draw_grid(ui: &mut egui::Ui, rect: egui::Rect, form: &ScenarioForm, dark: bool) {
    let painter = ui.painter().with_clip_rect(rect);
    let line_color = if dark {
        egui::Color32::from_rgb(58, 64, 72)
    } else {
        egui::Color32::from_rgb(188, 190, 184)
    };
    let major_color = if dark {
        egui::Color32::from_rgb(88, 98, 110)
    } else {
        egui::Color32::from_rgb(148, 152, 146)
    };
    let z = tile_zoom(form.zoom);
    let scale = preview_scale(form.zoom) as f32;
    let cos_pitch = form.pitch.to_radians().cos().max(0.12) as f32;
    let bearing = form.bearing.to_radians() as f32;
    let center = GeoPoint::new(form.center_x, form.center_y);
    let Ok((fx, fy)) = center.tile_fraction(z) else {
        return;
    };
    let center_px = egui::vec2(
        (fx * f64::from(TILE_PX)) as f32,
        (fy * f64::from(TILE_PX)) as f32,
    );
    // Ground resolution of one map pixel (Web Mercator) at this latitude/zoom.
    let meters_per_px = EQUATOR_CIRCUMFERENCE_METERS
        * form.center_y.to_radians().cos().abs().max(0.05)
        / (f64::from(TILE_PX) * 2f64.powi(i32::from(z)));
    let spacing = (form.grid_size_m / meters_per_px) as f32;
    if !spacing.is_finite() || spacing <= 0.0 {
        return;
    }
    if spacing * scale < 2.0 {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Zoom in untuk melihat grid",
            egui::FontId::proportional(12.0),
            if dark {
                egui::Color32::from_white_alpha(170)
            } else {
                egui::Color32::from_black_alpha(150)
            },
        );
        return;
    }
    let half = rect.width().hypot(rect.height()) / (2.0 * scale) + spacing * 2.0;
    let start_x = ((center_px.x - half) / spacing).floor() * spacing;
    let end_x = center_px.x + half;
    let start_y = ((center_px.y - half) / spacing).floor() * spacing;
    let end_y = center_px.y + half;
    let step = spacing.max(1.0);
    let mut x = start_x;
    let mut guard = 0;
    while x <= end_x && guard < 400 {
        let index = (x / step).round() as i64;
        let color = if index % 5 == 0 {
            major_color
        } else {
            line_color
        };
        let a = transform_map(
            egui::vec2(x, center_px.y - half),
            center_px,
            rect.center(),
            scale,
            bearing,
            cos_pitch,
        );
        let b = transform_map(
            egui::vec2(x, center_px.y + half),
            center_px,
            rect.center(),
            scale,
            bearing,
            cos_pitch,
        );
        painter.line_segment([a, b], egui::Stroke::new(1.0, color));
        x += step;
        guard += 1;
    }
    let mut y = start_y;
    guard = 0;
    while y <= end_y && guard < 400 {
        let index = (y / step).round() as i64;
        let color = if index % 5 == 0 {
            major_color
        } else {
            line_color
        };
        let a = transform_map(
            egui::vec2(center_px.x - half, y),
            center_px,
            rect.center(),
            scale,
            bearing,
            cos_pitch,
        );
        let b = transform_map(
            egui::vec2(center_px.x + half, y),
            center_px,
            rect.center(),
            scale,
            bearing,
            cos_pitch,
        );
        painter.line_segment([a, b], egui::Stroke::new(1.0, color));
        y += step;
        guard += 1;
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments)]
fn tile_mesh(
    rect: egui::Rect,
    center_px: egui::Vec2,
    map_origin: egui::Vec2,
    scale: f32,
    bearing: f32,
    cos_pitch: f32,
    texture: egui::TextureId,
) -> egui::Mesh {
    let mut mesh = egui::Mesh::with_texture(texture);
    for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
        let map = map_origin + egui::vec2(u * TILE_PX, v * TILE_PX);
        let pos = transform_map(map, center_px, rect.center(), scale, bearing, cos_pitch);
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv: egui::pos2(u, v),
            color: egui::Color32::WHITE,
        });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    mesh
}

#[allow(clippy::too_many_arguments)]
fn transform_map(
    map: egui::Vec2,
    center_px: egui::Vec2,
    screen_center: egui::Pos2,
    scale: f32,
    bearing: f32,
    cos_pitch: f32,
) -> egui::Pos2 {
    let delta = (map - center_px) * scale;
    let delta = egui::vec2(delta.x, delta.y * cos_pitch);
    let (sin, cos) = bearing.sin_cos();
    let rotated = egui::vec2(delta.x * cos - delta.y * sin, delta.x * sin + delta.y * cos);
    screen_center + rotated
}

fn draw_overlay(ui: &egui::Ui, rect: egui::Rect, form: &ScenarioForm, outdoor: bool) {
    let painter = ui.painter();
    painter.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.0, egui::Color32::from_white_alpha(45)),
        egui::StrokeKind::Inside,
    );
    let center = rect.center();
    let stroke = egui::Stroke::new(1.4, egui::Color32::from_white_alpha(190));
    painter.line_segment(
        [center - egui::vec2(9.0, 0.0), center + egui::vec2(9.0, 0.0)],
        stroke,
    );
    painter.line_segment(
        [center - egui::vec2(0.0, 9.0), center + egui::vec2(0.0, 9.0)],
        stroke,
    );
    painter.circle_stroke(center, 3.0, stroke);

    let hint = "Seret: geser · Shift+seret: pitch/bearing · Scroll: zoom";
    let galley = painter.layout_no_wrap(
        hint.to_owned(),
        egui::FontId::proportional(10.5),
        egui::Color32::WHITE,
    );
    let chip = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 7.0, rect.bottom() - galley.size().y - 11.0),
        galley.size() + egui::vec2(12.0, 6.0),
    );
    painter.rect_filled(chip, 4.0, egui::Color32::from_black_alpha(150));
    painter.galley(
        egui::pos2(chip.left() + 6.0, chip.top() + 3.0),
        galley,
        egui::Color32::WHITE,
    );

    let attribution = if outdoor {
        match form.base_layer.as_str() {
            "satellite" => "Tiles © Esri",
            _ => "© OpenStreetMap contributors",
        }
    } else if form.base_layer.starts_with("grid_") {
        "Grid"
    } else {
        "Blank"
    };
    painter.text(
        rect.right_bottom() + egui::vec2(-8.0, -7.0),
        egui::Align2::RIGHT_BOTTOM,
        attribution,
        egui::FontId::proportional(10.0),
        egui::Color32::from_white_alpha(190),
    );
}

#[cfg(target_arch = "wasm32")]
fn draw_outdoor_placeholder(ui: &egui::Ui, rect: egui::Rect) {
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "Pratinjau peta tersedia di aplikasi desktop",
        egui::FontId::proportional(12.0),
        egui::Color32::from_white_alpha(200),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form() -> ScenarioForm {
        ScenarioForm {
            id: None,
            operasi_id: String::new(),
            target_operasi_id: String::new(),
            name: String::new(),
            center_x: 115.225_263_767_569_5,
            center_y: -8.649_177_158_946_946,
            zoom: 18.0,
            min_zoom: 5.0,
            max_zoom: 26.0,
            pitch: 0.0,
            bearing: 0.0,
            map_mode: "outdoor".to_owned(),
            base_layer: "osm".to_owned(),
            grid_size_m: 1.0,
            indoor_config: "{}".to_owned(),
            copy: false,
            error: None,
        }
    }

    #[test]
    fn preview_scale_uses_fractional_zoom() {
        assert_eq!(tile_zoom(18.0), 18);
        assert_eq!(tile_zoom(18.9), 18);
        assert!((preview_scale(18.0) - 1.0).abs() < 1e-9);
        assert!((preview_scale(18.5) - 2f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn pan_moves_the_center_west_when_dragging_east() {
        let mut form = form();
        let before = (form.center_x, form.center_y);
        pan(&mut form, egui::vec2(120.0, 0.0));
        assert!(form.center_x < before.0);
        assert!((form.center_y - before.1).abs() < 1e-9);
    }

    #[test]
    fn orbit_clamps_pitch_and_wraps_bearing() {
        let mut form = form();
        form.bearing = 359.0;
        orbit(&mut form, egui::vec2(10.0, 500.0));
        assert_eq!(form.pitch, MAX_PITCH);
        assert!((0.0..360.0).contains(&form.bearing));
    }
}
