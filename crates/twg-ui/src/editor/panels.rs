use bevy_egui::egui;
use twg_client::design::{DARK, FONT_SM, FONT_XS};
use twg_map::{BaseLayer, GeoPoint};
use twg_render::{EditorModelShading, EditorShapeKind};

use crate::state::ClientUi;
use crate::theme;

use super::input::distance_meters;
use super::model::{
    AssetMarkerDraft, EditorAssetRow, EditorKind, EditorState, EditorTextureRow, EditorTool,
    MIN_MARKER_SCALE, ObjectKey, ObjectType, ParticleSettings, marker_color, parse_hex_color,
    particle_kind_label,
};
use super::textures;

#[derive(Debug, Clone, PartialEq, Eq)]
enum EditorAction {
    None,
    Save,
    SaveAndClose,
    Close,
    Reload,
    SaveAndReload,
    Geocode,
    SaveView,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    UploadTexture {
        key: ObjectKey,
        path: std::path::PathBuf,
    },
}

pub fn draw(context: &egui::Context, client: &mut ClientUi) {
    client.ensure_ws();
    client.editor.base_url.clone_from(&client.base_url);
    client.sync_editor_marker_models();
    let asset_rows = client.editor_asset_rows();
    let texture_rows = client.editor_texture_rows();
    let can_admin = client.role().is_admin_like();
    let mut action = EditorAction::None;
    {
        let editor = &mut client.editor;
        root_ui(context, |ui| {
            if editor.preview {
                draw_preview_bar(ui, editor);
            } else {
                draw_menu(ui, editor, &mut action);
                draw_toolbar(ui, editor, &mut action);
                draw_status(ui, editor);
                draw_console_panel(ui, editor);
                draw_scene_panel(ui, editor, &asset_rows);
                draw_inspector(ui, editor, can_admin, &texture_rows, &mut action);
            }
            draw_viewport(ui, editor);
        });
        draw_context_menu(context, editor);
        draw_dialogs(context, editor, &mut action);
        if editor.requested_geocode {
            editor.requested_geocode = false;
            action = EditorAction::Geocode;
        }
        if editor.loading {
            egui::Area::new(egui::Id::new("editor_loading"))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(context, |ui| {
                    egui::Frame::new()
                        .fill(theme::color(DARK.card))
                        .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::same(18))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(theme::title("Memuat skenario..."));
                            });
                        });
                });
        }
    }

    if client.editor.requested_asset_sync {
        client.editor.requested_asset_sync = false;
        client.sync_assets_now();
    }

    match action {
        EditorAction::Save => client.editor_save(),
        EditorAction::SaveAndClose => {
            client.editor_save();
            client.close_editor();
        }
        EditorAction::Close => client.close_editor(),
        EditorAction::Reload => client.reload_editor(),
        EditorAction::SaveAndReload => {
            client.editor_save();
            client.reload_editor();
        }
        EditorAction::Geocode => client.editor_geocode(),
        EditorAction::SaveView => client.editor_save_view(),
        EditorAction::UploadTexture { key, path } => client.editor_upload_texture(key, path),
        EditorAction::None => {}
    }
}

fn root_ui<R>(context: &egui::Context, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut ui = egui::Ui::new(
        context.clone(),
        egui::Id::new("twg_editor_root"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(context.viewport_rect()),
    );
    add(&mut ui)
}

fn draw_preview_bar(ui: &mut egui::Ui, editor: &mut EditorState) {
    egui::Panel::top("editor_preview_bar")
        .exact_size(34.0)
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .inner_margin(egui::Margin::symmetric(10, 4)),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(egui::RichText::new("▶ PLAY").strong());
                ui.separator();
                ui.label(&editor.scenario_name);
                ui.separator();
                ui.label(theme::muted(&format!(
                    "{} unit · {} dipilih",
                    editor.play.units.len(),
                    editor.selected.len()
                )));
                ui.separator();
                ui.label(theme::muted(
                    "Klik/Shift pilih · seret kotak · klik kanan gerak",
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("■ Stop").clicked() {
                        editor.preview = false;
                        editor.touch();
                    }
                });
            });
        });
}

fn draw_menu(ui: &mut egui::Ui, editor: &mut EditorState, action: &mut EditorAction) {
    egui::Panel::top("editor_menu")
        .exact_size(34.0)
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .inner_margin(egui::Margin::symmetric(8, 3)),
        )
        .show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Simpan            Ctrl+S").clicked() {
                        *action = EditorAction::Save;
                        ui.close();
                    }
                    if ui.button("Muat ulang").clicked() {
                        if editor.dirty {
                            editor.confirm_reload = true;
                        } else {
                            *action = EditorAction::Reload;
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Tutup editor").clicked() {
                        if editor.dirty {
                            editor.confirm_close = true;
                        } else {
                            *action = EditorAction::Close;
                        }
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(
                            !editor.undo.is_empty(),
                            egui::Button::new("Undo       Ctrl+Z"),
                        )
                        .clicked()
                    {
                        editor.undo();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !editor.redo.is_empty(),
                            egui::Button::new("Redo       Ctrl+Y"),
                        )
                        .clicked()
                    {
                        editor.redo();
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            !editor.selected.is_empty(),
                            egui::Button::new("Duplicate  Ctrl+D"),
                        )
                        .clicked()
                    {
                        editor.duplicate_selected();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !editor.selected.is_empty(),
                            egui::Button::new("Delete     Del"),
                        )
                        .clicked()
                    {
                        editor.delete_selected();
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    let mut changed = false;
                    changed |= ui.checkbox(&mut editor.show_grid, "Grid overlay").changed();
                    changed |= ui
                        .checkbox(&mut editor.labels_visible, "Label objek")
                        .changed();
                    changed |= ui
                        .checkbox(&mut editor.visible.raster, "Tile raster")
                        .changed();
                    ui.separator();
                    changed |= ui
                        .checkbox(&mut editor.show_console, "Panel console")
                        .changed();
                    if changed {
                        editor.touch();
                    }
                });
                ui.menu_button("Help", |ui| {
                    ui.label("Q/W/E/R  Select · Move · Rotate · Scale");
                    ui.label("1..4     Bentuk · Line · Text · Measure");
                    ui.label("Marker 3D ditambahkan dari tab Assets");
                    ui.label("Klik ganda / Enter: selesai menggambar");
                    ui.label("Esc: batal · Del: hapus · G: snap");
                    ui.label("Klik kanan tahan: orbit · tengah: pan");
                });
            });
        });
}

fn draw_toolbar(ui: &mut egui::Ui, editor: &mut EditorState, action: &mut EditorAction) {
    egui::Panel::top("editor_toolbar")
        .exact_size(40.0)
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.background))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .inner_margin(egui::Margin::symmetric(8, 4)),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                for tool in EditorTool::ALL {
                    let selected = editor.tool == tool;
                    let clicked = match tool {
                        EditorTool::Select
                        | EditorTool::Move
                        | EditorTool::Rotate
                        | EditorTool::Scale => tool_icon_button(ui, tool, selected).clicked(),
                        _ => {
                            let text =
                                egui::RichText::new(format!("{}  {}", tool.icon(), tool.label()))
                                    .size(FONT_XS)
                                    .color(if selected {
                                        theme::color(DARK.primary_foreground)
                                    } else {
                                        theme::color(DARK.foreground)
                                    });
                            ui.add(
                                egui::Button::new(text)
                                    .fill(if selected {
                                        theme::color(DARK.primary)
                                    } else {
                                        theme::color(DARK.secondary)
                                    })
                                    .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                                    .corner_radius(egui::CornerRadius::same(4))
                                    .min_size(egui::vec2(0.0, 24.0)),
                            )
                            .clicked()
                        }
                    };
                    if clicked {
                        editor.set_tool(tool);
                    }
                }
                ui.separator();
                if editor.tool == EditorTool::Shape {
                    ui.label("Bentuk");
                    egui::ComboBox::from_id_salt("editor_shape_kind")
                        .selected_text(editor.active_shape.label())
                        .width(128.0)
                        .show_ui(ui, |ui| {
                            for shape in EditorShapeKind::ALL {
                                if ui
                                    .selectable_label(editor.active_shape == shape, shape.label())
                                    .clicked()
                                {
                                    editor.active_shape = shape;
                                }
                            }
                        });
                    ui.separator();
                }
                if editor.tool == EditorTool::Particle {
                    ui.label("Jenis");
                    egui::ComboBox::from_id_salt("editor_particle_kind")
                        .selected_text(particle_kind_label(editor.active_particle))
                        .width(128.0)
                        .show_ui(ui, |ui| {
                            for kind in twg_contracts::map::ParticleKind::ALL {
                                if ui
                                    .selectable_label(
                                        editor.active_particle == kind,
                                        particle_kind_label(kind),
                                    )
                                    .clicked()
                                {
                                    editor.active_particle = kind;
                                }
                            }
                        });
                    ui.separator();
                }
                ui.add_enabled_ui(!editor.preview, |ui| {
                    ui.checkbox(&mut editor.particle_preview, "Animasi particle")
                        .on_hover_text(
                            "Khusus editor. Saat dimatikan efek disembunyikan dan tidak dianimasikan; \
                             Play/simulasi tetap menampilkan animasi particle.",
                        );
                });
                ui.separator();
                ui.label("Snap");
                egui::ComboBox::from_id_salt("editor_snap")
                    .selected_text(snap_label(editor.snap_meters))
                    .width(78.0)
                    .show_ui(ui, |ui| {
                        for value in [0.0, 1.0, 5.0, 10.0, 25.0] {
                            ui.selectable_value(&mut editor.snap_meters, value, snap_label(value));
                        }
                    });
                if ui
                    .selectable_label(editor.indoor, "Indoor")
                    .on_hover_text("Sembunyikan raster dan tampilkan grid lantai")
                    .clicked()
                {
                    editor.indoor = !editor.indoor;
                    editor.touch();
                }
                ui.separator();
                ui.label("Model");
                egui::ComboBox::from_id_salt("editor_model_shading")
                    .selected_text(editor.model_shading.label())
                    .width(96.0)
                    .show_ui(ui, |ui| {
                        for shading in EditorModelShading::ALL {
                            if ui
                                .selectable_label(editor.model_shading == shading, shading.label())
                                .clicked()
                                && editor.model_shading != shading
                            {
                                editor.model_shading = shading;
                                editor.touch();
                            }
                        }
                    });
                ui.separator();
                if ui
                    .add(egui::Button::new(if editor.dirty {
                        "● Simpan"
                    } else {
                        "Simpan"
                    }))
                    .clicked()
                {
                    *action = EditorAction::Save;
                }
                if ui.button("▶ Play").clicked() {
                    editor.preview = true;
                    editor.touch();
                }
                if ui.button("◀").on_hover_text("Kembali").clicked() {
                    if editor.dirty {
                        editor.confirm_close = true;
                    } else {
                        *action = EditorAction::Close;
                    }
                }
            });
        });
}

