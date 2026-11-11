//! Admin-managed semantic categories for the asset library.

use bevy_egui::egui;
use twg_client::design::{DARK, SPACE_2, SPACE_3};
use twg_domain::Role;

use crate::state::{ClientUi, ConfirmAction, ConfirmDialog, Modal};
use crate::theme;
use crate::widgets;

pub fn draw(ui: &mut egui::Ui, client: &mut ClientUi) {
    widgets::section(
        ui,
        "Kategori Aset",
        Some(
            "Kelompokkan model, texture, dan icon berdasarkan jenisnya (Personel, Bangunan, \
             Kendaraan Bermotor, dan sebagainya). Setiap unggahan aset wajib memilih kategori.",
        ),
    );

    let admin = client.role() == Role::Admin;
    ui.horizontal_wrapped(|ui| {
        if widgets::secondary_button(ui, "Muat ulang").clicked() {
            client.refresh_assets();
        }
        if !admin {
            ui.label(theme::muted(
                "Hanya admin yang dapat membuat, mengubah, atau menghapus kategori.",
            ));
        }
    });

    if admin {
        ui.add_space(SPACE_3);
        if client.asset_group_edit_id.is_none() {
            widgets::card(ui, |ui| {
                ui.label(theme::heading("Kategori baru"));
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::muted("Nama"));
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut client.asset_group_new_name)
                            .desired_width(260.0)
                            .margin(egui::Margin::symmetric(8, 5)),
                    );
                    let enter = response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    let valid = !client.asset_group_new_name.trim().is_empty();
                    let clicked = ui
                        .add_enabled_ui(valid, |ui| widgets::primary_button(ui, "Tambah").clicked())
                        .inner;
                    if valid && (clicked || enter) {
                        client.begin_asset_group_save();
                    }
                });
            });
        }
    }

    if let Some(error) = &client.asset_group_error {
        ui.add_space(SPACE_2);
        widgets::error_banner(ui, error);
    }

    ui.add_space(SPACE_3);
    if client.data.asset_groups.is_empty() {
        widgets::empty(ui, "Belum ada kategori aset.");
        return;
    }

    let groups = client.data.asset_groups.clone();
    for group in groups {
        let count = client
            .assets
            .iter()
            .filter(|asset| asset.group_id == Some(group.id))
            .count();
        let editing = client.asset_group_edit_id == Some(group.id);
        egui::Frame::new()
            .fill(theme::color(DARK.card))
            .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                if editing {
                    ui.horizontal_wrapped(|ui| {
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut client.asset_group_edit_name)
                                .desired_width(260.0)
                                .margin(egui::Margin::symmetric(8, 5)),
                        );
                        let enter = response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter));
                        let valid = !client.asset_group_edit_name.trim().is_empty();
                        if valid && (widgets::primary_button(ui, "Simpan").clicked() || enter) {
                            client.begin_asset_group_save();
                        }
                        if widgets::ghost_button(ui, "Batal").clicked() {
                            client.cancel_asset_group_edit();
                        }
                    });
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(theme::title(&group.name));
                        widgets::badge(ui, &format!("{count} aset"), DARK.muted_foreground);
                        if admin {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if widgets::danger_button(ui, "Hapus").clicked() {
                                        client.modal = Modal::Confirm(ConfirmDialog {
                                            title: "Hapus kategori aset".to_owned(),
                                            message: format!(
                                                "Hapus kategori '{}'? Aset di dalamnya tetap ada \
                                                 dan menjadi tanpa kategori.",
                                                group.name
                                            ),
                                            action: ConfirmAction::DeleteAssetGroup(group.id),
                                        });
                                    }
                                    if widgets::secondary_button(ui, "Ubah").clicked() {
                                        client.begin_asset_group_edit(group.id, &group.name);
                                    }
                                },
                            );
                        }
                    });
                }
            });
        ui.add_space(SPACE_2);
    }
}
