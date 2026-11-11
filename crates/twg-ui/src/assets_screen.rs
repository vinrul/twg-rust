//! Admin asset screen: catalog, preview, conversions, offline "Gunakan".

use bevy_egui::egui;
use twg_client::design::{CONTROL_HEIGHT, DARK, FONT_SM, FONT_XS, RADIUS_SM, SPACE_2, SPACE_3};
use twg_contracts::assets::{
    Asset, AssetCategory, AssetModelKind, AssetMoveMode, AssetSource, AssetSourceSlot,
    AssetVariantKind, ConvertTarget,
};
use twg_domain::Role;
use twg_render::{PREVIEW_SIZE, PREVIEW_SLOTS, PreviewOrbitInput, PreviewPhase};

use crate::state::{
    AssetSyncState, ClientUi, ConfirmAction, ConfirmDialog, Modal, PreviewPanelUi, PreviewUi,
    UsedAsset,
};
use crate::theme::{self, color, color_alpha};
use crate::widgets;

/// Tinggi minimum kolom daftar/detail agar layar tetap dua kolom.
const BODY_MIN_HEIGHT: f32 = 220.0;
/// Tinggi area unggah yang dipin di bawah layar. Form-nya bisa di-scroll,
/// sedangkan baris tombol Upload selalu berada di dalam panel.
const FOOTER_HEIGHT: f32 = 224.0;
/// Di bawah tinggi ini dua kolom tidak lagi menyisakan ruang yang layak, jadi
/// seluruh halaman memakai satu scroll area.
const BODY_MIN_TOTAL: f32 = BODY_MIN_HEIGHT + FOOTER_HEIGHT;
const NON_ADMIN_NOTE: &str = "Hanya admin yang dapat mengunggah atau mengonversi aset.";

pub fn draw(ui: &mut egui::Ui, client: &mut ClientUi) {
    widgets::section(
        ui,
        "Aset",
        Some(
            "Pustaka model static/animasi, texture, dan icon. Konversi berjalan di server; \
             editor dan simulasi memakai file dari cache offline.",
        ),
    );
    draw_toolbar(ui, client);
    ui.add_space(SPACE_3);

    if client.data.is_loading("assets") && client.assets.is_empty() {
        widgets::loading(ui, "Memuat aset...");
        return;
    }
    if client.assets.is_empty() {
        widgets::empty(ui, "Belum ada aset di server.");
    }

    if ui.available_height() < BODY_MIN_TOTAL {
        // Jendela pendek: satu scroll area dengan tinggi kolom tetap. Kolom
        // dibungkus ui berukuran tetap agar separator vertikal tidak memakai
        // tinggi tak terbatas dari scroll area luar.
        egui::ScrollArea::vertical()
            .id_salt("asset-screen")
            .show(ui, |ui| {
                let width = ui.available_width();
                ui.allocate_ui_with_layout(
                    egui::vec2(width, BODY_MIN_HEIGHT),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| draw_body(ui, client, BODY_MIN_HEIGHT),
                );
                ui.add_space(SPACE_3);
                draw_footer_inline(ui, client);
            });
        return;
    }

    // Blok unggah dipin di bawah agar daftar dan detail memakai sisa tinggi
    // jendela, bukan tinggi tetap.
    egui::Panel::bottom("asset-footer")
        .resizable(false)
        .exact_size(FOOTER_HEIGHT)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(color(DARK.background))
                .inner_margin(egui::Margin {
                    left: 0,
                    right: 0,
                    top: SPACE_2 as i8,
                    bottom: 0,
                }),
        )
        .show(ui, |ui| draw_footer_pinned(ui, client));

    draw_body(ui, client, ui.available_height().max(0.0));
}

