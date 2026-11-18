use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[cfg(feature = "ttf")]
use crate::font::Font;
use crate::geom::{vec2, Color, Rect, Vec2};
use crate::input::Input;
use crate::layout::{Kind, Layout, Size};
use crate::theme::Theme;
use crate::widgets::Toast;

/// Widget identity. Derived from the label and the id stack ([`Ui::push_id`]).
pub type Id = u64;

#[derive(Clone, Copy)]
pub(crate) enum Cmd {
    Rect(Rect, Color),
    Text { pos: Vec2, size: f32, color: Color, start: usize, end: usize },
    Clip(Rect),
}

/// One draw primitive in device pixels. This is all a backend has to render.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Draw<'a> {
    /// Filled rectangle.
    Rect { rect: Rect, color: Color },
    /// Text with its top-left at `pos`, `size` pixels high.
    Text { pos: Vec2, text: &'a str, size: f32, color: Color },
    /// Replaces the scissor rectangle for all following primitives.
    Clip(Rect),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Result of [`Ui::interact`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Resp {
    pub hovered: bool,
    /// Mouse button went down on the widget this frame.
    pub pressed: bool,
    /// Mouse button is held since it went down on the widget.
    pub held: bool,
    /// Mouse button was released over the widget after pressing it.
    pub clicked: bool,
    /// Second press of a double-click landed on the widget this frame.
    pub double_clicked: bool,
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

struct Fnv(u64);

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ *b as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// Derives a child id.
pub(crate) fn sub(id: Id, n: u64) -> Id {
    let mut h = Fnv(id ^ FNV_OFFSET);
    h.write(&n.to_le_bytes());
    h.0.max(1)
}

/// Text width of the monospace 8x8 bitmap font.
fn bitmap_width(s: &str, size: f32) -> f32 {
    s.chars().count() as f32 * size.round()
}

pub struct Ui {
    pub theme: Theme,
    pub input: Input,
    /// Text width in device pixels for `(text, size)`. Kept in sync by
    /// [`set_font`](Ui::set_font); replace it only when you render text
    /// yourself with a font drazui does not know about.
    pub measure: Box<dyn Fn(&str, f32) -> f32>,
    #[cfg(feature = "ttf")]
    font: Option<Font>,
    /// How long a toast stays visible, in seconds.
    pub toast_secs: f32,
    /// Clipboard text. [`Key::Copy`](crate::Key::Copy) and `Key::Cut` write
    /// the selection here, `Key::Paste` inserts it. To share it with the
    /// system clipboard, set it before sending `Key::Paste` and forward
    /// [`Ui::take_copied`] after each frame.
    pub clipboard: String,

    // per frame
    pub(crate) screen: Rect,
    pub(crate) mouse: Vec2,
    pub(crate) mouse_delta: Vec2,
    pub(crate) pressed: bool,
    pub(crate) released: bool,
    was_down: bool,
    /// The press of this frame is the second one of a double-click.
    pub(crate) dbl: bool,
    /// Seconds since the first frame.
    time: f32,
    /// Time and position of the previous press.
    last_press: (f32, Vec2),
    /// Table cell being edited and its text.
    pub(crate) edit: Id,
    pub(crate) edit_buf: String,
    /// Right mouse button went down this frame.
    pub(crate) rpressed: bool,
    was_right: bool,
    /// Rect of the most recently placed widget or container.
    pub(crate) last: Rect,
    pub(crate) cmds: Vec<Cmd>,
    pub(crate) overlay: Vec<Cmd>,
    text: String,
    pub(crate) clips: Vec<Rect>,
    pub(crate) layouts: Vec<Layout>,
    pub(crate) ids: Vec<Id>,
    pub(crate) next: [Option<Size>; 2],

    // persistent
    pub(crate) active: Id,
    pub(crate) focus: Id,
    pub(crate) caret: usize,
    /// Other end of the text selection (equal to `caret`: nothing selected).
    pub(crate) sel: usize,
    /// Focus at the end of the previous frame (a press clears `focus`).
    pub(crate) prev_focus: Id,
    pub(crate) copied: bool,
    pub(crate) edit_scroll: f32,
    pub(crate) open: Id,
    /// Id scope of the menubar the open menu belongs to (0 = none).
    pub(crate) open_group: Id,
    /// Popup opened this frame; it must not be closed by the opening click.
    pub(crate) just_opened: Id,
    /// The open modal dialog (0 = none).
    pub(crate) modal: Id,
    /// The dialog was opened this frame; the keys of this frame are not for it.
    pub(crate) modal_opened: bool,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) popup_prev: Rect,
    pub(crate) popup_cur: Rect,
    pub(crate) in_popup: bool,
    store: HashMap<Id, Vec2>,
}

impl Default for Ui {
    fn default() -> Self {
        Ui::new()
    }
}

impl Ui {
    /// A new context using the default font: the built-in Univers Condensed
    /// with the `ttf` feature, the 8x8 bitmap font without it.
    pub fn new() -> Ui {
        #[allow(unused_mut)]
        let mut ui = Ui::with_bitmap_font();
        #[cfg(feature = "ttf")]
        {
            ui.theme.font_size = Theme::default().font_size;
            ui.set_font(Some(Font::builtin()));
        }
        ui
    }

    /// A new context using the monospace 8x8 bitmap font at size 8.
    pub fn with_bitmap_font() -> Ui {
        Ui {
            theme: Theme { font_size: 8.0, ..Theme::default() },
            input: Input::default(),
            measure: Box::new(bitmap_width),
            #[cfg(feature = "ttf")]
            font: None,
            toast_secs: 3.0,
            clipboard: String::new(),
            screen: Rect::ZERO,
            mouse: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            pressed: false,
            released: false,
            was_down: false,
            dbl: false,
            time: 0.0,
            last_press: (-10.0, Vec2::ZERO),
            edit: 0,
            edit_buf: String::new(),
            rpressed: false,
            was_right: false,
            last: Rect::ZERO,
            cmds: Vec::new(),
            overlay: Vec::new(),
            text: String::new(),
            clips: Vec::new(),
            layouts: Vec::new(),
            ids: Vec::new(),
            next: [None, None],
            active: 0,
            focus: 0,
            caret: 0,
            sel: 0,
            prev_focus: 0,
            copied: false,
            edit_scroll: 0.0,
            open: 0,
            open_group: 0,
            just_opened: 0,
            modal: 0,
            modal_opened: false,
            toasts: Vec::new(),
            popup_prev: Rect::ZERO,
            popup_cur: Rect::ZERO,
            in_popup: false,
            store: HashMap::new(),
        }
    }

    /// Sets the font used for layout and by the built-in renderers.
    /// `None` selects the 8x8 bitmap font (pair it with `theme.font_size = 8.0`).
    #[cfg(feature = "ttf")]
    pub fn set_font(&mut self, font: Option<Font>) {
        self.measure = match &font {
            Some(f) => f.measure(),
            None => Box::new(bitmap_width),
        };
        self.font = font;
    }

    /// The current vector font; `None` means the 8x8 bitmap font.
    #[cfg(feature = "ttf")]
    pub fn font(&self) -> Option<&Font> {
        self.font.as_ref()
    }

    /// Starts a frame. `width`/`height` are the surface size in device pixels.
    pub fn begin(&mut self, width: f32, height: f32) {
        let s = self.theme.scale;
        self.screen = Rect::new(0.0, 0.0, width / s, height / s);
        let m = vec2(self.input.mouse.x / s, self.input.mouse.y / s);
        self.mouse_delta = vec2(m.x - self.mouse.x, m.y - self.mouse.y);
        self.mouse = m;
        self.pressed = self.input.down && !self.was_down;
        self.released = !self.input.down && self.was_down;
        self.was_down = self.input.down;
        self.time += if self.input.dt > 0.0 { self.input.dt } else { 1.0 / 60.0 };
        self.dbl = false;
        if self.pressed {
            let (at, p) = self.last_press;
            self.dbl = self.time - at < 0.4 && (m.x - p.x).abs() < 4.0 && (m.y - p.y).abs() < 4.0;
            // a third quick press starts over instead of counting as another double-click
            self.last_press = (if self.dbl { -10.0 } else { self.time }, m);
        }
        self.rpressed = self.input.right && !self.was_right;
        self.was_right = self.input.right;
        self.last = Rect::ZERO;
        self.prev_focus = self.focus;
        if self.pressed {
            self.focus = 0;
        }
        self.cmds.clear();
        self.overlay.clear();
        self.text.clear();
        self.clips.clear();
        self.clips.push(self.screen);
        self.layouts.clear();
        self.layouts.push(Layout::new(Kind::Col, self.screen, self.theme.gap));
        self.ids.clear();
        self.ids.push(FNV_OFFSET);
        self.next = [None, None];
        self.fill(self.screen, self.theme.bg);
    }

    /// Ends the frame. Afterwards read the result with [`Ui::draws`].
    pub fn end(&mut self) {
        debug_assert_eq!(self.layouts.len(), 1, "unbalanced layouts");
        if !self.input.down {
            self.active = 0;
        }
        self.cmds.append(&mut self.overlay);
        self.draw_toasts();
        self.just_opened = 0;
        self.modal_opened = false;
        self.popup_prev = self.popup_cur;
        self.popup_cur = Rect::ZERO;
        self.input.text.clear();
        self.input.keys.clear();
        self.input.scroll = Vec2::ZERO;
    }

    /// The draw list of the last frame, in device pixels, back to front.
    pub fn draws(&self) -> impl Iterator<Item = Draw<'_>> + '_ {
        self.cmds.iter().map(move |c| match *c {
            Cmd::Rect(rect, color) => Draw::Rect { rect, color },
            Cmd::Text { pos, size, color, start, end } => Draw::Text { pos, size, color, text: &self.text[start..end] },
            Cmd::Clip(r) => Draw::Clip(r),
        })
    }

    /// The text copied or cut since the last call, for the system clipboard.
    pub fn take_copied(&mut self) -> Option<&str> {
        std::mem::take(&mut self.copied).then_some(self.clipboard.as_str())
    }

    /// True while a text field has keyboard focus.
    pub fn wants_keyboard(&self) -> bool {
        self.focus != 0
    }

    // ---- ids & state -------------------------------------------------------

    /// Id for `label` in the current id scope.
    pub fn id(&self, label: &str) -> Id {
        let mut h = Fnv(*self.ids.last().unwrap() ^ FNV_OFFSET);
        h.write(label.as_bytes());
        h.0.max(1)
    }

    /// Opens an id scope, e.g. `ui.push_id(i)` in loops with repeated labels.
    pub fn push_id(&mut self, v: impl Hash) {
        let mut h = Fnv(*self.ids.last().unwrap() ^ FNV_OFFSET);
        v.hash(&mut h);
        self.ids.push(h.0.max(1));
    }

    pub fn pop_id(&mut self) {
        if self.ids.len() > 1 {
            self.ids.pop();
        }
    }

    /// Persistent per-widget value (scroll offsets, open flags, sizes ...).
    pub fn state(&mut self, id: Id, default: Vec2) -> Vec2 {
        *self.store.entry(id).or_insert(default)
    }

    pub fn set_state(&mut self, id: Id, v: Vec2) {
        self.store.insert(id, v);
    }

    /// Opens or closes the accordion / tree node `label` of the current id scope.
    pub fn set_open(&mut self, label: &str, open: bool) {
        let id = self.id(label);
        self.set_state(id, vec2(open as u8 as f32, 0.0));
    }

    // ---- interaction -------------------------------------------------------

    /// The rect of the most recently placed widget or container, for drawing
    /// on top of it (a marker, a badge, ...).
    pub fn last_rect(&self) -> Rect {
        self.last
    }

    /// Current clip rectangle (logical units).
    pub fn clip(&self) -> Rect {
        *self.clips.last().unwrap()
    }

    pub(crate) fn hit(&self, r: Rect) -> bool {
        let m = self.mouse;
        self.clip().contains(m) && r.contains(m) && (self.in_popup || !self.popup_prev.contains(m))
    }

    /// Mouse interaction for a custom widget occupying `rect`.
    pub fn interact(&mut self, id: Id, rect: Rect) -> Resp {
        let hit = self.hit(rect);
        // The last widget drawn under the cursor wins the press.
        if hit && self.pressed {
            self.active = id;
        }
        Resp {
            hovered: hit && (self.active == 0 || self.active == id),
            pressed: hit && self.pressed,
            held: self.active == id && self.input.down,
            clicked: hit && self.released && self.active == id,
            double_clicked: hit && self.dbl,
        }
    }

    // ---- drawing -----------------------------------------------------------

    /// Logical rect → device pixels, snapped to the pixel grid.
    pub(crate) fn dev(&self, r: Rect) -> Rect {
        let s = self.theme.scale;
        let (x, y) = ((r.x * s).round(), (r.y * s).round());
        Rect::new(x, y, ((r.x + r.w) * s).round() - x, ((r.y + r.h) * s).round() - y)
    }

    /// Thickness of a hairline in device pixels.
    pub(crate) fn line_px(&self) -> f32 {
        self.theme.scale.round().max(1.0)
    }

    pub fn push_clip(&mut self, r: Rect) {
        let c = self.clip().intersect(r);
        self.clips.push(c);
        self.cmds.push(Cmd::Clip(self.dev(c)));
    }

    pub fn pop_clip(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
        }
        self.cmds.push(Cmd::Clip(self.dev(self.clip())));
    }

    pub fn fill(&mut self, r: Rect, color: Color) {
        if r.w > 0.0 && r.h > 0.0 && self.clip().overlaps(r) {
            self.cmds.push(Cmd::Rect(self.dev(r), color));
        }
    }

    pub(crate) fn border_rects(&self, r: Rect) -> [Rect; 4] {
        let d = self.dev(r);
        let t = self.line_px().min(d.w / 2.0).min(d.h / 2.0).max(0.0);
        [
            Rect::new(d.x, d.y, d.w, t),
            Rect::new(d.x, d.bottom() - t, d.w, t),
            Rect::new(d.x, d.y + t, t, d.h - 2.0 * t),
            Rect::new(d.right() - t, d.y + t, t, d.h - 2.0 * t),
        ]
    }

    /// Square one-pixel outline inside `r`.
    pub fn border(&mut self, r: Rect, color: Color) {
        if r.w > 0.0 && r.h > 0.0 && self.clip().overlaps(r) {
            for b in self.border_rects(r) {
                self.cmds.push(Cmd::Rect(b, color));
            }
        }
    }

    /// Pixel-stepped line (for small icons).
    pub fn line(&mut self, a: Vec2, b: Vec2, color: Color) {
        let s = self.theme.scale;
        let t = self.line_px();
        let (ax, ay, bx, by) = (a.x * s, a.y * s, b.x * s, b.y * s);
        let n = (bx - ax).abs().max((by - ay).abs()).ceil().max(1.0) as i32;
        for i in 0..=n {
            let k = i as f32 / n as f32;
            let p = Rect::new((ax + (bx - ax) * k).round(), (ay + (by - ay) * k).round(), t, t);
            self.cmds.push(Cmd::Rect(p, color));
        }
    }

    /// Small triangle centered in `r`, pointing down or right.
    pub(crate) fn arrow(&mut self, r: Rect, down: bool, color: Color) {
        if !self.clip().overlaps(r) {
            return;
        }
        let d = self.dev(r);
        let t = self.line_px();
        let (cx, cy) = ((d.x + d.w / 2.0).floor(), (d.y + d.h / 2.0).floor());
        for i in 0..3 {
            let half = (3 - i) as f32 * t;
            let o = (i as f32 - 1.5) * t;
            let p = if down {
                Rect::new(cx - half, (cy + o).floor(), 2.0 * half, t)
            } else {
                Rect::new((cx + o).floor(), cy - half, t, 2.0 * half)
            };
            self.cmds.push(Cmd::Rect(p, color));
        }
    }

    /// Width of `s` in logical units.
    pub fn text_width(&self, s: &str) -> f32 {
        let k = self.theme.scale;
        (self.measure)(s, self.theme.font_size * k) / k
    }

    pub fn row_h(&self) -> f32 {
        self.theme.row_h()
    }

    /// Draws text with its top-left corner at `pos`.
    pub fn text(&mut self, pos: Vec2, s: &str, color: Color) {
        let c = self.clip();
        let fs = self.theme.font_size;
        if s.is_empty() || pos.y >= c.bottom() || pos.y + fs <= c.y || pos.x >= c.right() {
            return;
        }
        let k = self.theme.scale;
        let start = self.text.len();
        self.text.push_str(s);
        self.cmds.push(Cmd::Text {
            pos: vec2((pos.x * k).round(), (pos.y * k).round()),
            size: fs * k,
            color,
            start,
            end: self.text.len(),
        });
    }

    /// Draws text vertically centered in `r`, clipped if it does not fit.
    pub fn text_in(&mut self, r: Rect, s: &str, color: Color, align: Align) {
        if s.is_empty() {
            return;
        }
        let w = self.text_width(s);
        let x = match align {
            Align::Left => r.x,
            Align::Center => r.x + ((r.w - w) / 2.0).max(0.0),
            Align::Right => r.x + (r.w - w).max(0.0),
        };
        let y = r.y + (r.h - self.theme.font_size) / 2.0;
        let over = w > r.w + 0.5;
        if over {
            self.push_clip(r);
        }
        self.text(vec2(x, y), s, color);
        if over {
            self.pop_clip();
        }
    }
}