/// Icon-only button for the transform tools. The icons are painted directly so
/// they do not depend on glyph coverage in the bundled fonts.
fn tool_icon_button(ui: &mut egui::Ui, tool: EditorTool, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(30.0, 24.0), egui::Sense::click());
    let fill = if selected {
        theme::color(DARK.primary)
    } else if response.hovered() {
        theme::color(DARK.secondary)
    } else {
        theme::color(DARK.card)
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(4),
        fill,
        egui::Stroke::new(
            1.0,
            theme::color(if selected { DARK.primary } else { DARK.border }),
        ),
        egui::StrokeKind::Inside,
    );
    let color = if selected {
        theme::color(DARK.primary_foreground)
    } else {
        theme::color(DARK.foreground)
    };
    paint_tool_icon(ui.painter(), rect, tool, color);
    response.on_hover_text(format!("{} ({})", tool.label(), tool.shortcut()))
}

fn paint_tool_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    tool: EditorTool,
    color: egui::Color32,
) {
    let center = rect.center();
    let stroke = egui::Stroke::new(1.5, color);
    match tool {
        EditorTool::Select => {
            // Mouse pointer: filled triangle plus a short tail.
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + egui::vec2(-2.0, -7.0),
                    center + egui::vec2(5.5, 1.2),
                    center + egui::vec2(-4.2, 3.0),
                ],
                color,
                egui::Stroke::NONE,
            ));
            painter.line_segment(
                [
                    center + egui::vec2(-1.4, 2.4),
                    center + egui::vec2(1.4, 7.4),
                ],
                stroke,
            );
        }
        EditorTool::Move => {
            for direction in [
                egui::vec2(1.0, 0.0),
                egui::vec2(-1.0, 0.0),
                egui::vec2(0.0, 1.0),
                egui::vec2(0.0, -1.0),
            ] {
                let tip = center + direction * 7.0;
                painter.line_segment([center, tip], stroke);
                let side = egui::vec2(-direction.y, direction.x) * 2.4;
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        tip,
                        tip - direction * 3.2 + side,
                        tip - direction * 3.2 - side,
                    ],
                    color,
                    egui::Stroke::NONE,
                ));
            }
        }
        EditorTool::Rotate => {
            let points = (0..=36)
                .map(|step| {
                    let angle = -std::f32::consts::FRAC_PI_2
                        + step as f32 / 36.0 * std::f32::consts::TAU * 0.82;
                    center + egui::vec2(angle.cos(), angle.sin()) * 6.4
                })
                .collect::<Vec<_>>();
            painter.add(egui::Shape::line(points.clone(), stroke));
            if let Some(end) = points.last().copied() {
                let direction = egui::vec2(0.30, -0.95);
                let side = egui::vec2(-direction.y, direction.x) * 2.6;
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        end + direction * 2.4,
                        end - direction * 2.0 + side,
                        end - direction * 2.0 - side,
                    ],
                    color,
                    egui::Stroke::NONE,
                ));
            }
        }
        EditorTool::Scale => {
            painter.line_segment(
                [
                    center + egui::vec2(-5.5, 5.5),
                    center + egui::vec2(5.5, -5.5),
                ],
                stroke,
            );
            for offset in [egui::vec2(-5.5, 5.5), egui::vec2(5.5, -5.5)] {
                painter.rect_filled(
                    egui::Rect::from_center_size(center + offset, egui::vec2(4.2, 4.2)),
                    egui::CornerRadius::same(1),
                    color,
                );
            }
        }
        _ => {}
    }
}

fn snap_label(value: f32) -> String {
    if value <= 0.0 {
        "Off".to_owned()
    } else {
        format!("{value:.0} m")
    }
}

fn draw_status(ui: &mut egui::Ui, editor: &EditorState) {
    egui::Panel::bottom("editor_status")
        .exact_size(24.0)
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .inner_margin(egui::Margin::symmetric(8, 2)),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let cursor = editor
                    .cursor_geo
                    .map(|point| format!("{:.6}, {:.6}", point.latitude, point.longitude))
                    .unwrap_or_else(|| "—, —".to_owned());
                let dirty = if editor.dirty { " ●" } else { "" };
                ui.monospace(format!("{}{}", editor.scenario_name, dirty));
                ui.separator();
                ui.monospace(format!("cursor {cursor}"));
                ui.separator();
                ui.monospace(format!(
                    "z {:.2} · tile z{} · {:.2} m/px",
                    editor.map_state.zoom,
                    editor.map_state.tile_zoom_for(19),
                    editor.map_state.meters_per_pixel()
                ));
                ui.separator();
                ui.monospace(format!(
                    "bearing {:.1}° · pitch {:.1}°",
                    editor.map_state.bearing_degrees, editor.map_state.pitch_degrees
                ));
                ui.separator();
                ui.monospace(format!("{:.0} FPS", editor.fps));
                ui.separator();
                ui.monospace(format!(
                    "{} · {} objek · {} dipilih",
                    if editor.indoor { "Indoor" } else { "Outdoor" },
                    editor.objects.len(),
                    editor.selected.len()
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(error) = &editor.save_error {
                        ui.colored_label(theme::color(DARK.destructive), error);
                    } else if let Some(saved) = &editor.last_save {
                        ui.colored_label(theme::color(DARK.success), format!("Tersimpan {saved}"));
                    } else if editor.dirty {
                        ui.colored_label(theme::color(DARK.warning), "Belum disimpan");
                    } else {
                        ui.label(theme::muted("Tersimpan"));
                    }
                    if editor.snap_meters > 0.0 {
                        ui.separator();
                        ui.monospace(format!("snap {:.0} m", editor.snap_meters));
                    }
                });
            });
        });
}

fn draw_console_panel(ui: &mut egui::Ui, editor: &mut EditorState) {
    if !editor.show_console {
        return;
    }
    egui::Panel::bottom("editor_bottom")
        .resizable(true)
        .default_size(120.0)
        .min_size(56.0)
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .inner_margin(egui::Margin::symmetric(8, 4)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::title("Console"));
                ui.separator();
                ui.label(theme::muted("Log editor"));
            });
            ui.separator();
            draw_console_tab(ui, editor);
        });
}

fn draw_asset_tab(ui: &mut egui::Ui, editor: &mut EditorState, asset_rows: &[EditorAssetRow]) {
    if asset_rows.is_empty() {
        ui.add_space(6.0);
        ui.label(theme::muted(
            "Belum ada model di server. Unggah GLB/glTF lewat menu Aset.",
        ));
        return;
    }
    let missing = asset_rows.iter().filter(|row| !row.cached).count();
    if missing > 0 {
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::muted(&format!(
                "{missing} model belum ada di cache offline."
            )));
            if ui.small_button("Sinkronkan").clicked() {
                editor.requested_asset_sync = true;
            }
        });
    }
    ui.label(theme::muted(if editor.asset_drag.is_some() {
        "Lepas di peta untuk menambah marker"
    } else {
        "Tarik kartu ke peta atau klik Tambah"
    }));
    egui::ScrollArea::vertical()
        .id_salt("editor_asset_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let width = ui.available_width();
            for row in asset_rows {
                asset_card(ui, row, editor, width);
                ui.add_space(6.0);
            }
        });
    if let Some(drag) = &editor.asset_drag
        && let Some(position) = ui.ctx().pointer_latest_pos()
    {
        egui::Area::new(egui::Id::new("editor_asset_drag"))
            .order(egui::Order::Tooltip)
            .fixed_pos(position + egui::vec2(14.0, 14.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(theme::color(DARK.secondary))
                    .stroke(egui::Stroke::new(1.0, theme::color(DARK.ring)))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&drag.name).size(FONT_SM));
                    });
            });
    }
}