/// Dua kolom yang mengisi tinggi yang tersedia: daftar di kiri (scroll
/// sendiri) dan detail di kanan (scroll sendiri).
fn draw_body(ui: &mut egui::Ui, client: &mut ClientUi, height: f32) {
    let assets = filtered_assets(client);
    let left_width = (ui.available_width() * 0.36).clamp(260.0, 420.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(left_width, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| draw_list(ui, client, &assets),
        );
        ui.add_space(SPACE_3);
        ui.separator();
        ui.add_space(SPACE_3);
        let detail_width = ui.available_width();
        ui.allocate_ui_with_layout(
            egui::vec2(detail_width, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("asset-detail")
                    .max_height(ui.available_height())
                    .auto_shrink([false, false])
                    .show(ui, |ui| draw_detail(ui, client));
            },
        );
    });
}

fn draw_footer_pinned(ui: &mut egui::Ui, client: &mut ClientUi) {
    if client.role() != Role::Admin {
        ui.add_space(SPACE_2);
        ui.label(theme::muted(NON_ADMIN_NOTE));
        return;
    }
    // Form di-scroll bila tidak muat; baris tombol selalu di bagian bawah
    // panel sehingga tombol Upload tidak pernah terpotong.
    let actions_height = CONTROL_HEIGHT + SPACE_2;
    egui::ScrollArea::vertical()
        .id_salt("asset-upload-form")
        .max_height((ui.available_height() - actions_height).max(0.0))
        .auto_shrink([false, false])
        .show(ui, |ui| draw_upload_form(ui, client));
    draw_upload_actions(ui, client);
}

/// Versi tanpa pin untuk jendela pendek: seluruh halaman yang di-scroll.
fn draw_footer_inline(ui: &mut egui::Ui, client: &mut ClientUi) {
    if client.role() != Role::Admin {
        ui.add_space(SPACE_2);
        ui.label(theme::muted(NON_ADMIN_NOTE));
        return;
    }
    draw_upload_form(ui, client);
    draw_upload_actions(ui, client);
}

/// Blocking modal shown while the manifest is checked or files are downloaded.
pub fn draw_sync_overlay(context: &egui::Context, client: &mut ClientUi) {
    // Only block input when the user is waiting to enter the map; background
    // syncs show their state in the Aset toolbar instead.
    if client.pending_asset_nav.is_none() || !client.asset_sync.state.is_busy() {
        return;
    }
    egui::Window::new("Sinkronisasi aset")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(context, |ui| {
            ui.set_min_width(320.0);
            ui.label(match client.asset_sync.state {
                AssetSyncState::Checking => "Memeriksa daftar aset di server...",
                AssetSyncState::Syncing => "Mengunduh aset yang berubah...",
                _ => "Menyiapkan aset...",
            });
            ui.add_space(SPACE_2);
            match &client.asset_sync.progress {
                Some(progress) => {
                    let guard = progress.lock().expect("sync progress poisoned");
                    ui.add(
                        egui::ProgressBar::new(guard.fraction())
                            .show_percentage()
                            .desired_width(300.0),
                    );
                    ui.add_space(SPACE_2);
                    ui.label(theme::muted(&format!(
                        "{}/{}  {}",
                        guard.done, guard.total, guard.current
                    )));
                }
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(theme::muted("menghubungi server"));
                    });
                }
            }
        });
}

