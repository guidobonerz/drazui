use crate::geom::{vec2, Color, Vec2};

/// All look-and-feel knobs. Every field is public: tweak them live via `ui.theme`.
/// Metrics are logical units and get multiplied by `scale`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Global zoom of the whole UI (1.0 = tiny, 2.0 = double size, ...).
    pub scale: f32,
    pub font_size: f32,
    /// Padding inside widgets.
    pub pad: Vec2,
    /// Spacing between widgets.
    pub gap: f32,
    /// Padding inside containers (frames, docks, accordion bodies).
    pub margin: f32,
    /// Indentation of tree children.
    pub indent: f32,
    /// Scrollbar thickness.
    pub scrollbar: f32,

    pub bg: Color,
    pub panel: Color,
    pub widget: Color,
    pub hover: Color,
    pub active: Color,
    /// Background of inputs, lists and tables.
    pub field: Color,
    pub border: Color,
    pub text: Color,
    pub text_dim: Color,
    /// Check marks, focus, drag handles.
    pub accent: Color,
    /// Selected rows and items.
    pub select: Color,
    /// Text on top of `select`.
    pub select_text: Color,
    pub stripe: Color,
}

/// Default font size: 11 for the built-in vector font (`ttf` feature),
/// 8 for the 8x8 bitmap font.
const FONT_SIZE: f32 = if cfg!(feature = "ttf") { 11.0 } else { 8.0 };

impl Default for Theme {
    fn default() -> Self {
        Theme::dark()
    }
}

impl Theme {
    /// Derives a full theme from four colors: background, field (inputs/lists),
    /// accent and selection.
    pub fn palette(bg: Color, field: Color, accent: Color, select: Color) -> Theme {
        let ink = Theme::ink;
        let fg = ink(bg);
        Theme {
            scale: 1.0,
            font_size: FONT_SIZE,
            pad: vec2(4.0, 3.0),
            gap: 2.0,
            margin: 4.0,
            indent: 10.0,
            scrollbar: 6.0,
            bg,
            panel: bg.mix(fg, 0.04),
            widget: bg.mix(fg, 0.12),
            hover: bg.mix(fg, 0.22),
            active: bg.mix(fg, 0.32),
            field,
            border: bg.mix(fg, 0.28),
            text: fg,
            text_dim: fg.mix(bg, 0.45),
            accent,
            select,
            select_text: ink(select),
            stripe: field.mix(ink(field), 0.06),
        }
    }

    /// Light or dark text color, whichever is readable on `background`.
    pub fn ink(background: Color) -> Color {
        let c = background;
        let luma = c.r() as u32 * 3 + c.g() as u32 * 6 + c.b() as u32;
        if luma < 1275 {
            Color::hex(0xe4e6e3)
        } else {
            Color::hex(0x101010)
        }
    }

    /// Default: anthracite surfaces, black fields, green accent, orange selection.
    pub fn dark() -> Theme {
        Theme::palette(Color::hex(0x2a2d30), Color::hex(0x000000), Color::hex(0x3ddc5a), Color::hex(0xff8a00))
    }

    pub fn light() -> Theme {
        Theme::palette(Color::hex(0xe9e9e6), Color::hex(0xffffff), Color::hex(0x1f9d3a), Color::hex(0xff8a00))
    }

    /// Height of a standard widget row.
    pub fn row_h(&self) -> f32 {
        self.font_size + 2.0 * self.pad.y
    }
}
