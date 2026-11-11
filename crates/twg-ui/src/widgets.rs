use bevy_egui::egui;
use chrono::{Datelike, Local, NaiveDate};
use twg_client::design::{
    CONTROL_HEIGHT, DARK, FONT_SM, FONT_XS, RADIUS_LG, RADIUS_SM, SPACE_2, SPACE_3, SPACE_4,
};

use crate::theme::{self, color, color_alpha};

pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    theme::card_frame().show(ui, add).inner
}

pub fn section(ui: &mut egui::Ui, title: &str, subtitle: Option<&str>) {
    ui.add_space(SPACE_2);
    ui.label(theme::heading(title));
    if let Some(subtitle) = subtitle {
        ui.label(theme::muted(subtitle));
    }
    ui.add_space(SPACE_2);
    ui.separator();
    ui.add_space(SPACE_2);
}

pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(
        egui::RichText::new(text)
            .size(FONT_SM)
            .color(color(DARK.primary_foreground)),
    )
    .fill(color(DARK.primary))
    .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8))
    .min_size(egui::vec2(0.0, CONTROL_HEIGHT));
    ui.add(button)
}

pub fn secondary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(egui::RichText::new(text).size(FONT_SM))
        .fill(color(DARK.secondary))
        .stroke(egui::Stroke::new(1.0, color(DARK.border)))
        .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8))
        .min_size(egui::vec2(0.0, CONTROL_HEIGHT));
    ui.add(button)
}

pub fn danger_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(
        egui::RichText::new(text)
            .size(FONT_SM)
            .color(color(DARK.destructive)),
    )
    .fill(color_alpha(DARK.destructive, 30))
    .stroke(egui::Stroke::new(1.0, color(DARK.destructive)))
    .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8))
    .min_size(egui::vec2(0.0, CONTROL_HEIGHT));
    ui.add(button)
}

pub fn ghost_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).size(FONT_SM))
            .fill(egui::Color32::TRANSPARENT)
            .min_size(egui::vec2(0.0, CONTROL_HEIGHT)),
    )
}

pub fn labeled_input(ui: &mut egui::Ui, label: &str, value: &mut String) -> egui::Response {
    labeled_input_with_width(ui, label, value, f32::INFINITY)
}

/// Seperti [`labeled_input`], tetapi lebar input dibatasi sehingga bisa dipakai
/// di dalam baris `horizontal_wrapped` tanpa memakan satu baris penuh.
pub fn labeled_input_with_width(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    width: f32,
) -> egui::Response {
    ui.label(egui::RichText::new(label).size(FONT_XS));
    ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(width)
            .margin(egui::Margin::symmetric(8, 5)),
    )
}

/// Password field with an eye show/hide toggle. `visible` is kept by the caller
/// so the state can be reset when a form is submitted or reopened.
pub fn password_input(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    visible: &mut bool,
) -> egui::Response {
    ui.label(egui::RichText::new(label).size(FONT_XS));
    ui.horizontal(|ui| {
        let button_width = 28.0;
        let input_width =
            (ui.available_width() - button_width - ui.spacing().item_spacing.x).max(80.0);
        let response = ui.add(
            egui::TextEdit::singleline(value)
                .password(!*visible)
                .desired_width(input_width)
                .margin(egui::Margin::symmetric(8, 5)),
        );
        if password_toggle(ui, *visible).clicked() {
            *visible = !*visible;
            response.request_focus();
        }
        response
    })
    .inner
}

/// Icon-only show/hide button. The eye is painted directly because the bundled
/// fonts do not contain a reliable eye glyph.
fn password_toggle(ui: &mut egui::Ui, visible: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect(
            rect,
            egui::CornerRadius::same(6),
            color(DARK.secondary),
            egui::Stroke::NONE,
            egui::StrokeKind::Inside,
        );
    }
    let tint = if response.hovered() {
        color(DARK.foreground)
    } else {
        color(DARK.muted_foreground)
    };
    paint_eye(painter, rect, visible, tint);
    response.on_hover_text(if visible {
        "Sembunyikan password"
    } else {
        "Tampilkan password"
    })
}

fn paint_eye(painter: &egui::Painter, rect: egui::Rect, open: bool, tint: egui::Color32) {
    let center = rect.center();
    let width = 15.0;
    let height = 9.0;
    let stroke = egui::Stroke::new(1.4, tint);
    let steps = 14;
    let mut points = Vec::with_capacity(steps * 2);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = center.x - width * 0.5 + width * t;
        let y = center.y - (std::f32::consts::PI * t).sin() * height * 0.5;
        points.push(egui::pos2(x, y));
    }
    for step in (1..steps).rev() {
        let t = step as f32 / steps as f32;
        let x = center.x - width * 0.5 + width * t;
        let y = center.y + (std::f32::consts::PI * t).sin() * height * 0.5;
        points.push(egui::pos2(x, y));
    }
    painter.add(egui::Shape::closed_line(points, stroke));
    if open {
        painter.circle_filled(center, 2.6, tint);
    } else {
        painter.line_segment(
            [
                center + egui::vec2(-width * 0.6, -height * 0.9),
                center + egui::vec2(width * 0.6, height * 0.9),
            ],
            stroke,
        );
    }
}