fn draw_toolbar(ui: &mut egui::Ui, client: &mut ClientUi) {
    ui.horizontal_wrapped(|ui| {
        if widgets::secondary_button(ui, "Muat ulang").clicked() {
            client.refresh_assets();
        }
        if widgets::secondary_button(ui, "Sinkronkan").clicked() {
            client.sync_assets_now();
        }
        ui.separator();
        let mut tab = match client.asset_filter {
            None => 0,
            Some(AssetCategory::Model) => 1,
            Some(AssetCategory::Texture) => 2,
            Some(AssetCategory::Icon) => 3,
        };
        widgets::tabs(ui, &mut tab, &["Semua", "Model", "Texture", "Icon"]);
        client.asset_filter = match tab {
            1 => Some(AssetCategory::Model),
            2 => Some(AssetCategory::Texture),
            3 => Some(AssetCategory::Icon),
            _ => None,
        };
        widgets::search_input(ui, &mut client.asset_search, "Cari nama aset...");
        let mut group_options = vec![(None, "Semua kategori".to_owned())];
        for group in &client.data.asset_groups {
            group_options.push((Some(group.id), group.name.clone()));
        }
        widgets::combo(
            ui,
            egui::Id::new("asset-group-filter"),
            &mut client.asset_group_filter,
            &group_options,
        );
        ui.separator();
        let (rgb, label) = match &client.asset_sync.state {
            AssetSyncState::Ready => (DARK.success, "Sinkron".to_owned()),
            AssetSyncState::Offline(error) => {
                (DARK.warning, format!("Offline ({})", truncate(error, 28)))
            }
            state if state.is_busy() => (DARK.warning, state.label().to_owned()),
            state => (DARK.muted_foreground, state.label().to_owned()),
        };
        widgets::badge(ui, &label, rgb);
    });
    if let Some(root) = client.asset_cache_root() {
        let last = client
            .asset_sync
            .last_sync
            .as_deref()
            .map(|time| format!(" · terakhir {time}"))
            .unwrap_or_default();
        ui.label(theme::muted(&format!(
            "Cache offline: {} aset · {root}{last}",
            client.cached_asset_count()
        )));
    } else {
        ui.label(theme::muted(
            "Cache offline hanya tersedia pada build desktop.",
        ));
    }
}

fn draw_rename(ui: &mut egui::Ui, client: &mut ClientUi, asset: &Asset) {
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::muted("Nama aset"));
        let response = ui.add(
            egui::TextEdit::singleline(&mut client.asset_edit_name)
                .desired_width(280.0)
                .margin(egui::Margin::symmetric(8, 5)),
        );
        let enter = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        let trimmed = client.asset_edit_name.trim().to_owned();
        let static_model =
            asset.category.is_model() && asset.model_kind_or_static() == AssetModelKind::Static;
        let mut group_options = client
            .data
            .asset_groups
            .iter()
            .map(|group| (Some(group.id), group.name.clone()))
            .collect::<Vec<_>>();
        if asset.group_id.is_none() {
            group_options.insert(0, (None, "Pilih kategori".to_owned()));
        }
        ui.label(theme::muted("Kategori"));
        widgets::combo(
            ui,
            egui::Id::new("asset-edit-group"),
            &mut client.asset_edit_group_id,
            &group_options,
        );
        if static_model {
            ui.label(theme::muted("Mode pindah"));
            let modes = move_mode_options();
            widgets::combo(
                ui,
                egui::Id::new("asset-edit-move-mode"),
                &mut client.asset_edit_move_mode,
                &modes,
            );
        }
        let changed = !trimmed.is_empty()
            && (trimmed != asset.name
                || client.asset_edit_group_id != asset.group_id
                || (static_model && client.asset_edit_move_mode != asset.move_mode));
        let saved = ui
            .add_enabled_ui(changed, |ui| {
                widgets::secondary_button(ui, "Simpan").clicked()
            })
            .inner;
        if let Some(()) = (changed && (saved || enter)).then_some(()) {
            client.begin_asset_rename(asset.id);
        }
        if changed && widgets::ghost_button(ui, "Batal").clicked() {
            client.asset_edit_name = asset.name.clone();
            client.asset_edit_group_id = asset.group_id;
            client.asset_edit_move_mode = asset.move_mode;
        }
    });
}

fn draw_replace_hint(ui: &mut egui::Ui, client: &mut ClientUi, slot: AssetSourceSlot) {
    ui.add_space(SPACE_2);
    egui::Frame::new()
        .fill(color_alpha(DARK.warning, 18))
        .stroke(egui::Stroke::new(1.0, color_alpha(DARK.warning, 90)))
        .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::muted(&format!(
                    "Siap mengganti file {}. Tarik file baru ke jendela aplikasi.",
                    slot_label(slot)
                )));
                if widgets::ghost_button(ui, "Batal").clicked() {
                    client.cancel_asset_replace();
                }
            });
        });
}

