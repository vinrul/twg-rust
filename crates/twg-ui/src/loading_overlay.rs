use bevy_egui::egui;
use twg_client::design::{DARK, FONT_SM, FONT_XS};
use twg_client::session::Screen;

use crate::state::ClientUi;
use crate::theme;

/// Compact, non-blocking loading bar for map-asset and scenario loading. It is
/// shared by the editor, Simulasi play, and the big-screen display so all three
/// show the same feedback while markers, animations, and tiles resolve.
pub fn draw(context: &egui::Context, client: &ClientUi) {
    if !matches!(
        client.state.screen,
        Screen::MapEditor | Screen::Preview | Screen::BigScreen
    ) {
        return;
    }

    let scenario_loading = client.editor.loading;
    let render = client.load_status;
    let sync_busy = client.asset_sync.state.is_busy();
    let sync_progress = sync_busy.then(|| {
        client.asset_sync.progress.as_ref().and_then(|progress| {
            progress.lock().ok().map(|progress| {
                (
                    progress.fraction(),
                    format!(
                        "{} · {}/{}",
                        progress.current, progress.done, progress.total
                    ),
                )
            })
        })
    });
    if !scenario_loading && !render.is_loading() && !sync_busy {
        return;
    }

    let (title, detail, fraction) = if scenario_loading {
        const STEPS: f32 = 6.0;
        let done = (STEPS - client.editor.pending as f32).clamp(0.0, STEPS);
        (
            "Memuat skenario...",
            format!("{done:.0} dari {STEPS:.0} bagian"),
            done / STEPS,
        )
    } else if sync_busy {
        let (fraction, detail) = sync_progress
            .flatten()
            .unwrap_or((0.0, client.asset_sync.state.label().to_owned()));
        ("Menyiapkan aset...", detail, fraction)
    } else {
        ("Memuat aset peta...", render.detail(), render.fraction())
    };

    egui::Area::new(egui::Id::new("map_loading_overlay"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -28.0))
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(context, |ui| {
            egui::Frame::new()
                .fill(theme::color(DARK.card))
                .stroke(egui::Stroke::new(1.0, theme::color(DARK.border)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_min_width(320.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(egui::RichText::new(title).size(FONT_SM).strong());
                    });
                    ui.add_space(4.0);
                    ui.add(
                        egui::ProgressBar::new(fraction.clamp(0.0, 1.0))
                            .desired_width(320.0)
                            .text(format!("{:.0}%", fraction.clamp(0.0, 1.0) * 100.0)),
                    );
                    if !detail.is_empty() {
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(detail)
                                .size(FONT_XS)
                                .color(theme::color(DARK.muted_foreground)),
                        );
                    }
                });
        });
}