pub fn textarea(ui: &mut egui::Ui, label: &str, value: &mut String, rows: usize) -> egui::Response {
    ui.label(egui::RichText::new(label).size(FONT_XS));
    ui.add(
        egui::TextEdit::multiline(value)
            .desired_rows(rows)
            .desired_width(f32::INFINITY)
            .margin(egui::Margin::symmetric(8, 5)),
    )
}

pub fn search_input(ui: &mut egui::Ui, value: &mut String, hint: &str) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(220.0)
            .margin(egui::Margin::symmetric(8, 5)),
    )
}

pub fn radio(ui: &mut egui::Ui, value: &mut usize, index: usize, label: &str) -> egui::Response {
    ui.radio_value(value, index, egui::RichText::new(label).size(FONT_SM))
}

pub fn checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    ui.checkbox(value, egui::RichText::new(label).size(FONT_SM))
}

pub fn switch(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    ui.horizontal(|ui| {
        let response = ui.add(
            egui::Button::new(
                egui::RichText::new(if *value { "ON" } else { "OFF" })
                    .size(FONT_XS)
                    .color(if *value {
                        color(DARK.primary_foreground)
                    } else {
                        color(DARK.muted_foreground)
                    }),
            )
            .fill(if *value {
                color(DARK.success)
            } else {
                color(DARK.secondary)
            })
            .corner_radius(egui::CornerRadius::same(12))
            .min_size(egui::vec2(40.0, 22.0)),
        );
        if response.clicked() {
            *value = !*value;
        }
        ui.label(egui::RichText::new(label).size(FONT_SM));
        response
    })
    .inner
}

pub fn combo<T: Clone + PartialEq>(
    ui: &mut egui::Ui,
    id: egui::Id,
    selected: &mut T,
    options: &[(T, String)],
) -> egui::Response {
    let selected_label = options
        .iter()
        .find(|(value, _)| value == selected)
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| "—".to_owned());
    let response = egui::ComboBox::from_id_salt(id)
        .selected_text(selected_label)
        .width(220.0)
        .show_ui(ui, |ui| {
            for (value, label) in options {
                if ui.selectable_label(value == selected, label).clicked() {
                    *selected = value.clone();
                }
            }
        });
    response.response
}

pub fn table(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    headers: &[&str],
    add_rows: impl FnOnce(&mut egui::Ui),
) {
    egui::ScrollArea::horizontal().show(ui, |ui| {
        egui::Grid::new(id)
            .num_columns(headers.len())
            .striped(true)
            .spacing(egui::vec2(SPACE_4, SPACE_2))
            .min_col_width(90.0)
            .show(ui, |ui| {
                for header in headers {
                    ui.label(
                        egui::RichText::new(*header)
                            .size(FONT_XS)
                            .strong()
                            .color(color(DARK.muted_foreground)),
                    );
                }
                ui.end_row();
                add_rows(ui);
            });
    });
}

pub fn loading(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(theme::muted(label));
    });
}

pub fn empty(ui: &mut egui::Ui, text: &str) {
    ui.add_space(SPACE_4);
    ui.vertical_centered(|ui| {
        ui.label(theme::muted(text));
    });
    ui.add_space(SPACE_4);
}

pub fn error_banner(ui: &mut egui::Ui, message: &str) {
    egui::Frame::new()
        .fill(color_alpha(DARK.destructive, 25))
        .stroke(egui::Stroke::new(1.0, color(DARK.destructive)))
        .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8))
        .inner_margin(egui::Margin::symmetric(SPACE_3 as i8, SPACE_2 as i8))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(message)
                    .size(FONT_SM)
                    .color(color(DARK.destructive)),
            );
        });
}

pub fn success_banner(ui: &mut egui::Ui, message: &str) {
    egui::Frame::new()
        .fill(color_alpha(DARK.success, 20))
        .stroke(egui::Stroke::new(1.0, color(DARK.success)))
        .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8))
        .inner_margin(egui::Margin::symmetric(SPACE_3 as i8, SPACE_2 as i8))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(message)
                    .size(FONT_SM)
                    .color(color(DARK.success)),
            );
        });
}

pub fn badge(ui: &mut egui::Ui, text: &str, rgb: twg_client::design::Rgb) {
    egui::Frame::new()
        .fill(color_alpha(rgb, 35))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(FONT_XS).color(color(rgb)));
        });
}