fn filtered_assets(client: &ClientUi) -> Vec<Asset> {
    let needle = client.asset_search.trim().to_lowercase();
    client
        .assets
        .iter()
        .filter(|asset| {
            client
                .asset_filter
                .is_none_or(|category| asset.category == category)
        })
        .filter(|asset| {
            client
                .asset_group_filter
                .is_none_or(|group_id| asset.group_id == Some(group_id))
        })
        .filter(|asset| {
            needle.is_empty()
                || asset.name.to_lowercase().contains(&needle)
                || asset
                    .sources
                    .iter()
                    .any(|source| source.file.filename.to_lowercase().contains(&needle))
        })
        .cloned()
        .collect()
}

fn draw_list(ui: &mut egui::Ui, client: &mut ClientUi, assets: &[Asset]) {
    ui.label(theme::muted(&format!("{} aset", assets.len())));
    ui.add_space(SPACE_2);
    egui::ScrollArea::vertical()
        .id_salt("asset-list")
        .max_height(ui.available_height().max(0.0))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for asset in assets {
                let selected = client.selected_asset == Some(asset.id);
                let cached = client.asset_is_cached(asset);
                let subtitle = format!(
                    "{} · {}{}",
                    category_label(asset.category),
                    asset.group_name.as_deref().unwrap_or("Tanpa kategori"),
                    if cached { " · offline" } else { "" }
                );
                let text = egui::RichText::new(format!("{}\n{}", asset.name, subtitle))
                    .size(FONT_SM)
                    .color(if selected {
                        color(DARK.foreground)
                    } else {
                        color(DARK.muted_foreground)
                    });
                let response = ui.add_sized(
                    [ui.available_width(), 50.0],
                    egui::Button::new(text)
                        .fill(if selected {
                            color(DARK.secondary)
                        } else {
                            color(DARK.card)
                        })
                        .stroke(egui::Stroke::new(
                            1.0,
                            color(if selected { DARK.ring } else { DARK.border }),
                        ))
                        .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8)),
                );
                if response.clicked() {
                    client.select_asset(asset.id);
                }
                ui.add_space(SPACE_2);
            }
        });
}

