#![cfg(feature = "ttf")]

use drazui::soft::{Canvas, Font};
use drazui::*;

fn be(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

/// Repacks an sfnt file as WOFF 1 with zlib-compressed tables.
fn to_woff(sfnt: &[u8]) -> Vec<u8> {
    let u32_at = |i: usize| u32::from_be_bytes(sfnt[i..i + 4].try_into().unwrap());
    let n = u16::from_be_bytes([sfnt[4], sfnt[5]]) as usize;
    let mut dir = Vec::new();
    let mut body = Vec::new();
    for i in 0..n {
        let e = 12 + 16 * i;
        let (off, len) = (u32_at(e + 8) as usize, u32_at(e + 12) as usize);
        let raw = &sfnt[off..off + len];
        let z = miniz_oxide::deflate::compress_to_vec_zlib(raw, 6);
        let data = if z.len() < len { &z[..] } else { raw };
        dir.extend(&sfnt[e..e + 4]);
        dir.extend(be((44 + 20 * n + body.len()) as u32));
        dir.extend(be(data.len() as u32));
        dir.extend(be(len as u32));
        dir.extend(&sfnt[e + 4..e + 8]);
        body.extend(data);
        body.resize(body.len().next_multiple_of(4), 0);
    }
    let mut out = b"wOFF".to_vec();
    out.extend(&sfnt[..4]);
    out.extend(be((44 + dir.len() + body.len()) as u32));
    out.extend((n as u16).to_be_bytes());
    out.extend([0; 30]);
    out.extend(dir);
    out.extend(body);
    out
}

fn lit_pixels(font: Font) -> usize {
    let mut ui = Ui::new();
    ui.set_font(Some(font));
    ui.theme.font_size = 12.0;
    let mut canvas = Canvas::new(200, 40);
    ui.begin(200.0, 40.0);
    ui.label("Hamburg");
    ui.end();
    canvas.render(&ui);
    let bg = Theme::dark().bg.0 >> 8;
    canvas.buf.iter().filter(|p| **p != bg).count()
}

#[test]
fn builtin_font_is_the_default() {
    let ui = Ui::new();
    let builtin = Font::builtin();
    assert_eq!(ui.theme.font_size, 11.0);
    assert!(ui.font().is_some());
    assert_eq!(ui.text_width("Hamburg"), builtin.width("Hamburg", 11.0));
    assert!(ui.text_width("iii") < ui.text_width("WWW"), "proportional");
    assert!(lit_pixels(builtin) > 50);

    let mut bitmap = Ui::with_bitmap_font();
    assert!(bitmap.font().is_none());
    assert_eq!(bitmap.text_width("abc"), 24.0);
    bitmap.set_font(Some(Font::builtin()));
    bitmap.set_font(None);
    assert_eq!(bitmap.text_width("abc"), 24.0);
}

#[test]
fn rejects_bad_data() {
    assert!(Font::load(b"wOF2....").is_err());
    assert!(Font::load(b"wOFF").is_err());
    assert!(Font::load(b"junk").is_err());
}

/// Uses system fonts; formats whose sample file is missing are skipped.
#[test]
fn truetype_opentype_woff() {
    let mut tested = 0;
    for path in ["C:/Windows/Fonts/segoeui.ttf", "C:/Windows/Fonts/DavidCLM-Medium.otf"] {
        let Ok(data) = std::fs::read(path) else { continue };
        tested += 1;
        let font = Font::load(&data).unwrap();
        let w = font.width("Hamburg", 12.0);
        assert!(w > 20.0 && w < 80.0, "{path}: width {w}");
        assert!(font.width("iii", 12.0) < font.width("WWW", 12.0), "{path}: proportional");
        let lit = lit_pixels(font);
        assert!(lit > 50, "{path}: {lit} text pixels");

        let woff = Font::load(&to_woff(&data)).unwrap();
        assert_eq!(woff.width("Hamburg", 12.0), w, "{path} as woff");
        assert_eq!(lit_pixels(woff), lit, "{path} as woff");
    }
    println!("font files tested: {tested}");
}