fn asset_card(ui: &mut egui::Ui, row: &EditorAssetRow, editor: &mut EditorState, width: f32) {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width.max(160.0), 80.0),
        egui::Sense::click_and_drag(),
    );
    let dragging = editor
        .asset_drag
        .as_ref()
        .is_some_and(|drag| drag.unit_id == row.unit_id);
    let fill = if dragging || response.hovered() {
        theme::color(DARK.secondary)
    } else {
        theme::color(DARK.card)
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(6),
        fill,
        egui::Stroke::new(
            1.0,
            theme::color(if dragging { DARK.ring } else { DARK.border }),
        ),
        egui::StrokeKind::Inside,
    );
    let mut content = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(9.0, 7.0)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    content.spacing_mut().item_spacing.y = 2.0;
    content.label(
        egui::RichText::new(&row.name)
            .size(FONT_SM)
            .strong()
            .color(theme::color(DARK.foreground)),
    );
    content.label(theme::muted(&format!(
        "{} · {}",
        row.kind_label,
        if row.cached {
            "siap offline"
        } else {
            "belum diunduh"
        }
    )));
    content.horizontal(|ui| {
        if ui
            .add_enabled(row.cached, egui::Button::new("Tambah").small())
            .on_hover_text("Tambah marker di tengah peta")
            .clicked()
        {
            let draft = AssetMarkerDraft {
                unit_id: row.unit_id.clone(),
                name: row.name.clone(),
            };
            let point = editor.snap_point(editor.map_state.center);
            editor.add_asset_marker(&draft, point);
        }
        ui.label(theme::muted("tarik ke peta"));
    });
    if row.cached && response.drag_started() {
        editor.asset_drag = Some(AssetMarkerDraft {
            unit_id: row.unit_id.clone(),
            name: row.name.clone(),
        });
    }
}

fn draw_console_tab(ui: &mut egui::Ui, editor: &mut EditorState) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for line in &editor.console {
                ui.monospace(line);
            }
        });
}

fn draw_scene_panel(ui: &mut egui::Ui, editor: &mut EditorState, asset_rows: &[EditorAssetRow]) {
    egui::Panel::left("editor_scene")
        .resizable(true)
        .default_size(250.0)
        .min_size(210.0)
        .max_size(460.0)
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .inner_margin(egui::Margin::same(8)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (index, label) in ["Scene", "Assets"].into_iter().enumerate() {
                    let selected = editor.left_tab == index;
                    let text = egui::RichText::new(label).size(FONT_XS).color(if selected {
                        theme::color(DARK.primary_foreground)
                    } else {
                        theme::color(DARK.foreground)
                    });
                    if ui
                        .add(
                            egui::Button::new(text)
                                .fill(if selected {
                                    theme::color(DARK.primary)
                                } else {
                                    theme::color(DARK.secondary)
                                })
                                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                                .corner_radius(egui::CornerRadius::same(4))
                                .min_size(egui::vec2(0.0, 22.0)),
                        )
                        .clicked()
                    {
                        editor.left_tab = index;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if editor.left_tab == 0 {
                        ui.monospace(format!("{}", editor.objects.len()));
                    }
                });
            });
            ui.separator();
            match editor.left_tab {
                1 => draw_asset_tab(ui, editor, asset_rows),
                _ => draw_scene_tree(ui, editor),
            }
        });
}

fn draw_scene_tree(ui: &mut egui::Ui, editor: &mut EditorState) {
    ui.label(theme::title("Layers"));
    let mut layers_changed = false;
    egui::Grid::new("editor_layers")
        .num_columns(2)
        .show(ui, |ui| {
            layers_changed |= ui.checkbox(&mut editor.visible.marker, "Marker").changed();
            layers_changed |= ui.checkbox(&mut editor.visible.line, "Line").changed();
            ui.end_row();
            layers_changed |= ui
                .checkbox(&mut editor.visible.building, "Bentuk")
                .changed();
            layers_changed |= ui.checkbox(&mut editor.visible.text, "Text").changed();
            ui.end_row();
            layers_changed |= ui
                .checkbox(&mut editor.visible.particle, "Particle")
                .changed();
            layers_changed |= ui.checkbox(&mut editor.visible.raster, "Raster").changed();
            ui.end_row();
        });
    if layers_changed {
        editor.touch();
    }

    ui.separator();
    ui.label(theme::title("Geocode"));
    ui.horizontal(|ui| {
        let response = ui.add(
            egui::TextEdit::singleline(&mut editor.geocode_query)
                .hint_text("Cari lokasi...")
                .desired_width(150.0),
        );
        let submit = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if (ui.button("Cari").clicked() || submit) && !editor.geocode_pending {
            editor.requested_geocode = true;
        }
    });
    for (index, result) in editor.geocode_results.iter().enumerate() {
        if ui
            .selectable_label(false, &result.label)
            .on_hover_text(format!("{}, {}", result.latitude, result.longitude))
            .clicked()
        {
            let mut target = editor.map_state;
            target.center = twg_map::GeoPoint::new(result.longitude, result.latitude);
            target.zoom = target.zoom.max(16.0);
            editor.requested_flight = Some(target);
            editor.geocode_results.remove(index);
            break;
        }
    }
    if editor.geocode_pending {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(theme::muted("Mencari..."));
        });
    }

    ui.separator();
    let mut select: Option<(ObjectKey, bool)> = None;
    let mut toggle_visibility: Option<ObjectKey> = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for object_type in ObjectType::ALL {
                let rows = editor
                    .objects
                    .iter()
                    .filter(|object| object.object_type() == object_type)
                    .map(|object| {
                        (
                            object.key,
                            object.name.clone(),
                            object.visible,
                            editor.selected.contains(&object.key),
                        )
                    })
                    .collect::<Vec<_>>();
                let header = format!(
                    "{} {} ({})",
                    object_type.icon(),
                    object_type.label(),
                    rows.len()
                );
                egui::CollapsingHeader::new(header)
                    .default_open(true)
                    .show(ui, |ui| {
                        if rows.is_empty() {
                            ui.label(theme::muted("—"));
                        }
                        for (key, name, visible, selected) in rows {
                            ui.horizontal(|ui| {
                                let mut visible = visible;
                                if ui
                                    .checkbox(&mut visible, "")
                                    .on_hover_text("Visibilitas")
                                    .changed()
                                {
                                    toggle_visibility = Some(key);
                                }
                                let additive = ui
                                    .ctx()
                                    .input(|input| input.modifiers.ctrl || input.modifiers.command);
                                if ui.selectable_label(selected, name).clicked() {
                                    select = Some((key, additive));
                                }
                            });
                        }
                    });
            }
        });
    if let Some((key, additive)) = select {
        if additive {
            editor.toggle_selected(key);
        } else {
            editor.select_only(key);
        }
        // Clicking a layer is also a "go to object" action.
        editor.focus_object(key);
    }
    if let Some(key) = toggle_visibility {
        if let Some(object) = editor.object_mut(key) {
            object.visible = !object.visible;
        }
        editor.touch();
    }
}

fn draw_inspector(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    can_admin: bool,
    texture_rows: &[EditorTextureRow],
    action: &mut EditorAction,
) {
    egui::Panel::right("editor_inspector")
        .resizable(true)
        .default_size(300.0)
        .min_size(230.0)
        .max_size(460.0)
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .inner_margin(egui::Margin::same(10)),
        )
        .show(ui, |ui| {
            ui.label(theme::title("Inspector"));
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if editor.selected.len() == 1 {
                        let key = *editor.selected.iter().next().unwrap_or(&0);
                        draw_object_inspector(editor, key, ui, texture_rows, action);
                    } else if editor.selected.len() > 1 {
                        ui.label(format!("{} objek dipilih", editor.selected.len()));
                        ui.add_space(6.0);
                        if ui.button("Duplicate").clicked() {
                            editor.duplicate_selected();
                        }
                        if ui.button("Hapus").clicked() {
                            editor.delete_selected();
                        }
                    } else {
                        draw_scenario_inspector(editor, ui, can_admin, action);
                    }
                });
        });
}