fn draw_detail(ui: &mut egui::Ui, client: &mut ClientUi) {
    let Some(asset_id) = client.selected_asset else {
        widgets::empty(
            ui,
            "Pilih aset di kiri untuk melihat preview, konversi, dan tombol Gunakan.",
        );
        return;
    };
    let Some(asset) = client
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .cloned()
    else {
        return;
    };
    let can_admin = client.role() == Role::Admin;

    ui.horizontal_wrapped(|ui| {
        ui.label(theme::title(&asset.name));
        ui.add_space(SPACE_2);
        widgets::badge(ui, category_label(asset.category), DARK.success);
        if let Some(group) = &asset.group_name {
            widgets::badge(ui, group, DARK.primary);
        } else {
            widgets::badge(ui, "Tanpa kategori", DARK.muted_foreground);
        }
        if let Some(kind) = asset.model_kind {
            widgets::badge(ui, model_kind_label(kind), DARK.warning);
        }
        if asset.category.is_model() {
            let (label, color) = match asset.move_mode {
                AssetMoveMode::None => ("tidak bisa pindah", DARK.muted_foreground),
                AssetMoveMode::Limited => ("pindah via gizmo", DARK.warning),
                AssetMoveMode::Free => ("pindah bebas", DARK.success),
            };
            widgets::badge(ui, label, color);
        }
        if client.asset_is_cached(&asset) {
            widgets::badge(ui, "offline siap", DARK.success);
        } else {
            widgets::badge(ui, "belum tersinkron", DARK.muted_foreground);
        }
    });
    ui.label(theme::muted(&format!(
        "id {} · versi {} · diperbarui {}",
        asset.id,
        asset.version,
        asset.updated_at.format("%d %b %Y %H:%M")
    )));
    if can_admin {
        ui.add_space(SPACE_2);
        draw_rename(ui, client, &asset);
    }
    if let Some(target) = client.asset_replace.clone()
        && target.asset_id == asset.id
    {
        draw_replace_hint(ui, client, target.slot);
    }
    ui.add_space(SPACE_3);

    client.ensure_asset_preview(ui.ctx());
    draw_preview(ui, client, &asset);

    let mut pending_convert: Option<(AssetSourceSlot, ConvertTarget, Option<u32>)> = None;
    let mut pending_replace = None;
    let mut pending_delete = false;
    let mut pending_release = false;
    for source in &asset.sources {
        draw_source(
            ui,
            source,
            &asset,
            can_admin,
            &mut pending_convert,
            &mut pending_replace,
        );
        ui.add_space(SPACE_2);
    }
    if let Some(slot) = pending_replace {
        client.begin_asset_replace(asset.id, slot);
    }

    ui.add_space(SPACE_2);
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(
            &mut client.asset_prefer_variants,
            "Gunakan hasil convert (Draco/LOD2) bila tersedia",
        );
        ui.add_space(SPACE_2);
        if widgets::primary_button(ui, "Gunakan").clicked() {
            let prefer = client.asset_prefer_variants;
            client.use_asset(asset.id, prefer);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if can_admin
            && widgets::secondary_button(ui, "Screenshot preview")
                .on_hover_text("Simpan gambar preview aset untuk tooltip di tab Assets editor")
                .clicked()
        {
            client.asset_preview_capture = Some(asset.id);
        }
        if can_admin && widgets::danger_button(ui, "Hapus aset").clicked() {
            pending_delete = true;
        }
        if client
            .used_asset
            .as_ref()
            .is_some_and(|used| used.asset_id == asset.id)
            && widgets::ghost_button(ui, "Lepas").clicked()
        {
            pending_release = true;
        }
    });

    if let Some(used) = &client.used_asset
        && used.asset_id == asset.id
    {
        draw_used(ui, used);
    }

    if let Some((slot, target, size)) = pending_convert {
        client.begin_asset_convert(asset.id, slot, target, size);
    }
    if pending_release {
        client.clear_used_asset();
    }
    if pending_delete {
        client.modal = Modal::Confirm(ConfirmDialog {
            title: "Hapus aset".to_owned(),
            message: format!(
                "Hapus aset '{}' beserta semua file dan variannya?",
                asset.name
            ),
            action: ConfirmAction::DeleteAsset(asset.id),
        });
    }
}

fn draw_preview(ui: &mut egui::Ui, client: &mut ClientUi, asset: &Asset) {
    let panels = client
        .preview
        .panels
        .iter()
        .filter(|panel| panel.asset_id == Some(asset.id))
        .cloned()
        .collect::<Vec<_>>();

    if panels.is_empty() {
        if client.preview_wanted.iter().any(Option::is_some) {
            widgets::loading(ui, "Memuat preview dari server...");
        } else if cfg!(target_arch = "wasm32") {
            ui.label(theme::muted("Preview hanya tersedia di desktop."));
        } else {
            ui.label(theme::muted("Preview tidak tersedia untuk aset ini."));
        }
        ui.add_space(SPACE_2);
        return;
    }

    let mut retry = false;
    let mut has_model = false;
    ui.horizontal_wrapped(|ui| {
        for panel in &panels {
            has_model |= panel.is_model;
            draw_preview_panel(ui, client, panel, &mut retry);
            ui.add_space(SPACE_3);
        }
    });
    if has_model {
        ui.label(theme::muted(
            "Drag gambar untuk memutar; model berputar otomatis saat tidak di-drag.",
        ));
        ui.add_space(SPACE_2);
    }
    if retry {
        client.preview_retry = client.preview_retry.wrapping_add(1);
        client.preview = PreviewUi::default();
        client.preview_orbit = [PreviewOrbitInput::default(); PREVIEW_SLOTS];
    }
}

