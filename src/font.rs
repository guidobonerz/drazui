//! Vector fonts for the software backend: TrueType (.ttf/.ttc), OpenType
//! (.otf) and WOFF 1 (.woff).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use crate::Color;

pub(crate) struct Glyph {
    pub xmin: i32,
    pub ymin: i32,
    pub w: usize,
    pub h: usize,
    pub advance: f32,
    /// Coverage, one byte per pixel.
    pub cov: Vec<u8>,
}

struct Inner {
    font: fontdue::Font,
    /// Rasterized glyphs per (char, pixel size).
    glyphs: RefCell<HashMap<(char, u32), Rc<Glyph>>>,
}

/// A loaded font. Cloning is cheap and shares the glyph cache.
///
/// ```no_run
/// # use drazui::{soft::{Canvas, Font}, Ui};
/// # let (mut ui, mut canvas) = (Ui::new(), Canvas::new(1, 1));
/// let font = Font::open("C:/Windows/Fonts/segoeui.ttf").unwrap();
/// ui.set_font(Some(font));       // layout and rendering use it from now on
/// ui.theme.font_size = 12.0;
/// # let _ = &mut canvas;
/// ```
#[derive(Clone)]
pub struct Font(Rc<Inner>);

fn be32(d: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?) as usize)
}

/// Unpacks a WOFF 1 container into a plain sfnt (TTF/OTF) file.
fn unwoff(d: &[u8]) -> Option<Vec<u8>> {
    let n = u16::from_be_bytes(d.get(12..14)?.try_into().ok()?) as usize;
    let mut head = Vec::with_capacity(12 + 16 * n);
    head.extend(&d[4..8]); // flavor
    head.extend((n as u16).to_be_bytes());
    head.extend([0; 6]); // search hints, unused by parsers
    let mut body = Vec::new();
    for i in 0..n {
        let e = 44 + 20 * i;
        let (off, comp, orig) = (be32(d, e + 4)?, be32(d, e + 8)?, be32(d, e + 12)?);
        let raw = d.get(off..off.checked_add(comp)?)?;
        let table = if comp < orig { miniz_oxide::inflate::decompress_to_vec_zlib(raw).ok()? } else { raw.to_vec() };
        if table.len() != orig {
            return None;
        }
        head.extend(&d[e..e + 4]); // tag
        head.extend(&d[e + 16..e + 20]); // checksum
        head.extend(((12 + 16 * n + body.len()) as u32).to_be_bytes());
        head.extend((orig as u32).to_be_bytes());
        body.extend(table);
        body.resize(body.len().next_multiple_of(4), 0);
    }
    head.extend(body);
    Some(head)
}

impl Font {
    /// The built-in default font (Univers Condensed), shared per thread.
    pub fn builtin() -> Font {
        thread_local! {
            static FONT: Font = Font::load(include_bytes!("../39335_UniversCondensed.ttf")).expect("built-in font");
        }
        FONT.with(Font::clone)
    }

    /// Loads a font from the bytes of a .ttf, .ttc, .otf or .woff file.
    pub fn load(data: &[u8]) -> Result<Font, String> {
        let unpacked;
        let sfnt = match data.get(..4) {
            Some(b"wOFF") => {
                unpacked = unwoff(data).ok_or("corrupt WOFF file")?;
                &unpacked[..]
            }
            Some(b"wOF2") => return Err("WOFF2 is not supported, use .woff, .ttf or .otf".into()),
            _ => data,
        };
        let font = fontdue::Font::from_bytes(sfnt, fontdue::FontSettings::default())?;
        Ok(Font(Rc::new(Inner { font, glyphs: RefCell::default() })))
    }

    /// Loads a font file from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Font, String> {
        let path = path.as_ref();
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Font::load(&data)
    }

    /// Text measuring function for [`Ui::measure`](crate::Ui::measure).
    pub fn measure(&self) -> Box<dyn Fn(&str, f32) -> f32> {
        let font = self.clone();
        Box::new(move |s, size| font.width(s, size))
    }

    /// Identity of the underlying font (clones share it).
    pub(crate) fn key(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }

    /// Width of `s` in pixels at `size`.
    pub fn width(&self, s: &str, size: f32) -> f32 {
        s.chars().map(|ch| self.glyph(ch, size).advance).sum()
    }

    pub(crate) fn glyph(&self, ch: char, size: f32) -> Rc<Glyph> {
        let px = size.round().max(1.0);
        let mut cache = self.0.glyphs.borrow_mut();
        let g = cache.entry((ch, px as u32)).or_insert_with(|| {
            let (m, cov) = self.0.font.rasterize(ch, px);
            Rc::new(Glyph { xmin: m.xmin, ymin: m.ymin, w: m.width, h: m.height, advance: m.advance_width, cov })
        });
        g.clone()
    }

    /// Distance from the top of a `size` high text box to the baseline.
    pub(crate) fn baseline(&self, size: f32) -> i32 {
        let px = size.round().max(1.0);
        let (asc, desc) = self.0.font.horizontal_line_metrics(px).map_or((px * 0.8, -px * 0.2), |m| (m.ascent, m.descent));
        // center the ascent..descent box in the text box
        ((px + asc + desc) / 2.0).round() as i32
    }

    /// Draws `text` with its top-left at (x, y) into a `0x00RRGGBB` buffer.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(&self, buf: &mut [u32], stride: usize, clip: [i32; 4], x: i32, y: i32, text: &str, size: f32, color: Color) {
        let base = y + self.baseline(size);
        let (r, g, b) = (color.r() as u32, color.g() as u32, color.b() as u32);
        let mut pen = x as f32;
        for ch in text.chars() {
            if pen as i32 >= clip[2] {
                break;
            }
            let gl = self.glyph(ch, size);
            let gx = pen.round() as i32 + gl.xmin;
            let gy = base - gl.ymin - gl.h as i32;
            for row in 0..gl.h {
                let py = gy + row as i32;
                if py < clip[1] || py >= clip[3] {
                    continue;
                }
                for col in 0..gl.w {
                    let qx = gx + col as i32;
                    let a = gl.cov[row * gl.w + col] as u32;
                    if a == 0 || qx < clip[0] || qx >= clip[2] {
                        continue;
                    }
                    let p = &mut buf[py as usize * stride + qx as usize];
                    let mix = |s: u32, shift: u32| ((s * a + (*p >> shift & 0xff) * (255 - a)) / 255) << shift;
                    *p = mix(r, 16) | mix(g, 8) | mix(b, 0);
                }
            }
            pen += gl.advance;
        }
    }
}
