//! Design tokens ported from the legacy Tailwind/shadcn dark theme.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn hex(self) -> u32 {
        ((self.0 as u32) << 16) | ((self.1 as u32) << 8) | self.2 as u32
    }

    pub const fn with_alpha(self, alpha: u8) -> [u8; 4] {
        [self.0, self.1, self.2, alpha]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignTokens {
    pub background: Rgb,
    pub foreground: Rgb,
    pub card: Rgb,
    pub card_foreground: Rgb,
    pub primary: Rgb,
    pub primary_foreground: Rgb,
    pub secondary: Rgb,
    pub secondary_foreground: Rgb,
    pub muted: Rgb,
    pub muted_foreground: Rgb,
    pub accent: Rgb,
    pub accent_foreground: Rgb,
    pub destructive: Rgb,
    pub border: Rgb,
    pub input: Rgb,
    pub ring: Rgb,
    pub sidebar: Rgb,
    pub sidebar_foreground: Rgb,
    pub sidebar_accent: Rgb,
    pub success: Rgb,
    pub warning: Rgb,
}

/// Dark palette matching shadcn's `.dark` tokens (oklch values converted to
/// sRGB approximations).
pub const DARK: DesignTokens = DesignTokens {
    background: Rgb(0x0a, 0x0a, 0x0a),
    foreground: Rgb(0xfa, 0xfa, 0xfa),
    card: Rgb(0x17, 0x17, 0x17),
    card_foreground: Rgb(0xfa, 0xfa, 0xfa),
    primary: Rgb(0xe5, 0xe5, 0xe5),
    primary_foreground: Rgb(0x17, 0x17, 0x17),
    secondary: Rgb(0x26, 0x26, 0x26),
    secondary_foreground: Rgb(0xfa, 0xfa, 0xfa),
    muted: Rgb(0x26, 0x26, 0x26),
    muted_foreground: Rgb(0xa3, 0xa3, 0xa3),
    accent: Rgb(0x26, 0x26, 0x26),
    accent_foreground: Rgb(0xfa, 0xfa, 0xfa),
    destructive: Rgb(0xf8, 0x71, 0x71),
    border: Rgb(0x2e, 0x2e, 0x2e),
    input: Rgb(0x33, 0x33, 0x33),
    ring: Rgb(0x73, 0x73, 0x73),
    sidebar: Rgb(0x17, 0x17, 0x17),
    sidebar_foreground: Rgb(0xfa, 0xfa, 0xfa),
    sidebar_accent: Rgb(0x26, 0x26, 0x26),
    success: Rgb(0x4a, 0xde, 0x80),
    warning: Rgb(0xfb, 0xbf, 0x24),
};

/// Radius scale in points (`--radius: 0.625rem` → 10 px).
pub const RADIUS_SM: f32 = 6.0;
pub const RADIUS_MD: f32 = 8.0;
pub const RADIUS_LG: f32 = 10.0;
pub const RADIUS_XL: f32 = 14.0;

pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_6: f32 = 24.0;
pub const SPACE_8: f32 = 32.0;

pub const FONT_XS: f32 = 11.0;
pub const FONT_SM: f32 = 13.0;
pub const FONT_BASE: f32 = 15.0;
pub const FONT_LG: f32 = 18.0;
pub const FONT_XL: f32 = 22.0;
pub const FONT_2XL: f32 = 28.0;

pub const SIDEBAR_WIDTH: f32 = 208.0;
pub const MOBILE_BREAKPOINT: f32 = 760.0;
pub const CONTROL_HEIGHT: f32 = 30.0;

#[cfg(test)]
mod tests {
    use super::{DARK, RADIUS_LG, Rgb, SPACE_4};

    #[test]
    fn hex_packs_channels() {
        assert_eq!(Rgb(0x12, 0x34, 0x56).hex(), 0x123456);
    }

    #[test]
    fn dark_tokens_match_legacy_contrast() {
        assert!(DARK.foreground.hex() > DARK.background.hex());
        assert!(DARK.card.hex() > DARK.background.hex());
        assert_eq!(RADIUS_LG, 10.0);
        assert_eq!(SPACE_4, 16.0);
    }
}
