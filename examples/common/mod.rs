//! Window loop shared by the examples.
//!
//! Default: minifb window + software renderer.
//! With `--features gpu`: miniquad window + OpenGL renderer.
//!
//! Command line options handled here:
//!   --font <file> [size]          use another .ttf/.otf/.woff font
//!   --bitmap                      use the 8x8 bitmap font
//!   --soft                        software renderer even if the gpu feature is on
//!   --shot <out.png> [steps]...   render one 960x600 image without a visible
//!                                 UI loop; steps are `x y` (click at device
//!                                 pixel), `rx y` (right click) or `t:text`

use drazui::soft::Canvas;
use drazui::*;
use std::sync::mpsc;

const SIZE: (usize, usize) = (960, 600);

/// Opens a window and calls `build` once per frame between `begin` and `end`.
pub fn run(title: &str, mut ui: Ui, build: impl FnMut(&mut Ui) + 'static) {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--font") {
        let path = args.get(i + 1).expect("--font <file> [size]").clone();
        let size = args.get(i + 2).and_then(|s| s.parse().ok());
        args.drain(i..(i + 2 + size.is_some() as usize).min(args.len()));
        load_font(&mut ui, &path);
        ui.theme.font_size = size.unwrap_or(11.0);
    }
    if let Some(i) = args.iter().position(|a| a == "--bitmap") {
        args.remove(i);
        #[cfg(feature = "ttf")]
        ui.set_font(None);
        ui.theme.font_size = 8.0;
    }
    let soft = args.iter().position(|a| a == "--soft").map(|i| args.remove(i)).is_some();
    let shot = (args.first().map(String::as_str) == Some("--shot")).then(|| args[1..].to_vec());

    #[cfg(feature = "gpu")]
    if !soft {
        return gpu::run(title, ui, build, shot);
    }
    let _ = soft;
    run_soft(title, ui, build, shot);
}

#[cfg(feature = "ttf")]
fn load_font(ui: &mut Ui, path: &str) {
    let font = drazui::soft::Font::open(path).unwrap_or_else(|e| panic!("{e}"));
    ui.set_font(Some(font));
}

#[cfg(not(feature = "ttf"))]
fn load_font(_: &mut Ui, _: &str) {
    panic!("--font needs the ttf feature");
}

/// Runs the scripted `--shot` steps, calling `frame` for every UI frame.
fn script(ui: &mut Ui, steps: &[String], frame: &mut dyn FnMut(&mut Ui)) {
    frame(ui);
    let mut steps = steps.iter();
    while let Some(step) = steps.next() {
        if let Some(text) = step.strip_prefix("t:") {
            ui.input.text.push_str(text);
            frame(ui);
        } else {
            let right = step.starts_with('r');
            let y = steps.next().expect("click needs x and y");
            ui.input.mouse = vec2(step.trim_start_matches('r').parse().expect("x"), y.parse().expect("y"));
            for down in [false, true, false] {
                ui.input.down = down && !right;
                ui.input.right = down && right;
                frame(ui);
            }
        }
    }
    frame(ui);
    frame(ui);
}

// ---- software renderer + minifb ---------------------------------------------

/// Display scaling factor (1.5 at 150 %). minifb does not declare the process
/// DPI-aware; Windows would then render the window at 96 dpi and stretch it,
/// which blurs everything. Declaring awareness here turns that off.
#[cfg(windows)]
fn dpi_scale() -> f32 {
    #[link(name = "user32")]
    extern "system" {
        fn SetProcessDPIAware() -> i32;
        fn GetDpiForSystem() -> u32;
    }
    // SAFETY: plain Win32 calls without arguments.
    unsafe {
        SetProcessDPIAware();
        GetDpiForSystem() as f32 / 96.0
    }
}

#[cfg(not(windows))]
fn dpi_scale() -> f32 {
    1.0
}