fn draw_scenario_inspector(
    editor: &mut EditorState,
    ui: &mut egui::Ui,
    can_admin: bool,
    action: &mut EditorAction,
) {
    ui.label(theme::title("Skenario"));
    ui.monospace(&editor.scenario_name);
    if let Some(id) = editor.scenario_id {
        ui.label(theme::muted(&format!("ID {id}")));
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Base layer");
        if ui
            .selectable_label(matches!(editor.base_layer, BaseLayer::OpenStreetMap), "OSM")
            .clicked()
        {
            editor.requested_layer = Some(BaseLayer::OpenStreetMap);
        }
        if ui
            .selectable_label(
                matches!(editor.base_layer, BaseLayer::ArcGisImagery),
                "Satelit",
            )
            .clicked()
        {
            editor.requested_layer = Some(BaseLayer::ArcGisImagery);
        }
    });
    ui.horizontal(|ui| {
        ui.label("Zoom");
        ui.monospace(format!("{:.2}", editor.map_state.zoom));
        if ui.small_button("−").clicked() {
            editor.requested_zoom = Some(-0.5);
        }
        if ui.small_button("+").clicked() {
            editor.requested_zoom = Some(0.5);
        }
        if ui.small_button("N").clicked() {
            editor.requested_north = true;
        }
    });
    let mut camera = editor.map_state;
    let mut camera_changed = false;
    camera_changed |= ui
        .add(
            egui::Slider::new(&mut camera.bearing_degrees, 0.0..=360.0)
                .text("Bearing")
                .suffix("°"),
        )
        .changed();
    camera_changed |= ui
        .add(
            egui::Slider::new(&mut camera.pitch_degrees, 0.0..=85.0)
                .text("Pitch")
                .suffix("°"),
        )
        .changed();
    if camera_changed {
        editor.requested_camera = Some(camera);
    }
    ui.add_space(8.0);
    ui.separator();
    ui.label(theme::title("Map"));
    let mut map_changed = false;
    map_changed |= ui.checkbox(&mut editor.show_grid, "Grid overlay").changed();
    map_changed |= ui
        .checkbox(&mut editor.labels_visible, "Label objek")
        .changed();
    map_changed |= ui
        .checkbox(&mut editor.floor_plan, "Floor plan (indoor)")
        .changed();
    if map_changed {
        editor.touch();
    }
    ui.horizontal(|ui| {
        ui.label("Grid m");
        ui.add(
            egui::DragValue::new(&mut editor.grid_spacing)
                .speed(0.1)
                .range(0.1..=50.0),
        );
    });
    ui.add_space(8.0);
    ui.separator();
    ui.label(theme::title("Tampilan kamera"));
    ui.horizontal(|ui| {
        ui.label(theme::muted("Pusat"));
        ui.monospace(format!(
            "{:.5}, {:.5}",
            editor.map_state.center.latitude, editor.map_state.center.longitude
        ));
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::muted(&format!("Zoom {:.2}", editor.map_state.zoom)));
        ui.label(theme::muted(&format!(
            "Bearing {:.0}°",
            editor.map_state.bearing_degrees
        )));
        ui.label(theme::muted(&format!(
            "Pitch {:.0}°",
            editor.map_state.pitch_degrees
        )));
    });
    ui.label(theme::muted(match editor.base_layer {
        BaseLayer::ArcGisImagery => "Layer satelit",
        BaseLayer::OpenStreetMap => "Layer OSM",
    }));
    let view_saved = editor.view_is_saved();
    let status = match (view_saved, editor.view_saved_at.as_deref()) {
        (false, _) => "Belum disimpan".to_owned(),
        (true, Some(time)) => format!("Tersimpan {time}"),
        (true, None) => "Sesuai skenario tersimpan".to_owned(),
    };
    if view_saved {
        ui.colored_label(theme::color(DARK.success), status);
    } else {
        ui.label(theme::muted(&status));
    }
    ui.horizontal(|ui| {
        if can_admin {
            let response = ui
                .add_enabled(
                    editor.loaded && editor.scenario_id.is_some() && !editor.saving_view,
                    egui::Button::new("Simpan tampilan"),
                )
                .on_hover_text(
                    "Simpan pusat, zoom, bearing, pitch, dan base layer sebagai default skenario",
                );
            if response.clicked() {
                *action = EditorAction::SaveView;
            }
        } else {
            ui.label(theme::muted("Hanya admin dapat menyimpan tampilan."));
        }
        if editor.saving_view {
            ui.spinner();
        }
    });
    ui.add_space(8.0);
    ui.separator();
    ui.label(theme::title("Shortcut"));
    ui.label(theme::muted("Q/W/E/R tool · 1..5 create · Del hapus"));
    ui.label(theme::muted(
        "Ctrl+Z undo · Ctrl+D duplicate · Ctrl+S simpan",
    ));
    ui.label(theme::muted(
        "Klik kanan tahan orbit · tengah pan · wheel zoom",
    ));
    if editor.draft.is_active() {
        ui.separator();
        ui.colored_label(
            theme::color(DARK.warning),
            format!(
                "{} titik · Enter selesai · Esc batal",
                editor.draft.points().len()
            ),
        );
    }
    if editor.measure.start.is_some() {
        let distance = editor
            .measure
            .end
            .map(|end| distance_meters(editor.measure.start.unwrap_or(end), end))
            .unwrap_or(0.0);
        ui.separator();
        ui.monospace(format!("Ukur: {distance:.1} m"));
    }
}