fn draw_preview_panel(
    ui: &mut egui::Ui,
    client: &mut ClientUi,
    panel: &PreviewPanelUi,
    retry: &mut bool,
) {
    match panel.phase {
        PreviewPhase::Ready => {
            let Some(texture) = panel.texture else {
                widgets::loading(ui, &format!("Menyiapkan {}...", panel.label));
                return;
            };
            let (width, height) = panel.dimensions.unwrap_or((PREVIEW_SIZE, PREVIEW_SIZE));
            let scale = (240.0 / width.max(height).max(1) as f32).min(1.0);
            let desired = if panel.is_model {
                egui::vec2(240.0, 240.0)
            } else {
                egui::vec2(width as f32 * scale, height as f32 * scale)
            };
            let image = egui::Image::new((texture, desired));
            let image = if panel.is_model {
                image.sense(egui::Sense::drag())
            } else {
                image
            };
            egui::Frame::new()
                .fill(color(DARK.card))
                .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    let response = ui.add(image);
                    if panel.is_model {
                        if response.dragged() {
                            let drag = response.drag_delta();
                            let slot = panel.slot.min(PREVIEW_SLOTS - 1);
                            let orbit = &mut client.preview_orbit[slot];
                            orbit.delta_yaw -= drag.x * 0.01;
                            orbit.delta_pitch += drag.y * 0.01;
                            orbit.dragging = true;
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                        } else if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        }
                    }
                    if panel.is_model {
                        ui.label(theme::muted(&format!("{} · server", panel.label)));
                    } else {
                        ui.label(theme::muted(&format!("{} × {} px · server", width, height)));
                    }
                });
            ui.add_space(SPACE_2);
        }
        PreviewPhase::Failed => {
            ui.label(theme::danger(&format!(
                "{}: {}",
                panel.label,
                panel.error.as_deref().unwrap_or("gagal memuat dari server")
            )));
            ui.add_space(SPACE_2);
            if widgets::secondary_button(ui, "Coba lagi").clicked() {
                *retry = true;
            }
            ui.add_space(SPACE_2);
        }
        _ => {
            widgets::loading(ui, &format!("Memuat {}...", panel.label));
            ui.add_space(SPACE_2);
        }
    }
}

fn draw_source(
    ui: &mut egui::Ui,
    source: &AssetSource,
    asset: &Asset,
    can_admin: bool,
    pending_convert: &mut Option<(AssetSourceSlot, ConvertTarget, Option<u32>)>,
    pending_replace: &mut Option<AssetSourceSlot>,
) {
    widgets::card(ui, |ui| {
        ui.set_min_width(ui.available_width().min(460.0));
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::heading(slot_label(source.slot)));
            ui.label(theme::muted(&source.file.mime));
            ui.label(theme::muted(&human_size(source.file.size_bytes)));
            if source.has_draco {
                widgets::badge(ui, "draco", DARK.success);
            }
            if let Some(level) = source.lod {
                widgets::badge(ui, &format!("lod{level}"), DARK.warning);
            }
        });
        ui.label(theme::muted(&format!(
            "file {} · sha {}…",
            source.file.filename,
            source.file.sha256.chars().take(12).collect::<String>()
        )));

        if !source.variants.is_empty() {
            ui.add_space(SPACE_2);
            for variant in &source.variants {
                ui.horizontal(|ui| {
                    widgets::badge(ui, variant_label(variant.kind), DARK.success);
                    ui.label(theme::muted(&format!(
                        "{} · {}{}",
                        human_size(variant.size_bytes),
                        variant.filename,
                        if let (Some(width), Some(height)) = (variant.width, variant.height) {
                            format!(" · {width}×{height}")
                        } else {
                            String::new()
                        }
                    )));
                    if variant.reused {
                        widgets::badge(ui, "dari file asli", DARK.muted_foreground);
                    }
                });
            }
        }

        if !can_admin {
            return;
        }
        ui.add_space(SPACE_2);
        ui.horizontal_wrapped(|ui| {
            match asset.category {
                AssetCategory::Model => {
                    for (kind, target) in [
                        (AssetVariantKind::Draco, ConvertTarget::Draco),
                        (AssetVariantKind::Lod2, ConvertTarget::Lod2),
                        (AssetVariantKind::DracoLod2, ConvertTarget::DracoLod2),
                    ] {
                        let done = source.variant(kind).is_some();
                        let satisfied = source_satisfies(source, kind);
                        if done || satisfied {
                            ui.label(theme::success(&if done {
                                format!("✓ {}", variant_label(kind))
                            } else {
                                format!("✓ {} (dari upload)", variant_label(kind))
                            }));
                        } else if widgets::secondary_button(ui, variant_label(kind)).clicked() {
                            *pending_convert = Some((source.slot, target, None));
                        }
                    }
                }
                AssetCategory::Texture | AssetCategory::Icon => {
                    for side in [512u32, 256, 128] {
                        let kind = AssetVariantKind::from_resize_side(side).expect("known side");
                        let done = source.variant(kind).is_some();
                        if done {
                            ui.label(theme::success(&format!("✓ {side}×{side}")));
                        } else if widgets::secondary_button(ui, &format!("Resize {side}")).clicked()
                        {
                            *pending_convert =
                                Some((source.slot, ConvertTarget::Resize, Some(side)));
                        }
                    }
                }
            }
            ui.separator();
            if widgets::secondary_button(ui, "Ganti file").clicked() {
                *pending_replace = Some(source.slot);
            }
        });
    });
}

