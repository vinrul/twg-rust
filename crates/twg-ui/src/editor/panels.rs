use bevy_egui::egui;
use twg_client::design::{DARK, FONT_SM, FONT_XS};
use twg_contracts::assets::{AssetCategory, AssetModelKind, AssetMoveMode, AssetSourceSlot};
use twg_map::{BaseLayer, GeoPoint};
use twg_render::{EditorModelShading, EditorShapeKind, SkyMode};

use crate::state::ClientUi;
use crate::theme;
use crate::widgets;

use super::input::distance_meters;
use super::model::{
    AssetMarkerDraft, EditorAssetRow, EditorKind, EditorState, EditorTextureRow, EditorTool,
    MIN_MARKER_SCALE, ObjectKey, ObjectType, ParticleSettings, PlaybackState, marker_color,
    parse_hex_color, particle_kind_label,
};

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
}

pub fn draw(context: &egui::Context, client: &mut ClientUi) {
    client.ensure_ws();
    client.editor.base_url.clone_from(&client.base_url);
    client.sync_editor_marker_models();
    let asset_rows = client.editor_asset_rows();
    let import_rows = client.editor_import_rows();
    let texture_rows = client.editor_texture_rows();
    let can_admin = client.role().is_admin_like();
    let mut action = EditorAction::None;
    {
        let editor = &mut client.editor;
        root_ui(context, |ui| {
            if editor.preview {
                draw_preview_bar(ui, editor, can_admin, &mut action);
                draw_animation_play_panel(ui, editor);
            } else {
                draw_menu(ui, editor, &mut action);
                draw_toolbar(ui, editor, &mut action);
                draw_status(ui, editor);
                draw_console_panel(ui, editor);
                draw_scene_panel(ui, editor, &asset_rows, &import_rows);
                draw_inspector(ui, editor, can_admin, &texture_rows, &mut action);
            }
            draw_viewport(ui, editor);
        });
        draw_context_menu(context, editor);
        draw_dialogs(context, editor, &mut action);
        draw_import_dialog(context, editor);
        draw_import_delete_dialog(context, editor);
        draw_delete_dialog(context, editor);
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
    if client.editor.requested_import_upload {
        client.editor.requested_import_upload = false;
        client.begin_import_upload();
    }
    if let Some(asset_id) = client.editor.requested_import_delete.take() {
        client.begin_import_delete(asset_id);
    }
    if client.editor.requested_import_refresh {
        client.editor.requested_import_refresh = false;
        client.refresh_imports();
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

fn draw_preview_bar(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    can_admin: bool,
    action: &mut EditorAction,
) {
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
                    let mut sky = editor.sky;
                    egui::ComboBox::from_id_salt("play_sky_mode")
                        .selected_text(sky_mode_label(sky))
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            for option in SkyMode::ALL {
                                ui.selectable_value(&mut sky, option, sky_mode_label(option));
                            }
                        });
                    let changed = sky != editor.sky;
                    editor.sky = sky;
                    if changed && can_admin && editor.loaded && editor.scenario_id.is_some() {
                        *action = EditorAction::SaveView;
                    }
                    ui.label("Langit");
                });
            });
        });
}

enum RouteCommand {
    Play,
    Pause,
    Stop,
}

