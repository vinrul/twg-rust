use bevy_egui::egui;
use twg_client::design::DARK;
use twg_map::BaseLayer;

use crate::state::ClientUi;
use crate::theme;

use super::input::distance_meters;
use super::model::{
    EditorKind, EditorState, EditorTool, ObjectKey, ObjectType, UNIT_CATALOG, marker_color,
    parse_hex_color,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorAction {
    None,
    Save,
    SaveAndClose,
    Close,
    Reload,
    SaveAndReload,
    Geocode,
}

pub fn draw(context: &egui::Context, client: &mut ClientUi) {
    client.ensure_ws();
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
                draw_bottom(ui, editor);
                draw_scene_panel(ui, editor);
                draw_inspector(ui, editor);
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
                ui.label(egui::RichText::new("▶ PREVIEW").strong());
                ui.separator();
                ui.label(&editor.scenario_name);
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
                        .checkbox(&mut editor.show_assets, "Panel assets")
                        .changed();
                    changed |= ui
                        .checkbox(&mut editor.show_console, "Panel console")
                        .changed();
                    if changed {
                        editor.touch();
                    }
                });
                ui.menu_button("Help", |ui| {
                    ui.label("Q/W/E/R  Select · Move · Rotate · Scale");
                    ui.label("1..5     Marker · Line · Polygon · Text · Measure");
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
                    let label = format!("{}  {}", tool.icon(), tool.label());
                    if ui
                        .add(egui::Button::new(label).selected(selected).small())
                        .on_hover_text(format!("{} ({})", tool.label(), tool.shortcut()))
                        .clicked()
                    {
                        editor.tool = tool;
                        editor.draft.clear();
                    }
                }
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

fn draw_bottom(ui: &mut egui::Ui, editor: &mut EditorState) {
    if !editor.show_assets && !editor.show_console {
        return;
    }
    egui::Panel::bottom("editor_bottom")
        .resizable(true)
        .default_size(168.0)
        .min_size(90.0)
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .inner_margin(egui::Margin::symmetric(8, 4)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(editor.bottom_tab == 0, "Assets")
                    .clicked()
                {
                    editor.bottom_tab = 0;
                }
                if ui
                    .selectable_label(editor.bottom_tab == 1, "Console")
                    .clicked()
                {
                    editor.bottom_tab = 1;
                }
                ui.separator();
                ui.label(theme::muted(if editor.bottom_tab == 0 {
                    "Katalog unit lokal"
                } else {
                    "Log editor"
                }));
            });
            ui.separator();
            if editor.bottom_tab == 0 {
                egui::ScrollArea::horizontal()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (index, unit) in UNIT_CATALOG.iter().enumerate() {
                                egui::Frame::new()
                                    .fill(theme::color(DARK.secondary))
                                    .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                                    .corner_radius(egui::CornerRadius::same(6))
                                    .inner_margin(egui::Margin::same(8))
                                    .show(ui, |ui| {
                                        ui.set_width(150.0);
                                        ui.label(egui::RichText::new(unit.name).strong());
                                        ui.label(theme::muted(unit.category));
                                        ui.label(theme::muted(unit.model));
                                        ui.add_space(4.0);
                                        if ui
                                            .add(
                                                egui::Button::new(if editor.active_unit == index {
                                                    "Aktif"
                                                } else {
                                                    "Pilih"
                                                })
                                                .selected(editor.active_unit == index),
                                            )
                                            .clicked()
                                        {
                                            editor.active_unit = index;
                                            editor.tool = EditorTool::Marker;
                                        }
                                    });
                            }
                        });
                    });
            } else {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for line in &editor.console {
                            ui.monospace(line);
                        }
                    });
            }
        });
}

fn draw_scene_panel(ui: &mut egui::Ui, editor: &mut EditorState) {
    egui::Panel::left("editor_scene")
        .resizable(true)
        .default_size(238.0)
        .min_size(180.0)
        .max_size(420.0)
        .frame(
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .inner_margin(egui::Margin::same(8)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::title("Scene"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.monospace(format!("{}", editor.objects.len()));
                });
            });

            ui.separator();
            ui.label(theme::title("Layers"));
            let mut layers_changed = false;
            egui::Grid::new("editor_layers")
                .num_columns(2)
                .show(ui, |ui| {
                    layers_changed |= ui.checkbox(&mut editor.visible.marker, "Marker").changed();
                    layers_changed |= ui.checkbox(&mut editor.visible.line, "Line").changed();
                    ui.end_row();
                    layers_changed |= ui
                        .checkbox(&mut editor.visible.building, "Building")
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
                let submit =
                    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
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
                                        let additive = ui.ctx().input(|input| {
                                            input.modifiers.ctrl || input.modifiers.command
                                        });
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
            }
            if let Some(key) = toggle_visibility {
                if let Some(object) = editor.object_mut(key) {
                    object.visible = !object.visible;
                }
                editor.touch();
            }
        });
}

fn draw_inspector(ui: &mut egui::Ui, editor: &mut EditorState) {
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
                        draw_object_inspector(editor, key, ui);
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
                        draw_scenario_inspector(editor, ui);
                    }
                });
        });
}