/// System clipboard for the minifb window, which has no clipboard API.
#[cfg(windows)]
mod clip {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(owner: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn GetClipboardData(format: u32) -> *mut c_void;
        fn SetClipboardData(format: u32, mem: *mut c_void) -> *mut c_void;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalLock(mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(mem: *mut c_void) -> i32;
        fn GlobalFree(mem: *mut c_void) -> *mut c_void;
    }
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 2;

    pub fn get() -> Option<String> {
        // SAFETY: the handle is only read while locked and the clipboard is
        // open; CF_UNICODETEXT data is a NUL-terminated UTF-16 string.
        unsafe {
            if OpenClipboard(null_mut()) == 0 {
                return None;
            }
            let mut text = None;
            let mem = GetClipboardData(CF_UNICODETEXT);
            if !mem.is_null() {
                let p = GlobalLock(mem) as *const u16;
                if !p.is_null() {
                    let len = (0..).take_while(|&i| *p.add(i) != 0).count();
                    text = Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, len)));
                    GlobalUnlock(mem);
                }
            }
            CloseClipboard();
            text
        }
    }

    pub fn set(text: &str) {
        let wide: Vec<u16> = text.replace('\n', "\r\n").encode_utf16().chain([0]).collect();
        // SAFETY: the block is large enough for `wide`; the clipboard owns it
        // once SetClipboardData succeeds, otherwise it is freed here.
        unsafe {
            if OpenClipboard(null_mut()) == 0 {
                return;
            }
            EmptyClipboard();
            let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
            if !mem.is_null() {
                let p = GlobalLock(mem) as *mut u16;
                if p.is_null() {
                    GlobalFree(mem);
                } else {
                    std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                    GlobalUnlock(mem);
                    if SetClipboardData(CF_UNICODETEXT, mem).is_null() {
                        GlobalFree(mem);
                    }
                }
            }
            CloseClipboard();
        }
    }
}

/// Elsewhere copy and paste stay inside the application.
#[cfg(not(windows))]
mod clip {
    pub fn get() -> Option<String> {
        None
    }
    pub fn set(_: &str) {}
}

struct Chars(mpsc::Sender<u32>);

impl minifb::InputCallback for Chars {
    fn add_char(&mut self, c: u32) {
        let _ = self.0.send(c);
    }
}

fn run_soft(title: &str, mut ui: Ui, mut build: impl FnMut(&mut Ui), shot: Option<Vec<String>>) {
    let (mut w, mut h) = SIZE;
    let mut canvas = Canvas::new(w, h);
    let mut frame = |ui: &mut Ui, canvas: &mut Canvas| {
        ui.begin(canvas.w as f32, canvas.h as f32);
        build(ui);
        ui.end();
        canvas.render(ui);
    };
    if let Some(args) = shot {
        script(&mut ui, &args[1..], &mut |ui| frame(ui, &mut canvas));
        return std::fs::write(&args[0], png(canvas.w, canvas.h, &canvas.buf)).expect("write png");
    }

    // Pixels must reach the screen 1:1, so the window grows with the display
    // scaling instead of being stretched by the system.
    let k = dpi_scale();
    (w, h) = ((w as f32 * k) as usize, (h as f32 * k) as usize);
    canvas.resize(w, h);
    let opts = minifb::WindowOptions { resize: true, ..Default::default() };
    let mut window = minifb::Window::new(title, w, h, opts).expect("window");
    window.set_target_fps(60);
    let (tx, rx) = mpsc::channel();
    window.set_input_callback(Box::new(Chars(tx)));

    let mut last_frame = std::time::Instant::now();
    while window.is_open() {
        (w, h) = window.get_size();
        if w == 0 || h == 0 {
            window.update();
            continue;
        }
        canvas.resize(w, h);
        if let Some((x, y)) = window.get_mouse_pos(minifb::MouseMode::Pass) {
            ui.input.mouse = vec2(x, y);
        }
        ui.input.down = window.get_mouse_down(minifb::MouseButton::Left);
        ui.input.right = window.get_mouse_down(minifb::MouseButton::Right);
        ui.input.dt = last_frame.elapsed().as_secs_f32();
        last_frame = std::time::Instant::now();
        if let Some((x, y)) = window.get_scroll_wheel() {
            // wheel units differ per platform, so only the direction is used
            let dir = |v: f32| if v == 0.0 { 0.0 } else { v.signum() };
            ui.input.scroll = vec2(dir(x), dir(y));
        }
        use minifb::Key as K;
        let ctrl = window.is_key_down(K::LeftCtrl) || window.is_key_down(K::RightCtrl);
        ui.input.shift = window.is_key_down(K::LeftShift) || window.is_key_down(K::RightShift);
        for k in window.get_keys_pressed(minifb::KeyRepeat::Yes) {
            ui.input.keys.push(match k {
                K::A if ctrl => Key::SelectAll,
                K::C if ctrl => Key::Copy,
                K::X if ctrl => Key::Cut,
                K::V if ctrl => {
                    if let Some(text) = clip::get() {
                        ui.clipboard = text;
                    }
                    Key::Paste
                }
                K::Left => Key::Left,
                K::Right => Key::Right,
                K::Up => Key::Up,
                K::Down => Key::Down,
                K::PageUp => Key::PageUp,
                K::PageDown => Key::PageDown,
                K::Home => Key::Home,
                K::End => Key::End,
                K::Backspace => Key::Backspace,
                K::Delete => Key::Delete,
                K::Enter => Key::Enter,
                K::Escape => Key::Escape,
                _ => continue,
            });
        }
        ui.input.text.extend(rx.try_iter().filter_map(char::from_u32));

        frame(&mut ui, &mut canvas);
        if let Some(text) = ui.take_copied() {
            clip::set(text);
        }
        window.update_with_buffer(&canvas.buf, w, h).expect("present");
    }
}