fn draw_animation_play_panel(ui: &mut egui::Ui, editor: &mut EditorState) {
    egui::Panel::left("editor_animation_play")
        .resizable(true)
        .default_size(230.0)
        .min_size(190.0)
        .max_size(340.0)
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .inner_margin(egui::Margin::same(10)),
        )
        .show(ui, |ui| {
            ui.label(theme::title("Animasi"));
            ui.label(theme::muted("Kontrol rute selama Play."));
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("editor_play_animations")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if editor.play.animations.is_empty() {
                        ui.label(theme::muted(
                            "Belum ada animasi rute. Buat rute di tab Animasi saat mengedit skenario.",
                        ));
                        return;
                    }
                    let mut command: Option<(usize, RouteCommand)> = None;
                    for (index, animation) in editor.play.animations.iter().enumerate() {
                        ui.label(egui::RichText::new(&animation.name).strong());
                        let status = match animation.state {
                            PlaybackState::Stopped => "Berhenti",
                            PlaybackState::Paused => "Jeda",
                            PlaybackState::Playing if animation.is_finished() => "Selesai",
                            PlaybackState::Playing => "Berjalan",
                        };
                        ui.label(theme::muted(status));
                        if animation.marker_key.is_none() {
                            ui.label(theme::danger(
                                "Marker rute tidak ditemukan atau tidak bisa bergerak.",
                            ));
                            ui.separator();
                            continue;
                        }
                        let running = animation.state == PlaybackState::Playing
                            && !animation.is_finished();
                        let resume = animation.state == PlaybackState::Paused;
                        ui.horizontal_wrapped(|ui| {
                            if !running
                                && ui.button(if resume { "Resume" } else { "Play" }).clicked()
                            {
                                command = Some((index, RouteCommand::Play));
                            }
                            if running && ui.button("Pause").clicked() {
                                command = Some((index, RouteCommand::Pause));
                            }
                            if animation.state != PlaybackState::Stopped
                                && ui.button("Stop").clicked()
                            {
                                command = Some((index, RouteCommand::Stop));
                            }
                        });
                        ui.separator();
                    }
                    if let Some((index, command)) = command {
                        match command {
                            RouteCommand::Play => editor.play_route_animation(index),
                            RouteCommand::Pause => editor.pause_route_animation(index),
                            RouteCommand::Stop => editor.stop_route_animation(index),
                        }
                    }
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
                        editor.request_delete_selected();
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
    ui.add(
        egui::TextEdit::singleline(&mut editor.asset_search)
            .hint_text("Cari aset...")
            .desired_width(f32::INFINITY)
            .margin(egui::Margin::symmetric(8, 5)),
    );
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
    } else {
        ui.add_space(2.0);
    }

    let needle = editor.asset_search.trim().to_lowercase();
    let mut groups: std::collections::BTreeMap<&str, Vec<&EditorAssetRow>> =
        std::collections::BTreeMap::new();
    for row in asset_rows {
        if !needle.is_empty()
            && !row.name.to_lowercase().contains(&needle)
            && !row.group.to_lowercase().contains(&needle)
        {
            continue;
        }
        groups.entry(row.group.as_str()).or_default().push(row);
    }
    if groups.is_empty() {
        ui.label(theme::muted("Tidak ada aset yang cocok."));
        return;
    }
    for rows in groups.values_mut() {
        rows.sort_by_key(|row| row.name.to_lowercase());
    }
    ui.label(theme::muted(if editor.asset_drag.is_some() {
        "Lepas di peta untuk menambah marker"
    } else {
        "Tarik nama aset ke peta atau klik tombol +"
    }));
    egui::ScrollArea::vertical()
        .id_salt("editor_asset_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (group, rows) in groups {
                draw_asset_group(ui, editor, group, &rows, &needle);
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

fn draw_asset_group(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    group: &str,
    rows: &[&EditorAssetRow],
    needle: &str,
) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
    let open = !needle.is_empty() || !editor.asset_groups_closed.contains(group);
    let painter = ui.painter().clone();
    if response.hovered() {
        painter.rect_filled(rect, 4.0, theme::color(DARK.secondary));
    }
    let center = rect.center().y;
    let muted = theme::color(DARK.muted_foreground);
    let foreground = theme::color(DARK.foreground);
    let arrow_center = egui::pos2(rect.left() + 8.0, center);
    let points = if open {
        vec![
            arrow_center + egui::vec2(-4.0, -2.5),
            arrow_center + egui::vec2(4.0, -2.5),
            arrow_center + egui::vec2(0.0, 2.5),
        ]
    } else {
        vec![
            arrow_center + egui::vec2(-2.5, -4.0),
            arrow_center + egui::vec2(2.5, 0.0),
            arrow_center + egui::vec2(-2.5, 4.0),
        ]
    };
    painter.add(egui::Shape::convex_polygon(
        points,
        muted,
        egui::Stroke::NONE,
    ));
    painter.text(
        egui::pos2(rect.left() + 20.0, center),
        egui::Align2::LEFT_CENTER,
        "M",
        egui::FontId::proportional(FONT_SM),
        muted,
    );
    let galley = painter.layout_no_wrap(
        group.to_owned(),
        egui::FontId::proportional(FONT_SM),
        foreground,
    );
    let group_x = rect.left() + 34.0;
    painter.galley(
        egui::pos2(group_x, center - galley.size().y * 0.5),
        galley.clone(),
        foreground,
    );
    painter.text(
        egui::pos2(group_x + galley.size().x + 6.0, center),
        egui::Align2::LEFT_CENTER,
        format!("({})", rows.len()),
        egui::FontId::proportional(FONT_XS),
        muted,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() && needle.is_empty() {
        if open {
            editor.asset_groups_closed.insert(group.to_owned());
        } else {
            editor.asset_groups_closed.remove(group);
        }
    }
    if open {
        for row in rows {
            draw_asset_row(ui, editor, row);
        }
    }
}

fn draw_asset_row(ui: &mut egui::Ui, editor: &mut EditorState, row: &EditorAssetRow) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
    let plus_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 15.0, rect.center().y),
        egui::vec2(22.0, 22.0),
    );
    let row_rect =
        egui::Rect::from_min_max(rect.min, egui::pos2(plus_rect.left() - 4.0, rect.bottom()));
    let response = ui.interact(
        row_rect,
        ui.id().with(("asset-row", row.unit_id.as_str())),
        egui::Sense::click_and_drag(),
    );
    let plus = ui.interact(
        plus_rect,
        ui.id().with(("asset-add", row.unit_id.as_str())),
        egui::Sense::click(),
    );
    let painter = ui.painter().clone();
    if response.hovered() || plus.hovered() {
        painter.rect_filled(rect, 4.0, theme::color(DARK.secondary));
    }
    painter.vline(
        rect.left() + 10.0,
        rect.y_range(),
        egui::Stroke::new(1.0, theme::color(DARK.border)),
    );
    let name_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 24.0, rect.top()),
        egui::pos2(plus_rect.left() - 6.0, rect.bottom()),
    );
    painter.with_clip_rect(name_rect).text(
        egui::pos2(rect.left() + 24.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &row.name,
        egui::FontId::proportional(FONT_SM),
        if row.cached {
            theme::color(DARK.foreground)
        } else {
            theme::color(DARK.muted_foreground)
        },
    );
    let plus_color = if row.cached {
        theme::color(DARK.foreground)
    } else {
        theme::color(DARK.muted_foreground)
    };
    if plus.hovered() && row.cached {
        painter.rect_filled(plus_rect, 4.0, theme::color(DARK.border));
    }
    let stroke = egui::Stroke::new(1.4, plus_color);
    let center = plus_rect.center();
    painter.line_segment(
        [
            center + egui::vec2(-4.0, 0.0),
            center + egui::vec2(4.0, 0.0),
        ],
        stroke,
    );
    painter.line_segment(
        [
            center + egui::vec2(0.0, -4.0),
            center + egui::vec2(0.0, 4.0),
        ],
        stroke,
    );
    if plus.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if plus.clicked() && row.cached {
        add_asset_to_map(editor, row);
    }
    if row.cached && response.drag_started() {
        editor.asset_drag = Some(AssetMarkerDraft {
            unit_id: row.unit_id.clone(),
            name: row.name.clone(),
        });
    }
    if response.hovered() && editor.asset_drag.is_none() {
        response
            .clone()
            .on_hover_ui(|ui| draw_asset_tooltip(ui, editor, row));
    }
}

fn add_asset_to_map(editor: &mut EditorState, row: &EditorAssetRow) {
    let draft = AssetMarkerDraft {
        unit_id: row.unit_id.clone(),
        name: row.name.clone(),
    };
    let point = editor.snap_point(editor.map_state.center);
    editor.add_asset_marker(&draft, point);
}

