//! Image helpers shared by the editor UI.
//!
//! Texture assets now come from the server library (admin uploads or the
//! scenario editor's Import tab), so only the egui preview decoder lives here.

use std::path::Path;

use bevy_egui::egui;

#[cfg(not(target_arch = "wasm32"))]
const PREVIEW_MAX_SIDE: u32 = 256;

/// Decodes an image file into an egui texture for the Inspector preview.
pub fn load_preview(ctx: &egui::Context, path: &Path) -> Option<egui::TextureHandle> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let image = image::open(path)
            .ok()?
            .thumbnail(PREVIEW_MAX_SIDE, PREVIEW_MAX_SIDE)
            .to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let pixels = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        Some(ctx.load_texture(
            format!("texture-preview:{}", path.display()),
            pixels,
            egui::TextureOptions::LINEAR,
        ))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (ctx, path);
        None
    }
}