fn draw_scenario_inspector(editor: &mut EditorState, ui: &mut egui::Ui) {
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

fn draw_object_inspector(editor: &mut EditorState, key: ObjectKey, ui: &mut egui::Ui) {
    let Some(mut object) = editor.object(key).cloned() else {
        return;
    };
    let mut changed = false;
    let mut undo = false;
    let mut delete = false;
    let mut duplicate = false;

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
            let selected_unit = UNIT_CATALOG[marker.unit.min(UNIT_CATALOG.len() - 1)].name;
            egui::ComboBox::from_id_salt("marker_unit")
                .selected_text(selected_unit)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (index, entry) in UNIT_CATALOG.iter().enumerate() {
                        if ui
                            .selectable_label(marker.unit == index, entry.name)
                            .clicked()
                        {
                            marker.unit = index;
                            marker.unit_id = entry.id.to_owned();
                            changed = true;
                        }
                    }
                });
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
            });
            ui.add_space(4.0);
            ui.label(theme::title("Transform"));
            let lon = drag_row(ui, "Lon", &mut marker.lon, 0.000_001, 7, "");
            undo |= lon.gained_focus() || lon.drag_started();
            changed |= lon.changed();
            let lat = drag_row(ui, "Lat", &mut marker.lat, 0.000_001, 7, "");
            undo |= lat.gained_focus() || lat.drag_started();
            changed |= lat.changed();
            let altitude = drag_row(ui, "Alt", &mut marker.altitude, 0.5, 1, " m");
            undo |= altitude.gained_focus() || altitude.drag_started();
            changed |= altitude.changed();
            let rot_y = drag_row(ui, "Rot Y", &mut marker.rot_y, 1.0, 1, "°");
            undo |= rot_y.gained_focus() || rot_y.drag_started();
            changed |= rot_y.changed();
            let rot_x = drag_row(ui, "Rot X", &mut marker.rot_x, 1.0, 1, "°");
            undo |= rot_x.gained_focus() || rot_x.drag_started();
            changed |= rot_x.changed();
            let rot_z = drag_row(ui, "Rot Z", &mut marker.rot_z, 1.0, 1, "°");
            undo |= rot_z.gained_focus() || rot_z.drag_started();
            changed |= rot_z.changed();
            let scale_x = drag_row(ui, "Scale X", &mut marker.scale_x, 0.01, 2, "");
            undo |= scale_x.gained_focus() || scale_x.drag_started();
            changed |= scale_x.changed();
            let scale_y = drag_row(ui, "Scale Y", &mut marker.scale_y, 0.01, 2, "");
            undo |= scale_y.gained_focus() || scale_y.drag_started();
            changed |= scale_y.changed();
            let scale_z = drag_row(ui, "Scale Z", &mut marker.scale_z, 0.01, 2, "");
            undo |= scale_z.gained_focus() || scale_z.drag_started();
            changed |= scale_z.changed();
            ui.add_space(4.0);
            ui.label("Keterangan");
            changed |= ui.text_edit_multiline(&mut marker.keterangan).changed();
        }
        EditorKind::Line(line) => {
            color_row(ui, "Warna", &mut line.color, &mut changed);
            let width = drag_row(ui, "Lebar", &mut line.width, 0.2, 1, " px");
            undo |= width.drag_started();
            changed |= width.changed();
            let alpha = drag_row(ui, "Alpha", &mut line.alpha, 0.01, 2, "");
            undo |= alpha.drag_started();
            changed |= alpha.changed();
            ui.label(theme::muted(&format!("{} titik", line.points.len())));
        }
        EditorKind::Building(building) => {
            color_row(ui, "Warna", &mut building.color, &mut changed);
            let height = drag_row(ui, "Tinggi", &mut building.height, 0.5, 1, " m");
            undo |= height.drag_started();
            changed |= height.changed();
            ui.label(theme::muted(&format!("{} titik", building.points.len())));
            ui.label("Keterangan");
            changed |= ui.text_edit_multiline(&mut building.keterangan).changed();
        }
        EditorKind::Text(text) => {
            ui.label("Teks");
            changed |= ui.text_edit_multiline(&mut text.text).changed();
            color_row(ui, "Warna", &mut text.color, &mut changed);
            let size = drag_row(ui, "Font", &mut text.font_size, 0.5, 1, " px");
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
            let lon = drag_row(ui, "Lon", &mut text.lon, 0.000_001, 7, "");
            undo |= lon.drag_started();
            changed |= lon.changed();
            let lat = drag_row(ui, "Lat", &mut text.lat, 0.000_001, 7, "");
            undo |= lat.drag_started();
            changed |= lat.changed();
        }
        EditorKind::Particle(particle) => {
            ui.monospace(format!("Tipe: {}", particle.tipe));
            ui.monospace(format!("Spawn: {}", particle.is_spawn));
            ui.monospace(format!("Pos: {:.6}, {:.6}", particle.lat, particle.lon));
            ui.label("Data");
            ui.monospace(&particle.data);
            ui.label(theme::muted(
                "Particle hanya dapat dilihat (endpoint tulis belum ada).",
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
    if delete {
        editor.delete_selected();
    }
    if duplicate {
        editor.duplicate_selected();
    }
}

fn drag_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    speed: f64,
    decimals: usize,
    suffix: &str,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(value)
                .speed(speed)
                .max_decimals(decimals)
                .suffix(suffix),
        )
    })
    .inner
}

fn color_row(ui: &mut egui::Ui, label: &str, color: &mut String, changed: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut rgb = hex_to_rgb(color);
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            *color = hex_from_rgb(rgb);
            *changed = true;
        }
        ui.monospace(color.clone());
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
            }
            draw_labels(&painter, editor);
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
                .fixed_pos(egui::pos2(rect.right() - 118.0, rect.top() + 12.0))
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
                        });
                });
        });
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
            ObjectType::Marker | ObjectType::Text | ObjectType::Particle => {
                if let Some(center) = points.first() {
                    painter.circle_stroke(*center, 15.0, stroke);
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
                painter.circle_filled(*position, 4.0, color);
                painter.circle_stroke(*position, 4.0, outline);
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