fn draw_asset_tooltip(ui: &mut egui::Ui, editor: &mut EditorState, row: &EditorAssetRow) {
    ui.set_max_width(250.0);
    ui.label(egui::RichText::new(&row.name).size(FONT_SM).strong());
    ui.label(theme::muted(&format!(
        "Sumber: {}",
        asset_source_label(row)
    )));
    ui.label(theme::muted(&format!("Kategori: {}", row.group)));
    ui.label(theme::muted(&format!("Jenis: {}", row.kind_label)));
    if row.is_model {
        ui.label(theme::muted(&format!(
            "Mode pindah: {}",
            row.move_mode_label
        )));
    }
    ui.label(theme::muted(&format!("Versi: {}", row.version)));
    ui.label(theme::muted(&format!("Ukuran: {}", row.size_label)));
    ui.label(theme::muted(if row.cached {
        "Status: siap offline"
    } else {
        "Status: belum diunduh"
    }));
    if let Some(path) = &row.preview {
        if let Some(texture) = editor.stored_preview(ui.ctx(), path) {
            ui.add_space(4.0);
            ui.add(egui::Image::new(&texture).fit_to_exact_size(egui::vec2(180.0, 180.0)));
        }
    } else {
        ui.label(theme::muted("Belum ada gambar preview."));
    }
}

fn asset_source_label(row: &EditorAssetRow) -> &'static str {
    if row.imported {
        "Import operasi ini"
    } else {
        "Katalog admin"
    }
}

fn draw_import_tab(ui: &mut egui::Ui, editor: &mut EditorState, import_rows: &[EditorAssetRow]) {
    ui.add_space(2.0);
    if ui
        .add(egui::Button::new("Import aset").min_size(egui::vec2(ui.available_width(), 28.0)))
        .clicked()
    {
        editor.import_error = None;
        editor.import_dialog_open = true;
    }
    ui.label(theme::muted(
        "Unggah GLB/glTF atau gambar. Aset menjadi milik operasi skenario ini, \
         bisa dipakai di semua skenarionya, dan ikut tersinkron ke komputer lain.",
    ));

    ui.add_space(8.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(theme::title("Aset impor"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Muat ulang").clicked() {
                editor.requested_import_refresh = true;
            }
        });
    });
    if import_rows.is_empty() {
        ui.label(theme::muted("Belum ada aset impor di operasi ini."));
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("editor_import_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for row in import_rows {
                draw_import_row(ui, editor, row);
            }
        });
}

fn draw_animation_create_tab(ui: &mut egui::Ui, editor: &mut EditorState) {
    ui.add_space(2.0);
    ui.label(theme::title("Buat animasi"));
    ui.label(theme::muted(
        "Pilih jenis animasi lalu gambar di peta. Animasi yang sudah dibuat \
         tampil di Scene → Daftar Anim.",
    ));
    ui.add_space(6.0);

    egui::CollapsingHeader::new("Rute")
        .id_salt("editor_animation_create")
        .default_open(true)
        .show(ui, |ui| {
            ui.label(theme::muted(
                "Menggerakkan satu marker mengikuti garis dengan kecepatan tetap.",
            ));
            ui.add_space(4.0);
            if ui
                .add_enabled(
                    editor.loaded,
                    egui::Button::new("+ Single marker rute")
                        .min_size(egui::vec2(ui.available_width(), 26.0)),
                )
                .on_hover_text("Gambar garis rute di peta")
                .clicked()
            {
                editor.begin_route_draw();
            }
        });
}

fn draw_animation_list_tab(ui: &mut egui::Ui, editor: &mut EditorState) {
    let routes = editor
        .objects
        .iter()
        .filter(|object| object.is_animation_route())
        .map(|object| {
            let marker_id = match &object.kind {
                EditorKind::Line(line) => line.animation.as_ref().and_then(|route| route.marker_id),
                _ => None,
            };
            (
                object.key,
                object.name.clone(),
                marker_id,
                object.visible,
                editor.locked.contains(&object.key),
            )
        })
        .collect::<Vec<_>>();

    ui.add_space(2.0);
    ui.label(theme::muted(
        "Animasi yang sudah dibuat di skenario ini. Klik untuk memilih dan mengatur di Inspector.",
    ));
    ui.add_space(6.0);
    if routes.is_empty() {
        ui.label(theme::muted("Belum ada animasi di skenario ini."));
        if ui.button("Buka tab Animasi").clicked() {
            editor.left_tab = 3;
            editor.scene_tab = 0;
        }
        return;
    }

    ui.horizontal(|ui| {
        ui.label(theme::muted("Filter"));
        if ui
            .selectable_label(editor.anim_filter_hidden, "Tersembunyi")
            .on_hover_text("Hanya tampilkan animasi yang disembunyikan dari peta")
            .clicked()
        {
            editor.anim_filter_hidden = !editor.anim_filter_hidden;
        }
        if ui
            .selectable_label(editor.anim_filter_locked, "Terkunci")
            .on_hover_text("Hanya tampilkan animasi yang dikunci")
            .clicked()
        {
            editor.anim_filter_locked = !editor.anim_filter_locked;
        }
    });
    ui.add_space(4.0);

    let filtered = routes
        .iter()
        .filter(|(_, _, _, visible, locked)| {
            (!editor.anim_filter_hidden || !*visible) && (!editor.anim_filter_locked || *locked)
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut select = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::CollapsingHeader::new(format!("Rute ({})", filtered.len()))
                .id_salt("editor_animation_list")
                .default_open(true)
                .show(ui, |ui| {
                    if filtered.is_empty() {
                        ui.label(theme::muted("Tidak ada animasi yang cocok dengan filter."));
                        if ui.button("Reset filter").clicked() {
                            editor.anim_filter_hidden = false;
                            editor.anim_filter_locked = false;
                        }
                        return;
                    }
                    for (key, name, marker_id, visible, locked) in &filtered {
                        let marker_name = marker_id.and_then(|id| {
                            editor.objects.iter().find_map(|object| {
                                (object.server_id == Some(id)
                                    && matches!(object.kind, EditorKind::Marker(_)))
                                .then(|| object.name.clone())
                            })
                        });
                        let marker_label = if marker_name.is_some() {
                            marker_name.unwrap_or_default()
                        } else if marker_id.is_some() {
                            "marker hilang".to_owned()
                        } else {
                            "marker belum dipilih".to_owned()
                        };
                        ui.horizontal_top(|ui| {
                            let mut shown = *visible;
                            if ui
                                .checkbox(&mut shown, "")
                                .on_hover_text("Tampilkan animasi di peta")
                                .changed()
                            {
                                if let Some(object) = editor.object_mut(*key) {
                                    object.visible = shown;
                                }
                                editor.touch();
                            }
                            if lock_icon_button(ui, *locked).clicked() {
                                if *locked {
                                    editor.locked.remove(key);
                                } else {
                                    editor.locked.insert(*key);
                                }
                            }
                            let selected = editor.selected.contains(key);
                            let frame = egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(4, 2))
                                .corner_radius(egui::CornerRadius::same(4))
                                .fill(if selected {
                                    theme::color_alpha(DARK.ring, 90)
                                } else {
                                    egui::Color32::TRANSPARENT
                                });
                            let mut clicked = false;
                            frame.show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.spacing_mut().item_spacing.y = 1.0;
                                clicked |= ui
                                    .add(
                                        egui::Label::new(
                                            egui::RichText::new(name.as_str()).size(FONT_XS).color(
                                                if *visible {
                                                    theme::color(DARK.foreground)
                                                } else {
                                                    theme::color(DARK.muted_foreground)
                                                },
                                            ),
                                        )
                                        .wrap()
                                        .sense(egui::Sense::click()),
                                    )
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked();
                                clicked |= ui
                                    .add(
                                        egui::Label::new(
                                            egui::RichText::new(format!("· {marker_label}"))
                                                .size(FONT_XS)
                                                .color(theme::color(DARK.muted_foreground)),
                                        )
                                        .wrap()
                                        .sense(egui::Sense::click()),
                                    )
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked();
                            });
                            if clicked {
                                select = Some(*key);
                            }
                        });
                    }
                });
        });

    if let Some(key) = select {
        editor.select_only(key);
        editor.focus_object(key);
    }
}

