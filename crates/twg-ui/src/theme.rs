use bevy_egui::egui;
use twg_client::design::{
    DARK, DesignTokens, FONT_BASE, FONT_LG, FONT_SM, FONT_XL, FONT_XS, RADIUS_LG, RADIUS_MD,
    RADIUS_SM, Rgb, SPACE_2, SPACE_3,
};

pub fn color(rgb: Rgb) -> egui::Color32 {
    egui::Color32::from_rgb(rgb.0, rgb.1, rgb.2)
}

pub fn color_alpha(rgb: Rgb, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(rgb.0, rgb.1, rgb.2, alpha)
}

pub fn apply(context: &egui::Context) {
    context.set_theme(egui::Theme::Dark);
    let mut style = (*context.style_of(egui::Theme::Dark)).clone();
    style.visuals = visuals(DARK);
    style.spacing.item_spacing = egui::vec2(SPACE_2, SPACE_2);
    style.spacing.button_padding = egui::vec2(SPACE_3, SPACE_2);
    style.spacing.interact_size = egui::vec2(0.0, twg_client::design::CONTROL_HEIGHT);
    style.spacing.window_margin = egui::Margin::same(SPACE_3 as i8);
    style.spacing.menu_margin = egui::Margin::same(SPACE_2 as i8);
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(FONT_XL),
    );
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(FONT_BASE));
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(FONT_BASE),
    );
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(FONT_XS));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(FONT_SM));
    context.set_style_of(egui::Theme::Dark, style);
}

fn visuals(tokens: DesignTokens) -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = color(tokens.background);
    visuals.window_fill = color(tokens.card);
    visuals.extreme_bg_color = color(tokens.background);
    visuals.faint_bg_color = color(tokens.secondary);
    visuals.code_bg_color = color(tokens.muted);
    visuals.override_text_color = Some(color(tokens.foreground));
    visuals.hyperlink_color = color(tokens.success);
    visuals.warn_fg_color = color(tokens.warning);
    visuals.error_fg_color = color(tokens.destructive);
    visuals.selection.bg_fill = color_alpha(tokens.ring, 90);
    visuals.selection.stroke = egui::Stroke::new(1.0, color(tokens.foreground));
    visuals.window_corner_radius = egui::CornerRadius::same(RADIUS_LG as u8);
    visuals.window_stroke = egui::Stroke::new(1.0, color(tokens.border));
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: egui::Color32::from_black_alpha(120),
    };
    visuals.menu_corner_radius = egui::CornerRadius::same(RADIUS_MD as u8);
    visuals.widgets.noninteractive.bg_fill = color(tokens.card);
    visuals.widgets.noninteractive.weak_bg_fill = color(tokens.card);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, color(tokens.border));
    visuals.widgets.noninteractive.fg_stroke =
        egui::Stroke::new(1.0, color(tokens.muted_foreground));
    visuals.widgets.inactive.bg_fill = color(tokens.secondary);
    visuals.widgets.inactive.weak_bg_fill = color(tokens.secondary);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, color(tokens.border));
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, color(tokens.foreground));
    visuals.widgets.hovered.bg_fill = color(tokens.accent);
    visuals.widgets.hovered.weak_bg_fill = color(tokens.accent);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, color(tokens.ring));
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, color(tokens.foreground));
    visuals.widgets.active.bg_fill = color(tokens.muted);
    visuals.widgets.active.weak_bg_fill = color(tokens.muted);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, color(tokens.ring));
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, color(tokens.foreground));
    visuals.widgets.open.bg_fill = color(tokens.accent);
    visuals.widgets.open.weak_bg_fill = color(tokens.accent);
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, color(tokens.ring));
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, color(tokens.foreground));
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(RADIUS_SM as u8);
    }
    visuals
}

pub fn heading(text: &str) -> egui::RichText {
    egui::RichText::new(text).size(FONT_XL).strong()
}

pub fn title(text: &str) -> egui::RichText {
    egui::RichText::new(text).size(FONT_LG).strong()
}

pub fn muted(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .size(FONT_SM)
        .color(color(DARK.muted_foreground))
}

pub fn danger(text: &str) -> egui::RichText {
    egui::RichText::new(text).color(color(DARK.destructive))
}

pub fn success(text: &str) -> egui::RichText {
    egui::RichText::new(text).color(color(DARK.success))
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(color(DARK.card))
        .stroke(egui::Stroke::new(1.0, color(DARK.border)))
        .corner_radius(egui::CornerRadius::same(RADIUS_LG as u8))
        .inner_margin(egui::Margin::same(SPACE_3 as i8))
}