fn draw_used(ui: &mut egui::Ui, used: &UsedAsset) {
    ui.add_space(SPACE_2);
    egui::Frame::new()
        .fill(color_alpha(DARK.success, 18))
        .stroke(egui::Stroke::new(1.0, color_alpha(DARK.success, 90)))
        .corner_radius(egui::CornerRadius::same(RADIUS_SM as u8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.label(theme::success(&format!(
                "Dipakai: {} ({})",
                used.name,
                if used.prefer_variants {
                    "varian hasil convert"
                } else {
                    "file asli"
                }
            )));
            for file in &used.files {
                let variant = file.variant.map(variant_label).unwrap_or("original");
                ui.label(theme::muted(&format!(
                    "{} · {} · {}",
                    slot_label(file.slot),
                    variant,
                    file.path.display()
                )));
            }
        });
}

fn draw_upload_form(ui: &mut egui::Ui, client: &mut ClientUi) {
    ui.separator();
    ui.add_space(SPACE_2);
    ui.label(theme::heading("Unggah aset"));
    ui.label(theme::muted(
        "Tarik & lepas file ke jendela aplikasi. Model animasi membutuhkan dua file: idle dan animasi.",
    ));
    ui.add_space(SPACE_2);

    ui.horizontal_wrapped(|ui| {
        ui.label("Jenis file");
        let categories = [
            (AssetCategory::Model, "Model".to_owned()),
            (AssetCategory::Texture, "Texture".to_owned()),
            (AssetCategory::Icon, "Icon".to_owned()),
        ];
        if widgets::combo(
            ui,
            egui::Id::new("asset-upload-category"),
            &mut client.asset_upload.category,
            &categories,
        )
        .changed()
        {
            client.asset_upload.files.clear();
        }
        if client.asset_upload.category.is_model() {
            let kinds = [
                (AssetModelKind::Static, "Static".to_owned()),
                (AssetModelKind::Animated, "Animasi".to_owned()),
            ];
            if widgets::combo(
                ui,
                egui::Id::new("asset-upload-kind"),
                &mut client.asset_upload.model_kind,
                &kinds,
            )
            .changed()
            {
                client.asset_upload.files.clear();
                client.asset_upload.move_mode = AssetMoveMode::None;
            }
        }
        if client.asset_upload.category.is_model()
            && client.asset_upload.model_kind == AssetModelKind::Static
        {
            ui.label("Mode pindah");
            let modes = move_mode_options();
            widgets::combo(
                ui,
                egui::Id::new("asset-upload-move-mode"),
                &mut client.asset_upload.move_mode,
                &modes,
            );
        }
        widgets::labeled_input_with_width(ui, "Nama", &mut client.asset_upload.name, 240.0);
    });

    ui.horizontal_wrapped(|ui| {
        ui.label("Kategori aset");
        let mut groups = client
            .data
            .asset_groups
            .iter()
            .map(|group| (Some(group.id), group.name.clone()))
            .collect::<Vec<_>>();
        if groups.is_empty() {
            ui.label(theme::danger(
                "Belum ada kategori. Buat dulu di menu Kategori Aset.",
            ));
        } else {
            groups.insert(0, (None, "Pilih kategori".to_owned()));
            widgets::combo(
                ui,
                egui::Id::new("asset-upload-group"),
                &mut client.asset_upload.group_id,
                &groups,
            );
        }
    });

    let slots = client.asset_upload.required_files().to_vec();
    for (index, slot) in slots.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{}:", slot_label(*slot)))
                    .size(FONT_XS)
                    .color(color(DARK.muted_foreground)),
            );
            match client.asset_upload.files.get(index) {
                Some(path) => {
                    ui.label(
                        egui::RichText::new(
                            path.file_name()
                                .and_then(|value| value.to_str())
                                .unwrap_or("file"),
                        )
                        .size(FONT_SM),
                    );
                }
                None => {
                    ui.label(theme::muted("(belum ada — tarik file ke jendela)"));
                }
            }
        });
    }

    if let Some(error) = &client.asset_upload.error {
        widgets::error_banner(ui, error);
    }
}