/// Padlock toggle for a route row. Locked routes stay visible on the map but
/// cannot be picked or box-selected there.
fn lock_icon_button(ui: &mut egui::Ui, locked: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
    let color = if locked {
        theme::color(DARK.warning)
    } else if response.hovered() {
        theme::color(DARK.foreground)
    } else {
        theme::color(DARK.muted_foreground)
    };
    let painter = ui.painter();
    let center = rect.center();
    // Body of the padlock, then the shackle arc above it. The unlocked state
    // shifts the arc right so the open side is visible at a glance.
    let body = egui::Rect::from_center_size(center + egui::vec2(0.0, 2.5), egui::vec2(9.0, 7.0));
    painter.rect_filled(body, 1.5, color);
    let shackle = if locked {
        center + egui::vec2(0.0, -1.0)
    } else {
        center + egui::vec2(2.5, -2.5)
    };
    let radius = 3.5;
    let arc = (0..=12)
        .map(|step| {
            let angle = std::f32::consts::PI + step as f32 / 12.0 * std::f32::consts::PI;
            shackle + egui::vec2(angle.cos() * radius, angle.sin() * radius)
        })
        .collect::<Vec<_>>();
    painter.add(egui::Shape::line(arc, egui::Stroke::new(1.6, color)));
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.on_hover_text(if locked {
        "Buka kunci rute"
    } else {
        "Kunci rute (tidak bisa dipilih di peta)"
    })
}

/// Modal upload form opened from the Import tab.
fn draw_import_dialog(context: &egui::Context, editor: &mut EditorState) {
    if !editor.import_dialog_open {
        return;
    }
    let mut close = false;
    let response = egui::Modal::new(egui::Id::new("editor_import_modal"))
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::same(16)),
        )
        .show(context, |ui| {
            ui.set_width(380.0);
            ui.label(theme::title("Import aset"));
            ui.label(theme::muted(
                "Aset tersimpan di server untuk operasi ini dan dipakai semua skenarionya.",
            ));
            ui.add_space(6.0);
            draw_import_form(ui, editor);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let ready = import_ready(editor) && !editor.import_busy;
                if ui
                    .add_enabled(ready, egui::Button::new("Unggah aset"))
                    .clicked()
                {
                    editor.requested_import_upload = true;
                }
                if ui.button("Batal").clicked() && !editor.import_busy {
                    close = true;
                }
            });
        });
    if response.should_close() && !editor.import_busy {
        close = true;
    }
    if close {
        editor.import_dialog_open = false;
        editor.import_error = None;
    }
}

/// Confirmation before selected map objects are removed.
fn draw_delete_dialog(context: &egui::Context, editor: &mut EditorState) {
    if !editor.confirm_delete {
        return;
    }
    let count = editor.selected.len();
    let message = if count == 1 {
        editor
            .selected
            .iter()
            .next()
            .and_then(|key| editor.object(*key))
            .map(|object| format!("Hapus objek \"{}\" dari skenario?", object.name))
            .unwrap_or_else(|| "Hapus objek dari skenario?".to_owned())
    } else {
        format!("Hapus {count} objek dari skenario?")
    };
    let mut close = false;
    let response = egui::Modal::new(egui::Id::new("editor_delete_modal"))
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::same(16)),
        )
        .show(context, |ui| {
            ui.set_width(340.0);
            ui.label(theme::title("Hapus objek"));
            ui.label(message);
            ui.label(theme::muted(
                "Setelah dihapus masih bisa dikembalikan dengan Ctrl+Z.",
            ));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new("Hapus").fill(theme::color(DARK.destructive)))
                    .clicked()
                {
                    editor.confirm_delete = false;
                    editor.delete_selected();
                    close = true;
                }
                if ui.button("Batal").clicked() {
                    close = true;
                }
            });
        });
    if response.should_close() {
        close = true;
    }
    if close {
        editor.confirm_delete = false;
    }
}

/// Confirmation before an imported asset is removed from the server.
fn draw_import_delete_dialog(context: &egui::Context, editor: &mut EditorState) {
    let Some((asset_id, name)) = editor.import_delete.clone() else {
        return;
    };
    let mut close = false;
    let response = egui::Modal::new(egui::Id::new("editor_import_delete_modal"))
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::same(16)),
        )
        .show(context, |ui| {
            ui.set_width(340.0);
            ui.label(theme::title("Hapus aset impor"));
            ui.label(format!(
                "Aset \"{name}\" akan dihapus dari server dan dari cache komputer lain \
                 saat sinkronisasi berikutnya."
            ));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new("Hapus").fill(theme::color(DARK.destructive)))
                    .clicked()
                {
                    editor.requested_import_delete = Some(asset_id);
                    close = true;
                }
                if ui.button("Batal").clicked() {
                    close = true;
                }
            });
        });
    if response.should_close() {
        close = true;
    }
    if close {
        editor.import_delete = None;
    }
}

