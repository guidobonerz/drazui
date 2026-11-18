#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

impl Vec2 {
    pub const ZERO: Vec2 = vec2(0.0, 0.0);
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const ZERO: Rect = Rect::new(0.0, 0.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.right() && p.y < self.bottom()
    }
    pub fn overlaps(&self, o: Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
    pub fn intersect(&self, o: Rect) -> Rect {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let w = (self.right().min(o.right()) - x).max(0.0);
        let h = (self.bottom().min(o.bottom()) - y).max(0.0);
        Rect::new(x, y, w, h)
    }
    /// Shrinks the rect by `d` on every side.
    pub fn inset(&self, d: f32) -> Rect {
        self.inset_xy(d, d)
    }
    pub fn inset_xy(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, (self.w - 2.0 * dx).max(0.0), (self.h - 2.0 * dy).max(0.0))
    }
}

/// 32-bit color, `0xRRGGBBAA`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    /// Opaque color from `0xRRGGBB`.
    pub const fn hex(rgb: u32) -> Color {
        Color(rgb << 8 | 0xff)
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color((r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | a as u32)
    }
    pub const fn r(self) -> u8 {
        (self.0 >> 24) as u8
    }
    pub const fn g(self) -> u8 {
        (self.0 >> 16) as u8
    }
    pub const fn b(self) -> u8 {
        (self.0 >> 8) as u8
    }
    pub const fn a(self) -> u8 {
        self.0 as u8
    }
    pub const fn alpha(self, a: u8) -> Color {
        Color(self.0 & !0xff | a as u32)
    }
    /// Parses `#RGB`, `#RRGGBB` or `#RRGGBBAA` (the `#` is optional).
    pub fn parse(s: &str) -> Option<Color> {
        let h = s.trim().trim_start_matches('#');
        if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let v = u32::from_str_radix(h, 16).ok()?;
        match h.len() {
            3 => Some(Color::hex((v >> 8 & 15) * 0x11_0000 | (v >> 4 & 15) * 0x1100 | (v & 15) * 0x11)),
            6 => Some(Color::hex(v)),
            8 => Some(Color(v)),
            _ => None,
        }
    }

    /// `#rrggbb`, or `#rrggbbaa` if not opaque.
    pub fn to_hex(self) -> String {
        if self.a() == 255 {
            format!("#{:06x}", self.0 >> 8)
        } else {
            format!("#{:08x}", self.0)
        }
    }

    /// Linear blend towards `o` (`t` = 0 → self, 1 → `o`).
    pub fn mix(self, o: Color, t: f32) -> Color {
        let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8;
        Color::rgba(f(self.r(), o.r()), f(self.g(), o.g()), f(self.b(), o.b()), f(self.a(), o.a()))
    }
}