fn draw_object_inspector(
    editor: &mut EditorState,
    key: ObjectKey,
    ui: &mut egui::Ui,
    texture_rows: &[EditorTextureRow],
    action: &mut EditorAction,
) {
    #[cfg(target_arch = "wasm32")]
    let _ = &action;
    let Some(mut object) = editor.object(key).cloned() else {
        return;
    };
    let mut changed = false;
    let mut undo = false;
    let mut delete = false;
    let mut duplicate = false;
    let mut remove_point = false;

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} {}",
                object.object_type().icon(),
                object.object_type().label()
            ))
            .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Hapus").clicked() {
                delete = true;
            }
            if ui.small_button("Duplikat").clicked() {
                duplicate = true;
            }
        });
    });
    ui.separator();

    let name = ui.text_edit_singleline(&mut object.name);
    undo |= name.gained_focus();
    changed |= name.changed();
    ui.checkbox(&mut object.visible, "Tampil");

    match &mut object.kind {
        EditorKind::Marker(marker) => {
            let selected_unit = editor
                .marker_models
                .get(&marker.unit_id)
                .map(|model| model.name.clone())
                .unwrap_or_else(|| "Pilih model aset".to_owned());
            let mut picked_unit: Option<String> = None;
            egui::ComboBox::from_id_salt("marker_unit")
                .selected_text(selected_unit)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    if editor.marker_models.is_empty() {
                        ui.label(theme::muted("Belum ada model di tab Assets."));
                    }
                    let mut entries = editor
                        .marker_models
                        .iter()
                        .map(|(unit_id, model)| (unit_id.clone(), model.name.clone()))
                        .collect::<Vec<_>>();
                    entries.sort_by(|a, b| a.1.cmp(&b.1));
                    for (unit_id, name) in entries {
                        if ui
                            .selectable_label(marker.unit_id == unit_id, name)
                            .clicked()
                        {
                            picked_unit = Some(unit_id);
                        }
                    }
                });
            if let Some(unit_id) = picked_unit {
                marker.unit = 0;
                marker.unit_id = unit_id;
                changed = true;
            }
            ui.horizontal(|ui| {
                ui.label("Kategori");
                changed |= ui.text_edit_singleline(&mut marker.kategori).changed();
            });
            ui.horizontal(|ui| {
                let rgb = hex_to_rgb(marker_color(&marker.kategori));
                ui.colored_label(egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]), "████");
                ui.label(theme::muted("warna kategori"));
            });
            ui.horizontal(|ui| {
                ui.label("Jumlah");
                changed |= ui
                    .add(egui::DragValue::new(&mut marker.jumlah).range(0..=10_000))
                    .changed();
                if spinner_button(ui, true, "+1").clicked() {
                    marker.jumlah = (marker.jumlah + 1).min(10_000);
                    changed = true;
                }
                if spinner_button(ui, false, "-1").clicked() {
                    marker.jumlah = (marker.jumlah - 1).max(0);
                    changed = true;
                }
            });
            changed |= ui
                .checkbox(&mut marker.obstacle, "Obstacle")
                .on_hover_text(
                    "Default aktif. Jika dimatikan, unit saat Play bisa melewati marker ini.",
                )
                .changed();
            ui.add_space(4.0);
            ui.label(theme::title("Transform"));
            let lon = drag_row(ui, "Lon", &mut marker.lon, 0.000_001, 0.000_01, 7, "");
            undo |= lon.gained_focus() || lon.drag_started();
            changed |= lon.changed();
            let lat = drag_row(ui, "Lat", &mut marker.lat, 0.000_001, 0.000_01, 7, "");
            undo |= lat.gained_focus() || lat.drag_started();
            changed |= lat.changed();
            let altitude = drag_row(ui, "Alt", &mut marker.altitude, 0.5, 0.5, 1, " m");
            undo |= altitude.gained_focus() || altitude.drag_started();
            changed |= altitude.changed();
            let rot_y = drag_row(ui, "Rot Y", &mut marker.rot_y, 1.0, 1.0, 1, "°");
            undo |= rot_y.gained_focus() || rot_y.drag_started();
            changed |= rot_y.changed();
            let rot_x = drag_row(ui, "Rot X", &mut marker.rot_x, 1.0, 1.0, 1, "°");
            undo |= rot_x.gained_focus() || rot_x.drag_started();
            changed |= rot_x.changed();
            let rot_z = drag_row(ui, "Rot Z", &mut marker.rot_z, 1.0, 1.0, 1, "°");
            undo |= rot_z.gained_focus() || rot_z.drag_started();
            changed |= rot_z.changed();
            let scale_x = scale_row(ui, "Scale X", &mut marker.scale_x);
            undo |= scale_x.gained_focus() || scale_x.drag_started();
            changed |= scale_x.changed();
            let scale_y = scale_row(ui, "Scale Y", &mut marker.scale_y);
            undo |= scale_y.gained_focus() || scale_y.drag_started();
            changed |= scale_y.changed();
            let scale_z = scale_row(ui, "Scale Z", &mut marker.scale_z);
            undo |= scale_z.gained_focus() || scale_z.drag_started();
            changed |= scale_z.changed();
            let static_asset = editor
                .marker_models
                .get(&marker.unit_id)
                .is_some_and(|model| !model.animated);
            if let Some(model) = editor.marker_models.get(&marker.unit_id) {
                ui.add_space(4.0);
                let label = if model.animated {
                    "Bisa pindah bebas (animasi)".to_owned()
                } else {
                    match model.move_mode {
                        twg_contracts::assets::AssetMoveMode::None => {
                            "Tidak bisa pindah saat Play".to_owned()
                        }
                        twg_contracts::assets::AssetMoveMode::Limited => {
                            "Bisa pindah terbatas (gizmo) saat Play".to_owned()
                        }
                        twg_contracts::assets::AssetMoveMode::Free => {
                            "Bisa pindah bebas saat Play".to_owned()
                        }
                    }
                };
                ui.label(theme::muted(&format!("Mode pindah: {label}")));
                if static_asset {
                    ui.label(theme::muted("Ubah mode pindah di menu Aset (detail aset)."));
                }
            }
            ui.add_space(4.0);
            ui.label("Keterangan");
            changed |= ui.text_edit_multiline(&mut marker.keterangan).changed();
        }
        EditorKind::Line(line) => {
            let editing = editor.point_edit_active(key);
            if ui
                .button(if editing {
                    "Selesai edit titik"
                } else {
                    "Edit titik"
                })
                .clicked()
            {
                if editing {
                    editor.end_point_edit();
                } else {
                    editor.begin_point_edit(key);
                }
            }
            if editing {
                ui.label(theme::muted(
                    "Klik titik di peta untuk memilih, klik dua kali segmen untuk menambah titik.",
                ));
                if let Some(edit) = editor.point_edit
                    && let Some(index) = edit.point
                    && index < line.points.len()
                {
                    ui.label(format!("Titik {} dari {}", index + 1, line.points.len()));
                    let mut altitude = line.altitude(index);
                    let altitude_row = drag_row(ui, "Alt titik", &mut altitude, 0.25, 0.5, 1, " m");
                    undo |= altitude_row.gained_focus() || altitude_row.drag_started();
                    if altitude_row.changed() {
                        if index < line.altitudes.len() {
                            line.altitudes[index] = altitude.max(0.0);
                        }
                        changed = true;
                    }
                    if ui.button("Hapus titik").clicked() {
                        remove_point = true;
                    }
                } else {
                    ui.label(theme::muted("Pilih titik di peta."));
                }
            }
            color_row(ui, "Warna", &mut line.color, &mut changed);
            let width = drag_row(ui, "Lebar", &mut line.width, 0.2, 0.5, 1, " m");
            undo |= width.drag_started();
            changed |= width.changed();
            let alpha = drag_row(ui, "Alpha", &mut line.alpha, 0.01, 0.05, 2, "");
            undo |= alpha.drag_started();
            changed |= alpha.changed();
            ui.label(theme::muted(&format!("{} titik", line.points.len())));
        }
        EditorKind::Building(building) => {
            ui.horizontal(|ui| {
                ui.label("Jenis");
                egui::ComboBox::from_id_salt("shape_kind")
                    .selected_text(building.shape.label())
                    .show_ui(ui, |ui| {
                        for shape in EditorShapeKind::ALL {
                            if ui
                                .selectable_label(building.shape == shape, shape.label())
                                .clicked()
                                && building.shape != shape
                            {
                                building.shape = shape;
                                changed = true;
                                undo = true;
                                if shape != EditorShapeKind::Custom && editor.point_edit_active(key)
                                {
                                    editor.end_point_edit();
                                }
                                if shape == EditorShapeKind::Custom && building.points.is_empty() {
                                    let center = GeoPoint::new(building.lon, building.lat);
                                    for (east, north) in
                                        [(-2.0, -2.0), (2.0, -2.0), (2.0, 2.0), (-2.0, 2.0)]
                                    {
                                        if let Ok(point) =
                                            GeoPoint::from_local_offset(center, east, north)
                                        {
                                            building.points.push([point.longitude, point.latitude]);
                                        }
                                    }
                                    building.height = 4.0;
                                }
                            }
                        }
                    });
            });
            if building.shape != EditorShapeKind::Ground {
                changed |= ui
                    .checkbox(&mut building.obstacle, "Obstacle")
                    .on_hover_text(
                        "Default aktif. Jika dimatikan, unit saat Play bisa melewati bentuk ini.",
                    )
                    .changed();
            }
            ui.add_space(4.0);
            ui.label(theme::title("Transform"));
            let lon = drag_row(ui, "Lon", &mut building.lon, 0.000_001, 0.000_01, 7, "");
            undo |= lon.gained_focus() || lon.drag_started();
            changed |= lon.changed();
            let lat = drag_row(ui, "Lat", &mut building.lat, 0.000_001, 0.000_01, 7, "");
            undo |= lat.gained_focus() || lat.drag_started();
            changed |= lat.changed();
            let altitude = drag_row(ui, "Alt", &mut building.altitude, 0.25, 0.5, 1, " m");
            undo |= altitude.gained_focus() || altitude.drag_started();
            changed |= altitude.changed();
            let rot_x = drag_row(ui, "Rot X", &mut building.rot_x, 1.0, 1.0, 1, "°");
            undo |= rot_x.gained_focus() || rot_x.drag_started();
            changed |= rot_x.changed();
            let rot_y = drag_row(ui, "Rot Y", &mut building.rot_y, 1.0, 1.0, 1, "°");
            undo |= rot_y.gained_focus() || rot_y.drag_started();
            changed |= rot_y.changed();
            let rot_z = drag_row(ui, "Rot Z", &mut building.rot_z, 1.0, 1.0, 1, "°");
            undo |= rot_z.gained_focus() || rot_z.drag_started();
            changed |= rot_z.changed();
            let scale_x = scale_row(ui, "Scale X", &mut building.scale_x);
            undo |= scale_x.gained_focus() || scale_x.drag_started();
            changed |= scale_x.changed();
            let scale_y = scale_row(ui, "Scale Y", &mut building.scale_y);
            undo |= scale_y.gained_focus() || scale_y.drag_started();
            changed |= scale_y.changed();
            let scale_z = scale_row(ui, "Scale Z", &mut building.scale_z);
            undo |= scale_z.gained_focus() || scale_z.drag_started();
            changed |= scale_z.changed();

            if building.shape == EditorShapeKind::Custom {
                ui.add_space(6.0);
                ui.separator();
                ui.label(theme::title("Bentuk custom"));
                let editing = editor.point_edit_active(key);
                if ui
                    .button(if editing {
                        "Selesai edit titik"
                    } else {
                        "Edit titik"
                    })
                    .clicked()
                {
                    if editing {
                        editor.end_point_edit();
                    } else {
                        editor.begin_point_edit(key);
                    }
                }
                if editing {
                    ui.label(theme::muted(
                        "Klik titik di peta untuk memilih, klik dua kali segmen untuk menambah titik.",
                    ));
                    if let Some(edit) = editor.point_edit
                        && let Some(index) = edit.point
                        && index < building.points.len()
                    {
                        ui.label(format!(
                            "Titik {} dari {}",
                            index + 1,
                            building.points.len()
                        ));
                        if ui.button("Hapus titik").clicked() {
                            remove_point = true;
                        }
                    } else {
                        ui.label(theme::muted("Pilih titik di peta."));
                    }
                }
                let height = drag_row(ui, "Tinggi", &mut building.height, 0.5, 0.5, 1, " m");
                undo |= height.drag_started();
                changed |= height.changed();
            }

            ui.add_space(6.0);
            ui.separator();
            ui.label(theme::title("Material"));
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(!building.use_texture, "Solid color")
                    .clicked()
                    && building.use_texture
                {
                    building.use_texture = false;
                    changed = true;
                }
                if ui
                    .selectable_label(building.use_texture, "Texture")
                    .clicked()
                    && !building.use_texture
                {
                    building.use_texture = true;
                    changed = true;
                }
            });
            if building.use_texture {
                editor.refresh_local_textures();
                let local_rows = editor.local_textures.clone();
                let selected_name =
                    texture_reference_label(building.texture.as_deref(), texture_rows, &local_rows);
                let mut picked_texture: Option<String> = None;
                egui::ComboBox::from_id_salt("shape_texture")
                    .selected_text(selected_name)
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        if texture_rows.is_empty() && local_rows.is_empty() {
                            ui.label(theme::muted("Belum ada tekstur."));
                        }
                        if !texture_rows.is_empty() {
                            ui.label(theme::muted("Aset server"));
                        }
                        for row in texture_rows {
                            if row.available {
                                let selected = building.texture.as_deref() == Some(row.id.as_str());
                                if ui.selectable_label(selected, &row.name).clicked() {
                                    picked_texture = Some(row.id.clone());
                                }
                            } else {
                                ui.label(theme::muted(&format!("{} (belum diunduh)", row.name)));
                            }
                        }
                        if !local_rows.is_empty() {
                            ui.separator();
                            ui.label(theme::muted("Tekstur lokal"));
                            for (reference, label) in &local_rows {
                                let selected =
                                    building.texture.as_deref() == Some(reference.as_str());
                                if ui.selectable_label(selected, label).clicked() {
                                    picked_texture = Some(reference.clone());
                                }
                            }
                        }
                    });
                if let Some(reference) = picked_texture {
                    building.texture = Some(reference);
                    changed = true;
                }

                if let Some(reference) = building.texture.clone() {
                    draw_texture_preview(ui, editor, &reference);
                } else {
                    ui.label(theme::muted(
                        "Pilih tekstur dari aset atau unggah dari komputer.",
                    ));
                }

                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("Unggah tekstur ke server...").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Gambar", &["png", "jpg", "jpeg", "webp"])
                        .set_title("Pilih tekstur")
                        .pick_file()
                {
                    *action = EditorAction::UploadTexture { key, path };
                }
                #[cfg(target_arch = "wasm32")]
                ui.label(theme::muted("Unggah tekstur tersedia di aplikasi desktop."));

                if building
                    .texture
                    .as_deref()
                    .is_some_and(|reference| editor.texture_source(reference).is_none())
                {
                    ui.label(theme::muted("Tekstur belum tersedia; memakai warna solid."));
                }
                ui.label(theme::muted(
                    "Tekstur diunggah ke server agar bisa dilihat di komputer lain.",
                ));
            } else {
                color_row(ui, "Warna", &mut building.color, &mut changed);
                ui.label(theme::muted("Klik kotak warna untuk membuka color picker."));
                color_picker_inline(ui, &mut building.color, &mut changed);
            }
            ui.label("Keterangan");
            changed |= ui.text_edit_multiline(&mut building.keterangan).changed();
        }
        EditorKind::Text(text) => {
            ui.label("Teks");
            changed |= ui.text_edit_multiline(&mut text.text).changed();
            color_row(ui, "Warna", &mut text.color, &mut changed);
            let size = drag_row(ui, "Font", &mut text.font_size, 0.5, 1.0, 1, " px");
            undo |= size.drag_started();
            changed |= size.changed();
            egui::ComboBox::from_id_salt("text_align")
                .selected_text(&text.align)
                .show_ui(ui, |ui| {
                    for align in ["left", "center", "right"] {
                        if ui.selectable_label(text.align == align, align).clicked() {
                            text.align = align.to_owned();
                            changed = true;
                        }
                    }
                });
            changed |= ui.checkbox(&mut text.wrap_text, "Wrap").changed();
            let lon = drag_row(ui, "Lon", &mut text.lon, 0.000_001, 0.000_01, 7, "");
            undo |= lon.drag_started();
            changed |= lon.changed();
            let lat = drag_row(ui, "Lat", &mut text.lat, 0.000_001, 0.000_01, 7, "");
            undo |= lat.gained_focus() || lat.drag_started();
            changed |= lat.changed();
            let altitude = drag_row(ui, "Alt", &mut text.altitude, 0.25, 0.5, 1, " m");
            undo |= altitude.gained_focus() || altitude.drag_started();
            changed |= altitude.changed();
        }
        EditorKind::Particle(particle) => {
            ui.horizontal(|ui| {
                ui.label("Jenis");
                egui::ComboBox::from_id_salt("particle_kind")
                    .selected_text(particle_kind_label(particle.kind))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for kind in twg_contracts::map::ParticleKind::ALL {
                            if ui
                                .selectable_label(particle.kind == kind, particle_kind_label(kind))
                                .clicked()
                            {
                                particle.kind = kind;
                                changed = true;
                            }
                        }
                    });
            });
            let lon = drag_row(ui, "Lon", &mut particle.lon, 0.000_001, 0.000_01, 7, "");
            undo |= lon.gained_focus() || lon.drag_started();
            changed |= lon.changed();
            let lat = drag_row(ui, "Lat", &mut particle.lat, 0.000_001, 0.000_01, 7, "");
            undo |= lat.gained_focus() || lat.drag_started();
            changed |= lat.changed();
            let altitude = drag_row(ui, "Alt", &mut particle.altitude, 0.25, 0.5, 1, " m");
            undo |= altitude.gained_focus() || altitude.drag_started();
            changed |= altitude.changed();

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Pengaturan efek").strong());
                if ui.small_button("Reset").clicked() {
                    particle.settings = ParticleSettings::default();
                    changed = true;
                }
            });
            let (size_label, density_label, height_label, spread_label, opacity_label) =
                match particle.kind {
                    twg_contracts::map::ParticleKind::Fire => (
                        "Ukuran api",
                        "Kerapatan",
                        "Tinggi api",
                        "Sebaran",
                        "Intensitas",
                    ),
                    twg_contracts::map::ParticleKind::Smoke => (
                        "Ukuran asap",
                        "Jumlah asap",
                        "Tinggi asap",
                        "Angin",
                        "Ketebalan",
                    ),
                    twg_contracts::map::ParticleKind::Water => (
                        "Ukuran tetesan",
                        "Jumlah air",
                        "Tinggi pancuran",
                        "Lebar pancuran",
                        "Kepekatan",
                    ),
                };
            let before = particle.settings;
            let size = drag_row(
                ui,
                size_label,
                &mut particle.settings.size,
                0.01,
                0.1,
                2,
                "×",
            );
            undo |= size.gained_focus() || size.drag_started();
            changed |= size.changed();
            let density = drag_row(
                ui,
                density_label,
                &mut particle.settings.density,
                0.01,
                0.1,
                2,
                "×",
            );
            undo |= density.gained_focus() || density.drag_started();
            changed |= density.changed();
            let height = drag_row(
                ui,
                height_label,
                &mut particle.settings.height,
                0.01,
                0.1,
                2,
                "×",
            );
            undo |= height.gained_focus() || height.drag_started();
            changed |= height.changed();
            let spread = drag_row(
                ui,
                spread_label,
                &mut particle.settings.spread,
                0.01,
                0.1,
                2,
                "×",
            );
            undo |= spread.gained_focus() || spread.drag_started();
            changed |= spread.changed();
            let speed = drag_row(
                ui,
                "Kecepatan",
                &mut particle.settings.speed,
                0.01,
                0.1,
                2,
                "×",
            );
            undo |= speed.gained_focus() || speed.drag_started();
            changed |= speed.changed();
            let opacity = drag_row(
                ui,
                opacity_label,
                &mut particle.settings.opacity,
                0.01,
                0.1,
                2,
                "×",
            );
            undo |= opacity.gained_focus() || opacity.drag_started();
            changed |= opacity.changed();
            let clamped = particle.settings.clamped();
            if clamped != particle.settings {
                particle.settings = clamped;
                changed = true;
            }
            if particle.settings != before {
                changed = true;
            }
            ui.label(theme::muted(
                "Pengaturan efek ikut tersimpan bersama skenario.",
            ));
            ui.label(theme::muted(
                "Efek besar dan rapat menambah beban render; turunkan Jumlah/Ketebalan bila FPS menurun.",
            ));
        }
    }

    if undo {
        editor.push_undo();
    }
    if changed {
        object.dirty = true;
        editor.dirty = true;
    }
    if let Some(slot) = editor.object_mut(key) {
        *slot = object;
    }
    if changed || undo {
        editor.touch();
    }
    if remove_point {
        editor.remove_point_edit_point();
    }
    if delete {
        editor.delete_selected();
    }
    if duplicate {
        editor.duplicate_selected();
    }
}