fn draw_import_form(ui: &mut egui::Ui, editor: &mut EditorState) {
    ui.label("Nama");
    ui.add(
        egui::TextEdit::singleline(&mut editor.import_draft.name)
            .hint_text("Nama aset")
            .desired_width(f32::INFINITY)
            .margin(egui::Margin::symmetric(8, 5)),
    );

    ui.label("Kategori");
    let mut category = editor.import_draft.category;
    egui::ComboBox::from_id_salt("import_category")
        .selected_text(asset_category_label(category))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for option in AssetCategory::ALL {
                ui.selectable_value(&mut category, option, asset_category_label(option));
            }
        });
    if category != editor.import_draft.category {
        editor.import_draft.category = category;
        editor.import_draft.files.clear();
    }
    let formats = editor
        .import_draft
        .category
        .upload_extensions()
        .iter()
        .map(|extension| format!(".{extension}"))
        .collect::<Vec<_>>()
        .join(", ");
    ui.label(theme::muted(&format!("Format file: {formats}")));

    if editor.import_draft.category.is_model() {
        ui.label("Jenis model");
        let mut kind = editor.import_draft.model_kind;
        egui::ComboBox::from_id_salt("import_model_kind")
            .selected_text(model_kind_label(kind))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for option in AssetModelKind::ALL {
                    ui.selectable_value(&mut kind, option, model_kind_label(option));
                }
            });
        if kind != editor.import_draft.model_kind {
            editor.import_draft.model_kind = kind;
            editor.import_draft.files.clear();
        }
        if kind == AssetModelKind::Static {
            ui.label("Mode pindah di Play");
            let mut move_mode = editor.import_draft.move_mode;
            egui::ComboBox::from_id_salt("import_move_mode")
                .selected_text(move_mode_label(move_mode))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for option in AssetMoveMode::ALL {
                        ui.selectable_value(&mut move_mode, option, move_mode_label(option));
                    }
                });
            editor.import_draft.move_mode = move_mode;
        }
    }

    let slots = editor.import_draft.slots();
    for (index, slot) in slots.iter().enumerate() {
        ui.add_space(4.0);
        ui.label(import_slot_label(*slot));
        let selected = editor
            .import_draft
            .file(index)
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().to_string());
        ui.horizontal(|ui| {
            ui.label(theme::muted(selected.as_deref().unwrap_or("Belum dipilih")));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("Pilih file...").clicked()
                    && let Some(path) =
                        crate::file_picker::pick_asset_file(editor.import_draft.category)
                {
                    editor.import_draft.set_file(index, path);
                }
                #[cfg(target_arch = "wasm32")]
                ui.label(theme::muted("Khusus desktop"));
            });
        });
    }

    if editor.import_busy {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(theme::muted("Mengunggah ke server..."));
        });
    }
    if let Some(error) = editor.import_error.clone() {
        ui.colored_label(theme::color(DARK.destructive), error);
    }
}

fn import_ready(editor: &EditorState) -> bool {
    let slots = editor.import_draft.slots();
    !editor.import_draft.name.trim().is_empty()
        && slots
            .iter()
            .enumerate()
            .all(|(index, _)| editor.import_draft.file(index).is_some())
}

fn sky_mode_label(mode: SkyMode) -> &'static str {
    match mode {
        SkyMode::Black => "Hitam",
        SkyMode::Skybox => "Skybox",
        SkyMode::Environment => "Environment (IBL)",
    }
}

fn asset_category_label(category: AssetCategory) -> &'static str {
    match category {
        AssetCategory::Model => "Model 3D",
        AssetCategory::Texture => "Tekstur",
        AssetCategory::Icon => "Ikon",
    }
}

fn model_kind_label(kind: AssetModelKind) -> &'static str {
    match kind {
        AssetModelKind::Static => "Static",
        AssetModelKind::Animated => "Animasi",
    }
}

fn move_mode_label(mode: AssetMoveMode) -> &'static str {
    match mode {
        AssetMoveMode::None => "Tidak bisa pindah",
        AssetMoveMode::Limited => "Pindah via gizmo",
        AssetMoveMode::Free => "Pindah bebas",
    }
}

fn import_slot_label(slot: AssetSourceSlot) -> &'static str {
    match slot {
        AssetSourceSlot::Static => "File",
        AssetSourceSlot::Idle => "File diam (idle)",
        AssetSourceSlot::Animated => "File animasi",
    }
}

