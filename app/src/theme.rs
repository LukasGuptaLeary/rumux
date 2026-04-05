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