fn draw_upload_actions(ui: &mut egui::Ui, client: &mut ClientUi) {
    ui.horizontal(|ui| {
        let label = if client.asset_upload.busy {
            "Mengunggah..."
        } else {
            "Upload"
        };
        let ready = client.asset_upload.group_id.is_some() && !client.asset_upload.busy;
        let clicked = ui
            .add_enabled_ui(ready, |ui| widgets::primary_button(ui, label).clicked())
            .inner;
        if clicked {
            client.begin_asset_upload();
        }
        if widgets::ghost_button(ui, "Reset").clicked() {
            client.clear_asset_upload();
        }
        if client.asset_upload.group_id.is_none() {
            ui.label(theme::muted("Pilih kategori aset dulu."));
        }
    });
}

fn source_satisfies(source: &AssetSource, kind: AssetVariantKind) -> bool {
    if kind.is_draco() && !source.has_draco {
        return false;
    }
    if kind.is_lod2() && source.lod != Some(2) {
        return false;
    }
    kind.is_draco() || kind.is_lod2()
}

fn category_label(category: AssetCategory) -> &'static str {
    match category {
        AssetCategory::Model => "Model",
        AssetCategory::Texture => "Texture",
        AssetCategory::Icon => "Icon",
    }
}

fn model_kind_label(kind: AssetModelKind) -> &'static str {
    match kind {
        AssetModelKind::Static => "static",
        AssetModelKind::Animated => "animasi",
    }
}

fn move_mode_options() -> [(AssetMoveMode, String); 3] {
    [
        (AssetMoveMode::None, "Tidak bisa pindah".to_owned()),
        (AssetMoveMode::Limited, "Bisa pindah (gizmo)".to_owned()),
        (AssetMoveMode::Free, "Bisa pindah bebas".to_owned()),
    ]
}

fn slot_label(slot: AssetSourceSlot) -> &'static str {
    match slot {
        AssetSourceSlot::Static => "File",
        AssetSourceSlot::Idle => "Idle",
        AssetSourceSlot::Animated => "Animasi",
    }
}

fn variant_label(kind: AssetVariantKind) -> &'static str {
    match kind {
        AssetVariantKind::Draco => "Draco",
        AssetVariantKind::Lod2 => "LOD2",
        AssetVariantKind::DracoLod2 => "Draco+LOD2",
        AssetVariantKind::Resize512 => "512",
        AssetVariantKind::Resize256 => "256",
        AssetVariantKind::Resize128 => "128",
    }
}

fn human_size(bytes: i64) -> String {
    let bytes = bytes.max(0) as f64;
    if bytes >= 1024.0 * 1024.0 {
        format!("{:.1} MB", bytes / (1024.0 * 1024.0))
    } else if bytes >= 1024.0 {
        format!("{:.1} KB", bytes / 1024.0)
    } else {
        format!("{} B", bytes as i64)
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_owned()
    } else {
        format!("{}…", value.chars().take(max).collect::<String>())
    }
}