// ---- OpenGL renderer + miniquad ----------------------------------------------

#[cfg(feature = "gpu")]
mod gpu {
    use super::{png, script, SIZE};
    use drazui::gpu::Renderer;
    use drazui::{vec2, Key, Ui};
    use miniquad::*;

    struct Stage<B> {
        ctx: Box<dyn RenderingBackend>,
        renderer: Renderer,
        ui: Ui,
        build: B,
        shot: Option<Vec<String>>,
        last_frame: f64,
    }

    pub fn run(title: &str, ui: Ui, build: impl FnMut(&mut Ui) + 'static, shot: Option<Vec<String>>) {
        let conf = conf::Conf {
            window_title: title.to_owned(),
            window_width: SIZE.0 as i32,
            window_height: SIZE.1 as i32,
            high_dpi: true,
            ..Default::default()
        };
        miniquad::start(conf, move || {
            let mut ctx = window::new_rendering_backend();
            let renderer = Renderer::new(&mut *ctx);
            Box::new(Stage { ctx, renderer, ui, build, shot, last_frame: date::now() })
        });
    }

    impl<B: FnMut(&mut Ui)> Stage<B> {
        /// Renders into an offscreen texture, reads it back and saves it.
        fn screenshot(&mut self, args: &[String]) {
            let (w, h) = SIZE;
            let Stage { ctx, renderer, ui, build, .. } = self;
            script(ui, &args[1..], &mut |ui| {
                ui.begin(w as f32, h as f32);
                build(ui);
                ui.end();
            });
            let params = TextureParams { width: w as u32, height: h as u32, format: TextureFormat::RGBA8, ..Default::default() };
            let target = ctx.new_render_texture(params);
            let pass = ctx.new_render_pass(target, None);
            ctx.begin_pass(Some(pass), PassAction::clear_color(0.0, 0.0, 0.0, 1.0));
            renderer.render(&mut **ctx, ui, w as f32, h as f32);
            ctx.end_render_pass();
            let mut bytes = vec![0u8; w * h * 4];
            ctx.texture_read_pixels(target, &mut bytes);
            // GL rows are bottom-up
            let pixels: Vec<u32> = bytes
                .chunks(w * 4)
                .rev()
                .flat_map(|row| row.chunks(4).map(|p| (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32))
                .collect();
            std::fs::write(&args[0], png(w, h, &pixels)).expect("write png");
        }
    }

    impl<B: FnMut(&mut Ui)> EventHandler for Stage<B> {
        fn update(&mut self) {}

