//! Software rasterizer: renders a frame into a `0x00RRGGBB` pixel buffer.
//! Text uses the font of the [`Ui`] (see `Ui::set_font`) or, without one,
//! a monospace 8x8 bitmap font.

#[cfg(feature = "ttf")]
pub use crate::font::Font;
use crate::{Color, Draw, Rect, Ui};
use font8x8::{UnicodeFonts, BASIC_FONTS, LATIN_FONTS};

/// Double-buffered render target. Frames are drawn into a hidden back buffer
/// and swapped into `buf` only when complete, so `buf` never holds a
/// half-drawn frame. Frames identical to the previous one are not redrawn.
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    /// Front buffer: the last complete frame. Row-major pixels, `0x00RRGGBB`.
    pub buf: Vec<u32>,
    /// Back buffer: the frame being drawn.
    back: Vec<u32>,
    /// Fingerprint of the frame in `buf`; `None` forces a redraw.
    shown: Option<u64>,
}

type Span = [i32; 4]; // x0, y0, x1, y1

fn span(r: Rect) -> Span {
    [r.x as i32, r.y as i32, (r.x + r.w) as i32, (r.y + r.h) as i32]
}

fn isect(a: Span, b: Span) -> Span {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])]
}

fn rgb(c: Color) -> u32 {
    c.0 >> 8
}

pub(crate) fn glyph(ch: char) -> [u8; 8] {
    BASIC_FONTS.get(ch).or_else(|| LATIN_FONTS.get(ch)).unwrap_or([0x7e, 0x42, 0x42, 0x42, 0x42, 0x42, 0x7e, 0])
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas {
            w,
            h,
            buf: vec![0; w * h],
            back: vec![0; w * h],
            shown: None,
        }
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        if (w, h) != (self.w, self.h) {
            self.w = w;
            self.h = h;
            self.buf.resize(w * h, 0);
            self.back.resize(w * h, 0);
            self.shown = None;
        }
    }

    /// Forces the next [`render`](Canvas::render) to redraw, e.g. after
    /// writing to `buf` directly.
    pub fn invalidate(&mut self) {
        self.shown = None;
    }

    /// Fingerprint of everything that determines the rendered pixels.
    fn fingerprint(&self, ui: &Ui) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut add = |v: u32| h = (h ^ v as u64).wrapping_mul(0x0100_0000_01b3);
        #[cfg(feature = "ttf")]
        add(ui.font().map_or(0, |f| f.key() as u32));
        for d in ui.draws() {
            let (tag, r, color) = match d {
                Draw::Clip(r) => (1, r, Color(0)),
                Draw::Rect { rect, color } => (2, rect, color),
                Draw::Text { pos, text, size, color } => {
                    text.bytes().for_each(|b| add(b as u32));
                    (3, Rect::new(pos.x, pos.y, size, text.len() as f32), color)
                }
            };
            [tag, r.x.to_bits(), r.y.to_bits(), r.w.to_bits(), r.h.to_bits(), color.0].into_iter().for_each(&mut add);
        }
        h
    }

    /// Renders the last finished frame of `ui` into the back buffer and swaps
    /// it to the front (`buf`). Returns false, without drawing anything, if
    /// the frame is identical to the one already in `buf`.
    pub fn render(&mut self, ui: &Ui) -> bool {
        let print = self.fingerprint(ui);
        if self.shown == Some(print) {
            return false;
        }
        let full = [0, 0, self.w as i32, self.h as i32];
        let mut clip = full;
        self.back.fill(0);
        #[cfg(feature = "ttf")]
        let font = ui.font();
        for d in ui.draws() {
            match d {
                Draw::Clip(r) => clip = isect(span(r), full),
                Draw::Rect { rect, color } => self.fill(isect(span(rect), clip), color),
                Draw::Text { pos, text, size, color } => {
                    #[cfg(feature = "ttf")]
                    if let Some(font) = font {
                        font.draw(&mut self.back, self.w, clip, pos.x as i32, pos.y as i32, text, size, color);
                        continue;
                    }
                    self.text(pos.x as i32, pos.y as i32, text, size, color, clip)
                }
            }
        }
        std::mem::swap(&mut self.buf, &mut self.back);
        self.shown = Some(print);
        true
    }

    fn fill(&mut self, [x0, y0, x1, y1]: Span, c: Color) {
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let a = c.a() as u32;
        for y in y0..y1 {
            let row = &mut self.back[y as usize * self.w + x0 as usize..y as usize * self.w + x1 as usize];
            if a == 255 {
                row.fill(rgb(c));
            } else {
                for p in row {
                    let ch = |s: u8, shift: u32| ((s as u32 * a + (*p >> shift & 0xff) * (255 - a)) / 255) << shift;
                    *p = ch(c.r(), 16) | ch(c.g(), 8) | ch(c.b(), 0);
                }
            }
        }
    }

    fn text(&mut self, x: i32, y: i32, text: &str, size: f32, color: Color, clip: Span) {
        let cell = size.round().max(1.0) as i32;
        if y >= clip[3] || y + cell <= clip[1] {
            return;
        }
        let px = rgb(color);
        let mut gx = x;
        for ch in text.chars() {
            if gx >= clip[2] {
                break;
            }
            if gx + cell > clip[0] && ch != ' ' {
                let g = glyph(ch);
                for dy in 0..cell {
                    let py = y + dy;
                    let bits = g[(dy * 8 / cell) as usize];
                    if bits == 0 || py < clip[1] || py >= clip[3] {
                        continue;
                    }
                    for dx in 0..cell {
                        let qx = gx + dx;
                        if bits >> (dx * 8 / cell) & 1 != 0 && qx >= clip[0] && qx < clip[2] {
                            self.back[py as usize * self.w + qx as usize] = px;
                        }
                    }
                }
            }
            gx += cell;
        }
    }
}