/// Import asset row: drag the name onto the map, click **+** to add it at the
/// camera centre, and click the **x** to delete your own upload.
fn draw_import_row(ui: &mut egui::Ui, editor: &mut EditorState, row: &EditorAssetRow) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
    let trash_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 15.0, rect.center().y),
        egui::vec2(20.0, 20.0),
    );
    let plus_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 38.0, rect.center().y),
        egui::vec2(20.0, 20.0),
    );
    let row_rect =
        egui::Rect::from_min_max(rect.min, egui::pos2(plus_rect.left() - 4.0, rect.bottom()));
    let response = ui.interact(
        row_rect,
        ui.id().with(("import-row", row.unit_id.as_str())),
        egui::Sense::click_and_drag(),
    );
    let plus = ui.interact(
        plus_rect,
        ui.id().with(("import-add", row.unit_id.as_str())),
        egui::Sense::click(),
    );
    let trash = ui.interact(
        trash_rect,
        ui.id().with(("import-delete", row.unit_id.as_str())),
        egui::Sense::click(),
    );
    let painter = ui.painter().clone();
    if response.hovered() || plus.hovered() || trash.hovered() {
        painter.rect_filled(rect, 4.0, theme::color(DARK.secondary));
    }
    let name_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 6.0, rect.top()),
        egui::pos2(plus_rect.left() - 6.0, rect.bottom()),
    );
    painter.with_clip_rect(name_rect).text(
        egui::pos2(rect.left() + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &row.name,
        egui::FontId::proportional(FONT_SM),
        if row.cached {
            theme::color(DARK.foreground)
        } else {
            theme::color(DARK.muted_foreground)
        },
    );

    let plus_color = if row.cached {
        theme::color(DARK.foreground)
    } else {
        theme::color(DARK.muted_foreground)
    };
    if row.is_model {
        if plus.hovered() && row.cached {
            painter.rect_filled(plus_rect, 4.0, theme::color(DARK.border));
        }
        let plus_center = plus_rect.center();
        let plus_stroke = egui::Stroke::new(1.4, plus_color);
        painter.line_segment(
            [
                plus_center + egui::vec2(-4.0, 0.0),
                plus_center + egui::vec2(4.0, 0.0),
            ],
            plus_stroke,
        );
        painter.line_segment(
            [
                plus_center + egui::vec2(0.0, -4.0),
                plus_center + egui::vec2(0.0, 4.0),
            ],
            plus_stroke,
        );
    }

    if row.can_delete {
        if trash.hovered() {
            painter.rect_filled(trash_rect, 4.0, theme::color(DARK.border));
        }
        let center = trash_rect.center();
        let color = if trash.hovered() {
            theme::color(DARK.destructive)
        } else {
            theme::color(DARK.muted_foreground)
        };
        let stroke = egui::Stroke::new(1.4, color);
        painter.line_segment(
            [
                center + egui::vec2(-3.5, -3.5),
                center + egui::vec2(3.5, 3.5),
            ],
            stroke,
        );
        painter.line_segment(
            [
                center + egui::vec2(3.5, -3.5),
                center + egui::vec2(-3.5, 3.5),
            ],
            stroke,
        );
    }

    if plus.hovered() || trash.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if plus.clicked() && row.cached && row.is_model {
        add_asset_to_map(editor, row);
    }
    if trash.clicked()
        && row.can_delete
        && let Ok(asset_id) = uuid::Uuid::parse_str(&row.unit_id)
    {
        editor.import_delete = Some((asset_id, row.name.clone()));
    }
    if row.is_model && row.cached && response.drag_started() {
        editor.asset_drag = Some(AssetMarkerDraft {
            unit_id: row.unit_id.clone(),
            name: row.name.clone(),
        });
    }
    if response.hovered() && editor.asset_drag.is_none() {
        response
            .clone()
            .on_hover_ui(|ui| draw_asset_tooltip(ui, editor, row));
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

fn draw_scene_panel(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    asset_rows: &[EditorAssetRow],
    import_rows: &[EditorAssetRow],
) {
    egui::Panel::left("editor_scene")
        .resizable(true)
        .default_size(260.0)
        .min_size(230.0)
        .max_size(460.0)
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .inner_margin(egui::Margin::same(0)),
        )
        .show(ui, |ui| {
            draw_editor_tabs(ui, editor);
            egui::Frame::new()
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| match editor.left_tab {
                    1 => draw_asset_tab(ui, editor, asset_rows),
                    2 => draw_import_tab(ui, editor, import_rows),
                    3 => draw_animation_create_tab(ui, editor),
                    _ => {
                        draw_scene_subtabs(ui, editor);
                        match editor.scene_tab {
                            1 => draw_animation_list_tab(ui, editor),
                            _ => draw_scene_tree(ui, editor),
                        }
                    }
                });
        });
}

/// Secondary tab strip inside Scene. "Layers" is the existing scene tree;
/// "Daftar Anim" lists the route animations authored for this scenario.
fn draw_scene_subtabs(ui: &mut egui::Ui, editor: &mut EditorState) {
    const HEIGHT: f32 = 24.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), HEIGHT),
        egui::Sense::hover(),
    );
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 5.0, theme::color(DARK.secondary));
    let width = rect.width() / 2.0;
    for (index, label) in ["Layers", "Daftar Anim"].into_iter().enumerate() {
        let tab = egui::Rect::from_min_size(
            egui::pos2(rect.left() + width * index as f32, rect.top()),
            egui::vec2(width, rect.height()),
        );
        let response = ui.interact(
            tab,
            ui.id().with(("editor-scene-tab", index)),
            egui::Sense::click(),
        );
        let selected = editor.scene_tab == index;
        if selected {
            painter.rect_filled(tab.shrink(2.0), 4.0, theme::color(DARK.input));
            painter.hline(
                (tab.left() + 4.0)..=(tab.right() - 4.0),
                tab.bottom() - 2.0,
                egui::Stroke::new(2.0, theme::color(DARK.primary)),
            );
        } else if response.hovered() {
            painter.rect_filled(tab.shrink(2.0), 4.0, theme::color(DARK.border));
        }
        painter.text(
            tab.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(FONT_XS),
            theme::color(if selected {
                DARK.foreground
            } else {
                DARK.muted_foreground
            }),
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.clicked() {
            editor.scene_tab = index;
        }
    }
    ui.add_space(6.0);
    ui.separator();
}