/// Small square spinner button. The arrow is painted directly because the
/// bundled fonts do not contain ▲/▼ glyphs.
fn spinner_button(
    ui: &mut egui::Ui,
    up: bool,
    hint: impl Into<egui::WidgetText>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 18.0), egui::Sense::click());
    let fill = if response.hovered() {
        theme::color(DARK.secondary)
    } else {
        theme::color(DARK.card)
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(4),
        fill,
        egui::Stroke::new(1.0, theme::color(DARK.border)),
        egui::StrokeKind::Inside,
    );
    let center = rect.center();
    let apex = egui::pos2(center.x, center.y + if up { -4.5 } else { 4.5 });
    let base_y = center.y + if up { 4.0 } else { -4.0 };
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            apex,
            egui::pos2(center.x - 4.2, base_y),
            egui::pos2(center.x + 4.2, base_y),
        ],
        theme::color(DARK.foreground),
        egui::Stroke::NONE,
    ));
    response.on_hover_text(hint)
}

fn drag_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    speed: f64,
    step: f64,
    decimals: usize,
    suffix: &str,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut response = ui.add(
            egui::DragValue::new(value)
                .speed(speed)
                .max_decimals(decimals)
                .suffix(suffix),
        );
        let mut changed = response.changed();
        if spinner_button(ui, true, format!("+{step}{suffix}")).clicked() {
            *value += step;
            changed = true;
        }
        if spinner_button(ui, false, format!("-{step}{suffix}")).clicked() {
            *value -= step;
            changed = true;
        }
        if changed {
            response.mark_changed();
        }
        response
    })
    .inner
}

/// Scale row: three decimals with a floor of `MIN_MARKER_SCALE` (0.001).
fn scale_row(ui: &mut egui::Ui, label: &str, value: &mut f64) -> egui::Response {
    let mut response = drag_row(ui, label, value, 0.01, 0.01, 3, "");
    if response.changed() && *value < MIN_MARKER_SCALE {
        *value = MIN_MARKER_SCALE;
        response.mark_changed();
    }
    response
}

fn color_row(ui: &mut egui::Ui, label: &str, color: &mut String, changed: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut rgb = hex_to_rgb(color);
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().interact_size = egui::vec2(48.0, 24.0);
                ui.color_edit_button_srgb(&mut rgb)
            })
            .inner
            .on_hover_text("Klik untuk membuka color picker");
        if response.changed() {
            *color = hex_from_rgb(rgb);
            *changed = true;
        }
        ui.monospace(color.clone());
    });
}

