use gpui::*;
use gpui_terminal::ColorPalette;

pub fn catppuccin_mocha() -> ColorPalette {
    ColorPalette::builder()
        .background(0x1e, 0x1e, 0x2e)
        .foreground(0xcd, 0xd6, 0xf4)
        .cursor(0xf5, 0xe0, 0xdc)
        .black(0x45, 0x47, 0x5a)
        .red(0xf3, 0x8b, 0xa8)
        .green(0xa6, 0xe3, 0xa1)
        .yellow(0xf9, 0xe2, 0xaf)
        .blue(0x89, 0xb4, 0xfa)
        .magenta(0xf5, 0xc2, 0xe7)
        .cyan(0x94, 0xe2, 0xd5)
        .white(0xba, 0xc2, 0xde)
        .bright_black(0x58, 0x5b, 0x70)
        .bright_red(0xf3, 0x8b, 0xa8)
        .bright_green(0xa6, 0xe3, 0xa1)
        .bright_yellow(0xf9, 0xe2, 0xaf)
        .bright_blue(0x89, 0xb4, 0xfa)
        .bright_magenta(0xf5, 0xc2, 0xe7)
        .bright_cyan(0x94, 0xe2, 0xd5)
        .bright_white(0xa6, 0xad, 0xc8)
        .build()
}

// UI chrome colors (Catppuccin Mocha)
pub const BG_PRIMARY: u32 = 0x1e1e2e;
pub const BG_SECONDARY: u32 = 0x181825;
pub const BG_SURFACE: u32 = 0x11111b;
pub const BG_HOVER: u32 = 0x313244;
pub const TEXT_PRIMARY: u32 = 0xcdd6f4;
pub const TEXT_SECONDARY: u32 = 0xa6adc8;
pub const TEXT_DIM: u32 = 0x6c7086;
pub const ACCENT: u32 = 0x89b4fa;
pub const ACCENT_GREEN: u32 = 0xa6e3a1;
pub const ACCENT_RED: u32 = 0xf38ba8;
pub const ACCENT_YELLOW: u32 = 0xf9e2af;
pub const BORDER: u32 = 0x313244;
pub const DIVIDER: u32 = 0x45475a;

pub const WORKSPACE_COLORS: [u32; 8] = [
    0x89b4fa, // Blue
    0xa6e3a1, // Green
    0xf38ba8, // Red
    0xf9e2af, // Yellow
    0xf5c2e7, // Pink
    0x94e2d5, // Teal
    0xfab387, // Peach
    0xcba6f7, // Mauve
];

// Unicode icons for UI buttons
pub mod icons {
    pub const PLUS: &str = "+";
    pub const CLOSE: &str = "\u{2715}";          // ✕
    pub const RENAME: &str = "\u{270E}";          // ✎
    pub const SPLIT_H: &str = "\u{25EB}";         // ◫
    pub const SPLIT_V: &str = "\u{229F}";         // ⊟
    pub const MAXIMIZE: &str = "\u{2922}";         // ⤢
    pub const MINIMIZE: &str = "\u{2921}";         // ⤡
    pub const BELL: &str = "\u{2691}";             // ⚑
    pub const PALETTE: &str = "\u{2318}";          // ⌘
    pub const CHEVRON_LEFT: &str = "\u{25C2}";     // ◂
    pub const CHEVRON_RIGHT: &str = "\u{25B8}";    // ▸
    pub const CLOSE_OTHERS: &str = "\u{2298}";     // ⊘
    pub const AGENT: &str = "\u{2726}";            // ✦
    pub const OVERFLOW: &str = "\u{22EF}";         // ⋯
    pub const SEARCH: &str = "\u{2315}";           // ⌕
}

/// Reusable icon button builder with consistent styling.
pub fn icon_button(id: impl Into<ElementId>, icon: &str) -> Stateful<Div> {
    div()
        .id(id.into())
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(3.0))
        .text_size(px(14.0))
        .text_color(rgb(TEXT_DIM))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(BG_HOVER)).text_color(rgb(TEXT_PRIMARY)))
        .child(icon.to_string())
}

/// Icon button with active/highlighted state.
pub fn icon_button_active(id: impl Into<ElementId>, icon: &str) -> Stateful<Div> {
    div()
        .id(id.into())
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(3.0))
        .text_size(px(14.0))
        .bg(rgb(ACCENT))
        .text_color(rgb(BG_PRIMARY))
        .cursor_pointer()
        .child(icon.to_string())
}

pub fn hsla_from_rgb(hex: u32) -> Hsla {
    let r = ((hex >> 16) & 0xff) as f32 / 255.0;
    let g = ((hex >> 8) & 0xff) as f32 / 255.0;
    let b = (hex & 0xff) as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let l = (max + min) / 2.0;

    let s = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * l - 1.0).abs())
    };

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let h = if h < 0.0 { h + 360.0 } else { h } / 360.0;

    Hsla { h, s, l, a: 1.0 }
}