pub fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, hint: &str) {
    card(ui, |ui| {
        ui.set_min_width(150.0);
        ui.label(
            egui::RichText::new(label)
                .size(FONT_XS)
                .color(color(DARK.muted_foreground)),
        );
        ui.label(egui::RichText::new(value).size(26.0).strong());
        ui.label(theme::muted(hint));
    });
}

pub fn tabs(ui: &mut egui::Ui, current: &mut usize, labels: &[&str]) {
    ui.horizontal(|ui| {
        for (index, label) in labels.iter().enumerate() {
            let selected = *current == index;
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(*label).size(FONT_SM).color(
                        if selected {
                            color(DARK.foreground)
                        } else {
                            color(DARK.muted_foreground)
                        },
                    ))
                    .fill(if selected {
                        color(DARK.secondary)
                    } else {
                        egui::Color32::TRANSPARENT
                    })
                    .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8)),
                )
                .clicked()
            {
                *current = index;
            }
        }
    });
}

pub fn date_picker(ui: &mut egui::Ui, id: egui::Id, value: &mut Option<NaiveDate>) {
    let label = value
        .map(|date| date.format("%d %b %Y").to_string())
        .unwrap_or_else(|| "Semua tanggal".to_owned());
    ui.menu_button(label, |ui| {
        let today = Local::now().date_naive();
        let mut date = value.unwrap_or(today);
        let mut year = date.year();
        let mut month = date.month();
        let mut day = date.day();
        ui.horizontal(|ui| {
            ui.label("Tahun");
            ui.add(egui::DragValue::new(&mut year).range(2000..=2100));
            ui.label("Bulan");
            ui.add(egui::DragValue::new(&mut month).range(1..=12));
            ui.label("Hari");
            ui.add(egui::DragValue::new(&mut day).range(1..=31));
        });
        date = NaiveDate::from_ymd_opt(year, month, day).unwrap_or(today);
        if ui.button("Hari ini").clicked() {
            *value = Some(today);
            ui.close();
        }
        if ui.button("Terapkan").clicked() {
            *value = Some(date);
            ui.close();
        }
        if ui.button("Hapus filter").clicked() {
            *value = None;
            ui.close();
        }
    })
    .response
    .on_hover_text("Pilih tanggal");
    let _ = id;
}

/// Drag-and-drop reorder for the base-layer list.
pub fn reorderable_layers(
    ui: &mut egui::Ui,
    layers: &mut Vec<&'static str>,
    selected: &mut &'static str,
) -> bool {
    let mut move_request = None;
    let mut changed = false;
    for (index, &name) in layers.iter().enumerate() {
        let selected_here = name == *selected;
        let frame = egui::Frame::new()
            .fill(if selected_here {
                color(DARK.secondary)
            } else {
                egui::Color32::TRANSPARENT
            })
            .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8))
            .inner_margin(egui::Margin::symmetric(6, 4));
        let (inner, dropped) = ui.dnd_drop_zone::<usize, _>(frame, |ui| {
            ui.dnd_drag_source(egui::Id::new(("layer", index)), index, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("::").color(color(DARK.muted_foreground)));
                    ui.label(egui::RichText::new(name).size(FONT_SM));
                });
            })
        });
        if inner.inner.response.clicked() {
            *selected = name;
        }
        if let Some(from) = dropped {
            move_request = Some((*from, index));
        }
    }
    if let Some((from, to)) = move_request
        && from != to
    {
        let layer = layers.remove(from);
        layers.insert(to.min(layers.len()), layer);
        changed = true;
    }
    changed
}

/// Right-side sheet overlay. Returns `false` when the user closes it.
pub fn sheet(
    context: &egui::Context,
    id: &str,
    title: &str,
    open: &mut bool,
    add: impl FnOnce(&mut egui::Ui),
) -> bool {
    egui::Window::new(title)
        .id(egui::Id::new(id))
        .open(open)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
        .default_width(360.0)
        .show(context, add);
    *open
}

pub fn modal(context: &egui::Context, title: &str, add: impl FnOnce(&mut egui::Ui)) -> bool {
    let mut open = true;
    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .min_width(380.0)
        .show(context, add);
    !open
}

pub fn spacer(ui: &mut egui::Ui) {
    ui.add_space(SPACE_3);
}

pub fn key_hint(ui: &mut egui::Ui, key: &str, label: &str) {
    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(color(DARK.secondary))
            .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8))
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(key).size(FONT_XS).monospace());
            });
        ui.label(
            egui::RichText::new(label)
                .size(FONT_XS)
                .color(color(DARK.muted_foreground)),
        );
    });
}

pub fn panel_rounding() -> egui::CornerRadius {
    egui::CornerRadius::same(RADIUS_LG as u8)
}