/// Always-visible picker for users who miss the small colour swatch popup.
fn color_picker_inline(ui: &mut egui::Ui, color: &mut String, changed: &mut bool) {
    egui::CollapsingHeader::new("Color picker")
        .id_salt("shape_color_picker")
        .default_open(false)
        .show(ui, |ui| {
            ui.spacing_mut().slider_width = 220.0;
            let rgb = hex_to_rgb(color);
            let mut srgba = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            if egui::color_picker::color_picker_color32(
                ui,
                &mut srgba,
                egui::color_picker::Alpha::Opaque,
            ) {
                *color = hex_from_rgb([srgba.r(), srgba.g(), srgba.b()]);
                *changed = true;
            }
        });
}

fn texture_reference_label(
    reference: Option<&str>,
    texture_rows: &[EditorTextureRow],
    local_rows: &[(String, String)],
) -> String {
    let Some(reference) = reference else {
        return "Pilih tekstur".to_owned();
    };
    if textures::is_uploaded_reference(reference) {
        let name = reference.rsplit('/').next().unwrap_or(reference);
        return format!("Server: {name}");
    }
    if let Some((_, label)) = local_rows.iter().find(|(key, _)| key == reference) {
        return format!("Lokal: {label}");
    }
    if let Some(row) = texture_rows.iter().find(|row| row.id == reference) {
        return row.name.clone();
    }
    "Pilih tekstur".to_owned()
}

fn draw_texture_preview(ui: &mut egui::Ui, editor: &mut EditorState, reference: &str) {
    let Some(texture) = editor.texture_preview(ui.ctx(), reference) else {
        let message = if editor.texture_downloads.contains(reference) {
            "Mengunduh pratinjau..."
        } else {
            "Pratinjau belum tersedia."
        };
        ui.label(theme::muted(message));
        return;
    };
    let size = texture.size_vec2();
    let max_width = ui.available_width().clamp(64.0, 220.0);
    let scale = (max_width / size.x).min(140.0 / size.y).min(1.0);
    let desired = size * scale;
    egui::Frame::new()
        .fill(theme::color(DARK.card))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(4))
        .show(ui, |ui| {
            ui.add(egui::Image::new(&texture).fit_to_exact_size(desired));
        });
}

fn hex_to_rgb(hex: &str) -> [u8; 3] {
    let color = parse_hex_color(hex);
    [
        (color[0] * 255.0).round() as u8,
        (color[1] * 255.0).round() as u8,
        (color[2] * 255.0).round() as u8,
    ]
}

fn hex_from_rgb(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

fn draw_viewport(root: &mut egui::Ui, editor: &mut EditorState) {
    let context = root.ctx().clone();
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(root, |ui| {
            let rect = ui.max_rect();
            editor.viewport_rect = rect;
            let painter = ui.painter().clone();

            if editor.loading {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Memuat skenario...",
                    egui::FontId::proportional(16.0),
                    theme::color(DARK.foreground),
                );
                return;
            }

            if !editor.preview {
                draw_selection_overlay(&painter, editor);
                draw_object_pins(&painter, editor);
                draw_draft(&painter, editor);
                draw_measure(&painter, editor);
                draw_marquee(&painter, editor);
                draw_labels(&painter, editor);
                draw_point_edit_handles(&painter, editor);
            } else {
                draw_play_overlay(&painter, editor);
            }
            draw_text_overlays(&painter, editor);
            draw_compass(&painter, rect, editor);
            draw_scale_bar(&painter, rect, editor);
            draw_crosshair(&painter, rect);
            painter.text(
                rect.right_bottom() + egui::vec2(-10.0, -8.0),
                egui::Align2::RIGHT_BOTTOM,
                editor.base_layer.attribution(),
                egui::FontId::proportional(10.0),
                egui::Color32::from_white_alpha(190),
            );

            egui::Area::new(egui::Id::new("editor_view_controls"))
                // Keep the zoom/bearing panel clear of the Inspector on the
                // right edge of the viewport.
                .fixed_pos(egui::pos2(rect.right() - 156.0, rect.top() + 12.0))
                .order(egui::Order::Foreground)
                .show(&context, |ui| {
                    egui::Frame::new()
                        .fill(theme::color(DARK.card))
                        .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.small_button("+").clicked() {
                                    editor.requested_zoom = Some(0.5);
                                }
                                if ui.small_button("−").clicked() {
                                    editor.requested_zoom = Some(-0.5);
                                }
                                if ui.small_button("N").clicked() {
                                    editor.requested_north = true;
                                }
                            });
                            ui.monospace(format!("{:.2}", editor.map_state.zoom));
                            ui.monospace(format!(
                                "{:.1}° / {:.1}°",
                                editor.map_state.bearing_degrees, editor.map_state.pitch_degrees
                            ));
                            let fps = editor.fps.max(0.0);
                            let frame_ms = if fps > 0.1 { 1000.0 / fps } else { 0.0 };
                            let fps_color = if fps <= 0.1 {
                                theme::color(DARK.foreground)
                            } else if fps >= 50.0 {
                                theme::color(DARK.success)
                            } else if fps >= 30.0 {
                                theme::color(DARK.warning)
                            } else {
                                theme::color(DARK.destructive)
                            };
                            let fps_text = if fps > 0.1 {
                                format!("{fps:.0} FPS · {frame_ms:.1} ms")
                            } else {
                                "-- FPS".to_owned()
                            };
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(fps_text).monospace().color(fps_color),
                                )
                                .selectable(false),
                            );
                        });
                });
        });
}

/// RTS selection feedback: a green ring (plus corner brackets) under every
/// selected animated unit, a thin hover ring, and the marquee rectangle.
fn draw_play_overlay(painter: &egui::Painter, editor: &EditorState) {
    if let Some((start, current)) = editor.play.marquee {
        let rect = egui::Rect::from_two_pos(start, current);
        painter.rect_filled(
            rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(80, 200, 120, 26),
        );
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.2, theme::color(DARK.success)),
            egui::StrokeKind::Inside,
        );
    }

    let selected_stroke = egui::Stroke::new(2.4, theme::color(DARK.success));
    let hover_stroke = egui::Stroke::new(1.4, egui::Color32::from_white_alpha(170));
    for (key, ring) in &editor.play_rings {
        let stroke = if editor.selected.contains(key) {
            selected_stroke
        } else if editor.hover_key == Some(*key) {
            hover_stroke
        } else {
            continue;
        };
        painter.add(egui::Shape::closed_line(ring.clone(), stroke));
    }
}

fn draw_selection_overlay(painter: &egui::Painter, editor: &EditorState) {
    let selected_stroke = egui::Stroke::new(2.4, theme::color(DARK.warning));
    let hover_stroke = egui::Stroke::new(1.4, theme::color(DARK.foreground));
    for (key, _kind, points, selected) in &editor.projected {
        let stroke = if *selected {
            selected_stroke
        } else if editor.hover_key == Some(*key) {
            hover_stroke
        } else {
            continue;
        };
        match _kind {
            ObjectType::Marker | ObjectType::Text => {
                if let Some(center) = points.first() {
                    painter.circle_stroke(*center, 15.0, stroke);
                }
            }
            // Particle effects are their own visual; a crosshair keeps the
            // selection feedback without drawing another circle over them.
            ObjectType::Particle => {
                if let Some(center) = points.first() {
                    let gap = 4.0;
                    let tick = 7.0;
                    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                        painter.line_segment(
                            [
                                *center + egui::vec2(dx * gap, dy * gap),
                                *center + egui::vec2(dx * (gap + tick), dy * (gap + tick)),
                            ],
                            stroke,
                        );
                    }
                }
            }
            ObjectType::Line | ObjectType::Building => {
                painter.add(egui::Shape::line(points.clone(), stroke));
            }
        }
    }
}

fn draw_object_pins(painter: &egui::Painter, editor: &EditorState) {
    for (key, kind, points, _) in &editor.projected {
        let Some(position) = points.first() else {
            continue;
        };
        let color = editor
            .object(*key)
            .map(|object| parse_hex_color(&object.kind.color()))
            .unwrap_or([0.85, 0.85, 0.85, 1.0]);
        let color = egui::Color32::from_rgb(
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
        );
        let outline = egui::Stroke::new(1.2, egui::Color32::from_black_alpha(220));
        match kind {
            ObjectType::Marker => {
                painter.circle_filled(*position, 5.5, color);
                painter.circle_stroke(*position, 5.5, outline);
            }
            ObjectType::Text => {
                let rect = egui::Rect::from_center_size(*position, egui::vec2(10.0, 10.0));
                painter.rect_filled(rect, 2.0, color);
                painter.rect_stroke(rect, 2.0, outline, egui::StrokeKind::Inside);
            }
            ObjectType::Particle => {
                // When the editor preview is off the effect is hidden; leave a
                // small diamond so the particle can still be found and picked.
                if !editor.preview && !editor.particle_preview {
                    let radius = 5.0;
                    painter.add(egui::Shape::closed_line(
                        vec![
                            *position + egui::vec2(0.0, -radius),
                            *position + egui::vec2(radius, 0.0),
                            *position + egui::vec2(0.0, radius),
                            *position + egui::vec2(-radius, 0.0),
                        ],
                        egui::Stroke::new(1.4, color),
                    ));
                }
            }
            ObjectType::Line | ObjectType::Building => {}
        }
    }
}