        fn draw(&mut self) {
            if let Some(args) = self.shot.take() {
                self.screenshot(&args);
                return window::order_quit();
            }
            let (w, h) = window::screen_size();
            let now = date::now();
            self.ui.input.dt = (now - self.last_frame) as f32;
            self.last_frame = now;

            self.ui.begin(w, h);
            (self.build)(&mut self.ui);
            self.ui.end();
            if let Some(text) = self.ui.take_copied() {
                window::clipboard_set(text);
            }

            self.ctx.begin_default_pass(PassAction::clear_color(0.0, 0.0, 0.0, 1.0));
            self.renderer.render(&mut *self.ctx, &self.ui, w, h);
            self.ctx.end_render_pass();
            self.ctx.commit_frame();
        }

        fn mouse_motion_event(&mut self, x: f32, y: f32) {
            self.ui.input.mouse = vec2(x, y);
        }

        fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
            self.ui.input.mouse = vec2(x, y);
            match button {
                MouseButton::Left => self.ui.input.down = true,
                MouseButton::Right => self.ui.input.right = true,
                _ => {}
            }
        }

        fn mouse_button_up_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
            match button {
                MouseButton::Left => self.ui.input.down = false,
                MouseButton::Right => self.ui.input.right = false,
                _ => {}
            }
        }

        fn mouse_wheel_event(&mut self, x: f32, y: f32) {
            // wheel units differ per platform, so only the direction is used
            let dir = |v: f32| if v == 0.0 { 0.0 } else { v.signum() };
            self.ui.input.scroll = vec2(dir(x), dir(y));
        }

        fn char_event(&mut self, ch: char, mods: KeyMods, _repeat: bool) {
            if !ch.is_control() && !mods.ctrl {
                self.ui.input.text.push(ch);
            }
        }

        fn key_up_event(&mut self, _key: KeyCode, mods: KeyMods) {
            self.ui.input.shift = mods.shift;
        }

        fn key_down_event(&mut self, key: KeyCode, mods: KeyMods, _repeat: bool) {
            self.ui.input.shift = mods.shift;
            self.ui.input.keys.push(match key {
                KeyCode::A if mods.ctrl => Key::SelectAll,
                KeyCode::C if mods.ctrl => Key::Copy,
                KeyCode::X if mods.ctrl => Key::Cut,
                KeyCode::V if mods.ctrl => {
                    if let Some(text) = window::clipboard_get() {
                        self.ui.clipboard = text;
                    }
                    Key::Paste
                }
                KeyCode::Left => Key::Left,
                KeyCode::Right => Key::Right,
                KeyCode::Up => Key::Up,
                KeyCode::Down => Key::Down,
                KeyCode::PageUp => Key::PageUp,
                KeyCode::PageDown => Key::PageDown,
                KeyCode::Home => Key::Home,
                KeyCode::End => Key::End,
                KeyCode::Backspace => Key::Backspace,
                KeyCode::Delete => Key::Delete,
                KeyCode::Enter | KeyCode::KpEnter => Key::Enter,
                KeyCode::Escape => Key::Escape,
                _ => return,
            });
        }
    }
}

// ---- screenshots -------------------------------------------------------------

/// Minimal uncompressed PNG encoder for `0x00RRGGBB` pixels.
fn png(w: usize, h: usize, pixels: &[u32]) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend(tag);
        out.extend(data);
        let mut crc = !0u32;
        for b in &out[start..] {
            crc ^= *b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { 0xEDB8_8320 ^ (crc >> 1) } else { crc >> 1 };
            }
        }
        out.extend((!crc).to_be_bytes());
    }
    let mut raw = Vec::with_capacity((w * 3 + 1) * h);
    for row in pixels.chunks(w) {
        raw.push(0);
        for p in row {
            raw.extend([(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
        }
    }
    let mut z = vec![0x78, 0x01];
    let (mut a, mut b) = (1u32, 0u32);
    let blocks = raw.chunks(65535).count();
    for (i, block) in raw.chunks(65535).enumerate() {
        z.push((i + 1 == blocks) as u8);
        z.extend((block.len() as u16).to_le_bytes());
        z.extend((!(block.len() as u16)).to_le_bytes());
        z.extend(block);
        for v in block {
            a = (a + *v as u32) % 65521;
            b = (b + a) % 65521;
        }
    }
    z.extend((b << 16 | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend((w as u32).to_be_bytes());
    ihdr.extend((h as u32).to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}