/// Flat tab strip for the left editor panel. The bar spans the full panel
/// width, the active tab is a lighter block without rounded corners, and the
/// scene object count stays aligned to the right edge.
fn draw_editor_tabs(ui: &mut egui::Ui, editor: &mut EditorState) {
    const TAB_HEIGHT: f32 = 28.0;
    const TAB_PADDING: f32 = 8.0;
    if editor.left_tab > 3 {
        editor.left_tab = 0;
    }
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), TAB_HEIGHT),
        egui::Sense::hover(),
    );
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0.0, theme::color(DARK.secondary));
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        egui::Stroke::new(1.0, theme::color(DARK.border)),
    );

    let mut left = rect.left();
    for (index, label) in ["Scene", "Assets", "Import", "Animasi"]
        .into_iter()
        .enumerate()
    {
        let galley = painter.layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(FONT_XS),
            theme::color(DARK.foreground),
        );
        let width = (galley.size().x + TAB_PADDING * 2.0).max(50.0);
        let tab = egui::Rect::from_min_size(
            egui::pos2(left, rect.top()),
            egui::vec2(width, rect.height()),
        );
        let response = ui.interact(
            tab,
            ui.id().with(("editor-tab", index)),
            egui::Sense::click(),
        );
        let selected = editor.left_tab == index;
        let fill = if selected {
            Some(theme::color(DARK.input))
        } else if response.hovered() {
            Some(theme::color(DARK.border))
        } else {
            None
        };
        if let Some(fill) = fill {
            painter.rect_filled(tab, 0.0, fill);
        }
        painter.text(
            tab.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(FONT_XS),
            if selected {
                theme::color(DARK.foreground)
            } else {
                theme::color(DARK.muted_foreground)
            },
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.clicked() {
            editor.left_tab = index;
        }
        left += width;
    }

    if editor.left_tab == 0 {
        painter.text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            editor.objects.len().to_string(),
            egui::FontId::monospace(FONT_XS),
            theme::color(DARK.muted_foreground),
        );
    }
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
                    .filter(|object| {
                        object.object_type() == object_type
                            && !(object_type == ObjectType::Line && object.is_animation_route())
                    })
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
                        draw_object_inspector(editor, key, ui, texture_rows);
                    } else if editor.selected.len() > 1 {
                        ui.label(format!("{} objek dipilih", editor.selected.len()));
                        ui.add_space(6.0);
                        if ui.button("Duplicate").clicked() {
                            editor.duplicate_selected();
                        }
                        if ui.button("Hapus").clicked() {
                            editor.request_delete_selected();
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
    ui.horizontal(|ui| {
        ui.label("Langit");
        let mut sky = editor.sky;
        egui::ComboBox::from_id_salt("editor_sky_mode")
            .selected_text(sky_mode_label(sky))
            .width(170.0)
            .show_ui(ui, |ui| {
                for option in SkyMode::ALL {
                    ui.selectable_value(&mut sky, option, sky_mode_label(option));
                }
            });
        let changed = sky != editor.sky;
        editor.sky = sky;
        if changed && can_admin && editor.loaded && editor.scenario_id.is_some() {
            *action = EditorAction::SaveView;
        }
    });
    ui.label(theme::muted(match editor.sky {
        SkyMode::Black => "Latar hitam seperti sebelumnya.",
        SkyMode::Skybox => "Langit bergradasi tanpa mengubah pencahayaan.",
        SkyMode::Environment => "Langit + environment map (IBL) untuk pencahayaan.",
    }));
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
                    "Simpan pusat, zoom, bearing, pitch, base layer, dan langit sebagai default skenario",
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
) {
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
            if let Some(animation) = &mut line.animation {
                ui.label(theme::title("Animasi rute"));
                let speed = drag_row(ui, "Kecepatan", &mut animation.speed, 0.1, 0.5, 1, " m/s");
                undo |= speed.gained_focus() || speed.drag_started();
                if speed.changed() {
                    animation.speed = animation.speed.clamp(0.1, 100.0);
                    changed = true;
                }
                changed |= ui
                    .checkbox(&mut animation.repeat, "Ulangi (repeat)")
                    .on_hover_text("Mulai lagi dari titik pertama setelah mencapai ujung rute.")
                    .changed();
                ui.label("Marker yang bergerak");
                ui.horizontal(|ui| {
                    ui.label(theme::muted("Cari"));
                    ui.add(
                        egui::TextEdit::singleline(&mut editor.route_marker_search)
                            .hint_text("Nama marker...")
                            .desired_width(150.0),
                    );
                });
                let options = editor.animation_marker_options(key);
                let option_count = options.len();
                let mut marker = animation.marker_id;
                widgets::combo(
                    ui,
                    egui::Id::new(("route_marker", key)),
                    &mut marker,
                    &options,
                );
                if animation.marker_id != marker {
                    animation.marker_id = marker;
                    changed = true;
                }
                if option_count <= 1 {
                    if editor.route_marker_search.trim().is_empty() {
                        ui.label(theme::muted(
                            "Belum ada marker tersimpan. Simpan skenario (Ctrl+S) setelah membuat marker.",
                        ));
                    } else {
                        ui.label(theme::muted(
                            "Tidak ada marker yang cocok dengan pencarian.",
                        ));
                    }
                }
                let picking = editor.pick_route_marker == Some(key);
                if ui
                    .button(if picking {
                        "Klik marker di peta... (Batal)"
                    } else {
                        "Pilih marker di peta"
                    })
                    .clicked()
                {
                    if picking {
                        editor.pick_route_marker = None;
                    } else {
                        editor.end_point_edit();
                        editor.pick_route_marker = Some(key);
                        editor.log("Klik marker di peta untuk dijadikan target rute. Esc batal.");
                    }
                }
                match animation.marker_id {
                    Some(id) => {
                        let marker_object = editor.objects.iter().find(|object| {
                            object.server_id == Some(id)
                                && matches!(&object.kind, EditorKind::Marker(_))
                        });
                        match marker_object {
                            Some(object) => {
                                let animatable = match &object.kind {
                                    EditorKind::Marker(marker) => {
                                        editor.marker_is_animatable(marker)
                                    }
                                    _ => false,
                                };
                                if !animatable {
                                    ui.label(theme::danger(
                                        "Marker ini tidak bisa bergerak. Ubah mode pindah asetnya \
                                         menjadi Free/Limited atau pakai model animasi.",
                                    ));
                                }
                            }
                            None => {
                                ui.label(theme::danger(
                                    "Marker rute tidak ditemukan. Pilih marker lain lalu simpan.",
                                ));
                            }
                        }
                        let used_elsewhere = editor.objects.iter().any(|other| {
                            other.key != key
                                && matches!(&other.kind, EditorKind::Line(line) if
                                    line.animation.as_ref().and_then(|route| route.marker_id)
                                        == Some(id))
                        });
                        if used_elsewhere {
                            ui.label(theme::danger("Marker ini sudah dipakai rute lain."));
                        }
                    }
                    None => {
                        ui.label(theme::muted(
                            "Pilih dari daftar (bisa dicari) atau klik marker langsung di peta.",
                        ));
                    }
                }
                ui.label(theme::muted(
                    "Titik, warna, lebar, dan alpha garis tetap bisa diedit seperti line biasa.",
                ));
                ui.separator();
            }
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
                let selected_name =
                    texture_reference_label(building.texture.as_deref(), texture_rows);
                let mut picked_texture: Option<String> = None;
                egui::ComboBox::from_id_salt("shape_texture")
                    .selected_text(selected_name)
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        if texture_rows.is_empty() {
                            ui.label(theme::muted("Belum ada tekstur. Unggah lewat tab Import."));
                        }
                        let mut first = true;
                        for (imported, label) in
                            [(false, "Aset admin"), (true, "Import operasi ini")]
                        {
                            let rows = texture_rows
                                .iter()
                                .filter(|row| row.imported == imported)
                                .collect::<Vec<_>>();
                            if rows.is_empty() {
                                continue;
                            }
                            if !first {
                                ui.separator();
                            }
                            first = false;
                            ui.label(theme::muted(label));
                            for row in rows {
                                if row.available {
                                    let selected =
                                        building.texture.as_deref() == Some(row.id.as_str());
                                    if ui.selectable_label(selected, &row.name).clicked() {
                                        picked_texture = Some(row.id.clone());
                                    }
                                } else {
                                    ui.label(theme::muted(&format!(
                                        "{} (belum diunduh)",
                                        row.name
                                    )));
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
                    ui.label(theme::muted("Pilih tekstur dari daftar aset."));
                }

                if building
                    .texture
                    .as_deref()
                    .is_some_and(|reference| editor.texture_source(reference).is_none())
                {
                    ui.label(theme::muted("Tekstur belum tersedia; memakai warna solid."));
                }
                ui.label(theme::muted(
                    "Tambahkan gambar lewat tab Import agar tersedia di semua komputer.",
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
        editor.request_delete_selected();
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

fn texture_reference_label(reference: Option<&str>, texture_rows: &[EditorTextureRow]) -> String {
    let Some(reference) = reference else {
        return "Pilih tekstur".to_owned();
    };
    if reference.starts_with("/uploads/") {
        let name = reference.rsplit('/').next().unwrap_or(reference);
        return format!("Server: {name}");
    }
    if let Some(row) = texture_rows.iter().find(|row| row.id == reference) {
        return if row.imported {
            format!("Import: {}", row.name)
        } else {
            row.name.clone()
        };
    }
    "Pilih tekstur".to_owned()
}

fn draw_texture_preview(ui: &mut egui::Ui, editor: &mut EditorState, reference: &str) {
    let Some(texture) = editor.texture_preview(ui.ctx(), reference) else {
        ui.label(theme::muted("Pratinjau belum tersedia."));
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
            if editor.pick_route_marker.is_some() {
                let text = "Klik marker di peta untuk target rute · Esc batal";
                let galley = painter.layout_no_wrap(
                    text.to_owned(),
                    egui::FontId::proportional(12.0),
                    egui::Color32::WHITE,
                );
                let center = egui::pos2(rect.center().x, rect.top() + 20.0);
                let chip =
                    egui::Rect::from_center_size(center, galley.size() + egui::vec2(16.0, 8.0));
                painter.rect_filled(chip, 5.0, egui::Color32::from_black_alpha(190));
                painter.galley(center - galley.size() * 0.5, galley, egui::Color32::WHITE);
            }
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
    // A selected route animation rings the marker it drives, so the target is
    // easy to spot on a map with many markers.
    let route_markers = editor.selected_route_markers();
    let route_stroke = egui::Stroke::new(2.0, theme::color(DARK.success));
    for (key, _kind, points, selected) in &editor.projected {
        let is_route_marker = route_markers.contains(key);
        let stroke = if *selected {
            selected_stroke
        } else if is_route_marker {
            route_stroke
        } else if editor.hover_key == Some(*key) {
            hover_stroke
        } else {
            continue;
        };
        match _kind {
            ObjectType::Marker | ObjectType::Text => {
                if let Some(center) = points.first() {
                    if is_route_marker && !*selected {
                        dashed_ring(painter, *center, 19.0, stroke);
                    } else {
                        painter.circle_stroke(*center, 15.0, stroke);
                    }
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

/// Dashed circle used to mark the marker linked to the selected route.
fn dashed_ring(painter: &egui::Painter, center: egui::Pos2, radius: f32, stroke: egui::Stroke) {
    const SEGMENTS: usize = 40;
    let points = (0..=SEGMENTS)
        .map(|index| {
            let angle = index as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            center + egui::vec2(angle.cos() * radius, angle.sin() * radius)
        })
        .collect::<Vec<_>>();
    painter.extend(egui::Shape::dashed_line(&points, stroke, 5.0, 4.0));
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
    let is_route = editor
        .object(key)
        .is_some_and(|object| object.is_animation_route());
    let visible = editor.object(key).is_some_and(|object| object.visible);
    let locked = editor.is_locked(key);
    let locked_route = is_route && locked;
    let mut close = false;
    let area = egui::Area::new(egui::Id::new("editor_context_menu"))
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
                    if is_route {
                        let hide_label = if visible { "Sembunyikan" } else { "Tampilkan" };
                        if ui.button(hide_label).clicked() {
                            if let Some(object) = editor.object_mut(key) {
                                object.visible = !object.visible;
                            }
                            editor.touch();
                            close = true;
                        }
                        let lock_label = if locked { "Buka kunci" } else { "Kunci rute" };
                        if ui.button(lock_label).clicked() {
                            if locked {
                                editor.locked.remove(&key);
                            } else {
                                editor.locked.insert(key);
                            }
                            close = true;
                        }
                        ui.separator();
                    }
                    let duplicate = ui.add_enabled(!locked_route, egui::Button::new("Duplicate"));
                    if duplicate
                        .on_disabled_hover_text("Rute terkunci — buka kunci dulu")
                        .clicked()
                    {
                        editor.select_only(key);
                        editor.duplicate_selected();
                        close = true;
                    }
                    let delete = ui.add_enabled(!locked_route, egui::Button::new("Hapus"));
                    if delete
                        .on_disabled_hover_text("Rute terkunci — buka kunci dulu")
                        .clicked()
                    {
                        editor.select_only(key);
                        editor.request_delete_selected();
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
    // Clicking anywhere outside the menu dismisses it. A click on one of its
    // buttons is handled above and must not be swallowed. Only the primary
    // button counts: the right click that opened the menu would otherwise
    // close it on the same frame.
    if context.input(|input| input.pointer.primary_clicked()) && !area.response.contains_pointer() {
        close = true;
    }
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