fn draw_labels(painter: &egui::Painter, editor: &EditorState) {
    let font = egui::FontId::proportional(12.0);
    for (position, label, color) in &editor.labels {
        let text_position = *position + egui::vec2(0.0, -18.0);
        painter.text(
            text_position + egui::vec2(1.0, 1.0),
            egui::Align2::CENTER_CENTER,
            label,
            font.clone(),
            egui::Color32::from_black_alpha(200),
        );
        painter.text(
            text_position,
            egui::Align2::CENTER_CENTER,
            label,
            font.clone(),
            egui::Color32::from_rgb(
                (color[0] * 255.0) as u8,
                (color[1] * 255.0) as u8,
                (color[2] * 255.0) as u8,
            ),
        );
    }
}

fn draw_text_overlays(painter: &egui::Painter, editor: &EditorState) {
    for (position, label, color, size) in &editor.text_overlays {
        let font = egui::FontId::proportional((*size).clamp(6.0, 512.0));
        painter.text(
            *position + egui::vec2(1.0, 1.0),
            egui::Align2::CENTER_CENTER,
            label,
            font.clone(),
            egui::Color32::from_black_alpha(210),
        );
        painter.text(
            *position,
            egui::Align2::CENTER_CENTER,
            label,
            font,
            egui::Color32::from_rgb(
                (color[0] * 255.0) as u8,
                (color[1] * 255.0) as u8,
                (color[2] * 255.0) as u8,
            ),
        );
    }
}

fn draw_point_edit_handles(painter: &egui::Painter, editor: &EditorState) {
    let Some(edit) = editor.point_edit else {
        return;
    };
    let Some((_, _, points, _)) = editor
        .projected
        .iter()
        .find(|(key, _, _, _)| *key == edit.key)
    else {
        return;
    };
    for (index, position) in points.iter().enumerate() {
        let selected = edit.point == Some(index);
        let radius = if selected { 6.5 } else { 4.5 };
        let fill = if selected {
            theme::color(DARK.warning)
        } else {
            egui::Color32::WHITE
        };
        painter.circle_filled(*position, radius, fill);
        painter.circle_stroke(
            *position,
            radius,
            egui::Stroke::new(1.5, egui::Color32::from_black_alpha(180)),
        );
        painter.text(
            *position + egui::vec2(0.0, -12.0),
            egui::Align2::CENTER_BOTTOM,
            format!("{}", index + 1),
            egui::FontId::monospace(10.0),
            egui::Color32::WHITE,
        );
    }
}

fn draw_draft(painter: &egui::Painter, editor: &EditorState) {
    if !editor.draft.is_active() {
        return;
    }
    let mut points = editor.draft_screen.clone();
    if let Some(cursor) = painter.ctx().pointer_hover_pos() {
        if let Some(last) = points.last().copied() {
            painter.line_segment(
                [last, cursor],
                egui::Stroke::new(1.4, theme::color(DARK.warning)),
            );
        }
        points.push(cursor);
    }
    if points.len() >= 2 {
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(2.0, theme::color(DARK.warning)),
        ));
    }
}

fn draw_measure(painter: &egui::Painter, editor: &EditorState) {
    if editor.measure_screen.len() < 2 {
        return;
    }
    let start = editor.measure_screen[0];
    let end = editor.measure_screen[1];
    painter.line_segment(
        [start, end],
        egui::Stroke::new(2.0, theme::color(DARK.success)),
    );
    for point in [start, end] {
        painter.circle_filled(point, 3.0, theme::color(DARK.success));
    }
    if let (Some(a), Some(b)) = (editor.measure.start, editor.measure.end) {
        painter.text(
            start.lerp(end, 0.5) + egui::vec2(0.0, -12.0),
            egui::Align2::CENTER_CENTER,
            format!("{:.1} m", distance_meters(a, b)),
            egui::FontId::monospace(12.0),
            theme::color(DARK.foreground),
        );
    }
}

fn draw_marquee(painter: &egui::Painter, editor: &EditorState) {
    if let super::model::DragState::Marquee { start, current } = &editor.drag {
        let rect = egui::Rect::from_two_pos(*start, *current);
        painter.rect_filled(
            rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(80, 160, 255, 30),
        );
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.2, theme::color(DARK.ring)),
            egui::StrokeKind::Inside,
        );
    }
}

fn draw_compass(painter: &egui::Painter, rect: egui::Rect, editor: &EditorState) {
    let center = egui::pos2(rect.right() - 52.0, rect.top() + 52.0);
    painter.circle_filled(center, 26.0, egui::Color32::from_black_alpha(150));
    painter.circle_stroke(
        center,
        26.0,
        egui::Stroke::new(1.0, theme::color(DARK.border)),
    );
    let bearing = editor.map_state.bearing_degrees.to_radians();
    let direction = egui::vec2(-bearing.sin(), -bearing.cos());
    painter.line_segment(
        [center, center + direction * 20.0],
        egui::Stroke::new(2.0, theme::color(DARK.destructive)),
    );
    painter.text(
        center + direction * 26.0,
        egui::Align2::CENTER_CENTER,
        "N",
        egui::FontId::proportional(11.0),
        theme::color(DARK.foreground),
    );
}

fn draw_scale_bar(painter: &egui::Painter, rect: egui::Rect, editor: &EditorState) {
    let meters_per_pixel = editor.map_state.meters_per_pixel() as f32;
    if meters_per_pixel <= 0.0 {
        return;
    }
    let target = meters_per_pixel * 120.0;
    let nice = nice_distance(target);
    let width = (nice / meters_per_pixel).clamp(30.0, 260.0);
    let start = egui::pos2(rect.left() + 18.0, rect.bottom() - 22.0);
    let end = start + egui::vec2(width, 0.0);
    painter.line_segment(
        [start, end],
        egui::Stroke::new(2.0, theme::color(DARK.foreground)),
    );
    painter.line_segment(
        [start - egui::vec2(0.0, 5.0), start + egui::vec2(0.0, 5.0)],
        egui::Stroke::new(2.0, theme::color(DARK.foreground)),
    );
    painter.line_segment(
        [end - egui::vec2(0.0, 5.0), end + egui::vec2(0.0, 5.0)],
        egui::Stroke::new(2.0, theme::color(DARK.foreground)),
    );
    painter.text(
        start + egui::vec2(0.0, -10.0),
        egui::Align2::LEFT_CENTER,
        format!("{nice:.0} m"),
        egui::FontId::monospace(11.0),
        theme::color(DARK.foreground),
    );
}

fn nice_distance(value: f32) -> f32 {
    let exponent = value.max(1.0).log10().floor();
    let base = 10f32.powf(exponent);
    for factor in [1.0, 2.0, 5.0, 10.0] {
        if value <= base * factor {
            return base * factor;
        }
    }
    base * 10.0
}

fn draw_crosshair(painter: &egui::Painter, rect: egui::Rect) {
    let center = rect.center();
    let stroke = egui::Stroke::new(
        1.0,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 120),
    );
    painter.line_segment(
        [
            center - egui::vec2(10.0, 0.0),
            center + egui::vec2(10.0, 0.0),
        ],
        stroke,
    );
    painter.line_segment(
        [
            center - egui::vec2(0.0, 10.0),
            center + egui::vec2(0.0, 10.0),
        ],
        stroke,
    );
    painter.circle_stroke(center, 3.0, stroke);
}

fn draw_context_menu(context: &egui::Context, editor: &mut EditorState) {
    let Some((position, key)) = editor.context_menu else {
        return;
    };
    let mut close = false;
    egui::Area::new(egui::Id::new("editor_context_menu"))
        .fixed_pos(position)
        .order(egui::Order::Foreground)
        .show(context, |ui| {
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    ui.set_min_width(150.0);
                    if ui.button("Duplicate").clicked() {
                        editor.select_only(key);
                        editor.duplicate_selected();
                        close = true;
                    }
                    if ui.button("Hapus").clicked() {
                        editor.select_only(key);
                        editor.delete_selected();
                        close = true;
                    }
                    if ui.button("Fokus").clicked() {
                        if let Some(point) = editor.object(key).and_then(|object| object.point()) {
                            let mut target = editor.map_state;
                            target.center = point;
                            target.zoom = target.zoom.max(18.0);
                            editor.requested_flight = Some(target);
                        }
                        close = true;
                    }
                    if ui.button("Batal").clicked() {
                        close = true;
                    }
                });
        });
    if close {
        editor.context_menu = None;
    }
}

fn draw_dialogs(context: &egui::Context, editor: &mut EditorState, action: &mut EditorAction) {
    if editor.confirm_close {
        egui::Window::new("Perubahan belum disimpan")
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                ui.label("Skenario ini memiliki perubahan yang belum disimpan.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Simpan & tutup").clicked() {
                        editor.confirm_close = false;
                        *action = EditorAction::SaveAndClose;
                    }
                    if ui.button("Buang & tutup").clicked() {
                        editor.confirm_close = false;
                        *action = EditorAction::Close;
                    }
                    if ui.button("Batal").clicked() {
                        editor.confirm_close = false;
                    }
                });
            });
    }
    if editor.confirm_reload {
        egui::Window::new("Muat ulang skenario")
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                ui.label("Perubahan yang belum disimpan akan hilang.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Simpan & muat ulang").clicked() {
                        editor.confirm_reload = false;
                        *action = EditorAction::SaveAndReload;
                    }
                    if ui.button("Buang & muat ulang").clicked() {
                        editor.confirm_reload = false;
                        *action = EditorAction::Reload;
                    }
                    if ui.button("Batal").clicked() {
                        editor.confirm_reload = false;
                    }
                });
            });
    }
}
