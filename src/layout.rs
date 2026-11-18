use crate::geom::{vec2, Rect, Vec2};
use crate::ui::{sub, Cmd, Id, Ui};

/// Size of the next widget along one axis, set with [`Ui::w`] / [`Ui::h`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// Fit the content.
    Auto,
    /// Fixed size in logical units.
    Px(f32),
    /// Take all remaining space.
    Fill,
    /// Fraction of the parent layout.
    Frac(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Col,
    Row,
    Flow,
    Grid,
}

pub(crate) struct Layout {
    pub kind: Kind,
    pub rect: Rect,
    pub cur: Vec2,
    /// Bottom-right corner of everything placed so far.
    pub max: Vec2,
    pub line_h: f32,
    pub gap: f32,
    /// Grid columns as (x offset, width).
    pub cols: Vec<(f32, f32)>,
    pub col_i: usize,
    /// Widest natural content width seen so far (used to size popups).
    pub want: f32,
}

impl Layout {
    pub fn new(kind: Kind, rect: Rect, gap: f32) -> Layout {
        let p = vec2(rect.x, rect.y);
        Layout { kind, rect, cur: p, max: p, line_h: 0.0, gap, cols: Vec::new(), col_i: 0, want: 0.0 }
    }
}

const SALT_CONTENT: u64 = 0xC0;
const SALT_VBAR: u64 = 0xC1;
const SALT_HBAR: u64 = 0xC2;

impl Ui {
    // ---- sizing ------------------------------------------------------------

    /// Width of the next widget or container.
    pub fn w(&mut self, s: Size) -> &mut Self {
        self.next[0] = Some(s);
        self
    }

    /// Height of the next widget or container.
    pub fn h(&mut self, s: Size) -> &mut Self {
        self.next[1] = Some(s);
        self
    }

    pub fn size(&mut self, w: Size, h: Size) -> &mut Self {
        self.next = [Some(w), Some(h)];
        self
    }

    pub(crate) fn top(&mut self) -> &mut Layout {
        self.layouts.last_mut().unwrap()
    }

    /// Computes the rect for the next item without consuming the space.
    /// Returns the rect and whether each axis is content-sized.
    pub(crate) fn resolve(&mut self, content: Vec2, def: [Option<Size>; 2], container: bool) -> (Rect, [bool; 2]) {
        let next = std::mem::take(&mut self.next);
        let l = self.layouts.last_mut().unwrap();
        let grid = l.kind == Kind::Grid && !l.cols.is_empty();
        let (mut x, avail_w, base_w, dw) = if grid {
            let (cx, cw) = l.cols[l.col_i];
            (l.rect.x + cx, cw, cw, Size::Fill)
        } else if matches!(l.kind, Kind::Col | Kind::Grid) {
            (l.rect.x, l.rect.w, l.rect.w, Size::Fill)
        } else {
            (l.cur.x, l.rect.right() - l.cur.x, l.rect.w, Size::Auto)
        };
        let ws = next[0].or(def[0]).unwrap_or(dw);
        let hs = next[1].or(def[1]).unwrap_or(Size::Auto);
        let pick = |s: Size, content: f32, avail: f32, base: f32| {
            let v = match s {
                Size::Auto if container => avail,
                Size::Auto => content,
                Size::Px(p) => p,
                Size::Fill => avail,
                Size::Frac(f) => base * f,
            };
            v.max(0.0)
        };
        let mut w = pick(ws, content.x, avail_w, base_w);
        if !container {
            l.want = l.want.max(if let Size::Px(p) = ws { p } else { content.x });
        }
        let auto_box = container && ws == Size::Auto;
        if l.kind == Kind::Flow && l.cur.x > l.rect.x && !auto_box && x + w > l.rect.right() + 0.5 {
            // wrap to the next line
            l.cur.x = l.rect.x;
            l.cur.y += l.line_h + l.gap;
            l.line_h = 0.0;
            x = l.cur.x;
            if ws == Size::Fill {
                w = l.rect.w;
            }
        }
        let y = l.cur.y;
        let h = pick(hs, content.y, l.rect.bottom() - y, l.rect.h);
        (Rect::new(x, y, w, h), [ws == Size::Auto, hs == Size::Auto])
    }

    /// Consumes `r` in the current layout and moves the cursor past it.
    pub(crate) fn advance(&mut self, r: Rect) {
        self.last = r;
        let l = self.layouts.last_mut().unwrap();
        l.max.x = l.max.x.max(r.right());
        l.max.y = l.max.y.max(r.bottom());
        match l.kind {
            Kind::Grid if !l.cols.is_empty() => {
                l.line_h = l.line_h.max(r.h);
                l.col_i += 1;
                if l.col_i == l.cols.len() {
                    l.col_i = 0;
                    l.cur.y += l.line_h + l.gap;
                    l.line_h = 0.0;
                }
            }
            Kind::Col | Kind::Grid => {
                if r.h > 0.0 {
                    l.cur.y = r.bottom() + l.gap;
                }
            }
            Kind::Row | Kind::Flow => {
                l.line_h = l.line_h.max(r.h);
                if r.w > 0.0 {
                    l.cur.x = r.right() + l.gap;
                }
            }
        }
    }

    /// Allocates space for a widget with the given content size. Use this
    /// together with [`Ui::interact`] and the draw functions for custom widgets.
    pub fn alloc(&mut self, content: Vec2) -> Rect {
        let (r, _) = self.resolve(content, [None, None], false);
        self.advance(r);
        r
    }

    /// Like `alloc`, but fills the remaining space by default and never gets
    /// smaller than `min`, unless an explicit `Px` size was requested.
    pub(crate) fn alloc_fill(&mut self, min: Vec2) -> Rect {
        let px = self.next.map(|s| matches!(s, Some(Size::Px(_))));
        let (mut r, _) = self.resolve(min, [Some(Size::Fill), Some(Size::Fill)], false);
        if !px[0] {
            r.w = r.w.max(min.x);
        }
        if !px[1] {
            r.h = r.h.max(min.y);
        }
        self.advance(r);
        r
    }

    pub(crate) fn push_layout(&mut self, kind: Kind, rect: Rect) {
        self.layouts.push(Layout::new(kind, rect, self.theme.gap));
    }

    pub(crate) fn pop_layout(&mut self) -> Layout {
        self.layouts.pop().unwrap()
    }

    /// Nested layout; `m` = margins (left, top, right, bottom).
    pub(crate) fn container(&mut self, kind: Kind, m: [f32; 4], cols: &[Size], f: impl FnOnce(&mut Ui)) -> Rect {
        let (outer, auto) = self.resolve(Vec2::ZERO, [None, None], true);
        let rect = Rect::new(outer.x + m[0], outer.y + m[1], (outer.w - m[0] - m[2]).max(0.0), (outer.h - m[1] - m[3]).max(0.0));
        let mut l = Layout::new(kind, rect, self.theme.gap);
        if !cols.is_empty() {
            let n = cols.len() as f32;
            let total = (rect.w - l.gap * (n - 1.0)).max(0.0);
            let mut fixed = 0.0;
            let mut fills = 0.0;
            for c in cols {
                match *c {
                    Size::Px(p) => fixed += p,
                    Size::Frac(k) => fixed += total * k,
                    _ => fills += 1.0,
                }
            }
            let fill_w = if fills > 0.0 { ((total - fixed) / fills).max(0.0) } else { 0.0 };
            let mut x = 0.0;
            for c in cols {
                let w = match *c {
                    Size::Px(p) => p,
                    Size::Frac(k) => total * k,
                    _ => fill_w,
                };
                l.cols.push((x, w));
                x += w + l.gap;
            }
        }
        self.layouts.push(l);
        f(self);
        let l = self.pop_layout();
        let w = if auto[0] { l.max.x - rect.x + m[0] + m[2] } else { outer.w };
        let h = if auto[1] { l.max.y - rect.y + m[1] + m[3] } else { outer.h };
        let r = Rect::new(outer.x, outer.y, w, h);
        let natural = if matches!(kind, Kind::Row | Kind::Flow) { l.max.x - rect.x } else { l.want };
        let parent = self.top();
        parent.want = parent.want.max(natural + m[0] + m[2]);
        self.advance(r);
        r
    }

    // ---- layouts -----------------------------------------------------------

    /// Vertical stack (the default layout).
    pub fn col(&mut self, f: impl FnOnce(&mut Ui)) {
        self.container(Kind::Col, [0.0; 4], &[], f);
    }

    /// Horizontal stack.
    pub fn row(&mut self, f: impl FnOnce(&mut Ui)) {
        self.container(Kind::Row, [0.0; 4], &[], f);
    }

    /// Horizontal stack that wraps to the next line.
    pub fn flow(&mut self, f: impl FnOnce(&mut Ui)) {
        self.container(Kind::Flow, [0.0; 4], &[], f);
    }

    /// Grid: every widget goes into the next cell. `Fill`/`Auto` columns share
    /// the space left over by `Px` and `Frac` columns.
    pub fn grid(&mut self, cols: &[Size], f: impl FnOnce(&mut Ui)) {
        self.container(Kind::Grid, [0.0; 4], cols, f);
    }

    /// Vertical stack with the theme margin around it.
    pub fn pad(&mut self, f: impl FnOnce(&mut Ui)) {
        let m = self.theme.margin;
        self.container(Kind::Col, [m; 4], &[], f);
    }

    /// Vertical stack indented by `theme.indent`.
    pub fn indent(&mut self, f: impl FnOnce(&mut Ui)) {
        let d = self.theme.indent;
        self.container(Kind::Col, [d, 0.0, 0.0, 0.0], &[], f);
    }

    /// Padded vertical stack with background and border.
    pub fn frame(&mut self, f: impl FnOnce(&mut Ui)) {
        let i = self.cmds.len();
        for _ in 0..5 {
            self.cmds.push(Cmd::Rect(Rect::ZERO, self.theme.panel));
        }
        let m = self.theme.margin;
        let r = self.container(Kind::Col, [m; 4], &[], f);
        self.cmds[i] = Cmd::Rect(self.dev(r), self.theme.panel);
        for (k, b) in self.border_rects(r).into_iter().enumerate() {
            self.cmds[i + 1 + k] = Cmd::Rect(b, self.theme.border);
        }
    }

    /// Runs `f` in a clipped vertical stack covering exactly `rect`.
    pub fn pane(&mut self, rect: Rect, f: impl FnOnce(&mut Ui)) {
        self.push_clip(rect);
        self.push_layout(Kind::Col, rect);
        f(self);
        self.pop_layout();
        self.pop_clip();
    }

    /// Cuts a panel of `size` off one side of the current (vertical) layout.
    /// Everything placed afterwards uses the remaining area, so call docks first.
    pub fn dock(&mut self, side: Side, size: f32, f: impl FnOnce(&mut Ui)) {
        self.dock_with(side, size, self.theme.margin, f);
    }

    pub(crate) fn dock_with(&mut self, side: Side, size: f32, margin: f32, f: impl FnOnce(&mut Ui)) {
        let t = self.theme;
        let l = self.top();
        let rem = Rect::new(l.rect.x, l.cur.y, l.rect.w, (l.rect.bottom() - l.cur.y).max(0.0));
        let (pane, edge) = match side {
            Side::Top => {
                l.cur.y += size;
                (Rect::new(rem.x, rem.y, rem.w, size), Rect::new(rem.x, rem.y + size - 1.0, rem.w, 1.0))
            }
            Side::Bottom => {
                l.rect.h -= size;
                (Rect::new(rem.x, rem.bottom() - size, rem.w, size), Rect::new(rem.x, rem.bottom() - size, rem.w, 1.0))
            }
            Side::Left => {
                l.rect.x += size;
                l.rect.w -= size;
                l.cur.x += size;
                (Rect::new(rem.x, rem.y, size, rem.h), Rect::new(rem.x + size - 1.0, rem.y, 1.0, rem.h))
            }
            Side::Right => {
                l.rect.w -= size;
                (Rect::new(rem.right() - size, rem.y, size, rem.h), Rect::new(rem.right() - size, rem.y, 1.0, rem.h))
            }
        };
        l.max.x = l.max.x.max(pane.right());
        l.max.y = l.max.y.max(pane.bottom());
        self.fill(pane, t.panel);
        self.fill(edge, t.border);
        self.pane(pane.inset(margin), f);
        self.last = pane;
    }

    /// Two panes separated by a draggable handle. `vertical` stacks them
    /// top/bottom instead of left/right; `initial` is the size of the first pane.
    pub fn split(&mut self, id: &str, vertical: bool, initial: f32, a: impl FnOnce(&mut Ui), b: impl FnOnce(&mut Ui)) {
        const HANDLE: f32 = 3.0;
        let id = self.id(id);
        let t = self.theme;
        let r = self.alloc_fill(Vec2::ZERO);
        let len = if vertical { r.h } else { r.w };
        let mut pos = self.state(id, vec2(initial, 0.0)).x.min(len - HANDLE - 16.0).max(16.0);
        let handle = |pos: f32| if vertical { Rect::new(r.x, r.y + pos, r.w, HANDLE) } else { Rect::new(r.x + pos, r.y, HANDLE, r.h) };
        let resp = self.interact(id, handle(pos));
        if resp.held {
            pos += if vertical { self.mouse_delta.y } else { self.mouse_delta.x };
            pos = pos.min(len - HANDLE - 16.0).max(16.0);
            self.set_state(id, vec2(pos, 0.0));
        }
        let hr = handle(pos);
        let (ra, rb) = if vertical {
            (Rect::new(r.x, r.y, r.w, pos), Rect::new(r.x, hr.bottom(), r.w, (r.bottom() - hr.bottom()).max(0.0)))
        } else {
            (Rect::new(r.x, r.y, pos, r.h), Rect::new(hr.right(), r.y, (r.right() - hr.right()).max(0.0), r.h))
        };
        self.pane(ra, a);
        self.pane(rb, b);
        self.fill(hr, if resp.held { t.accent } else if resp.hovered { t.active } else { t.border });
        self.last = r;
    }

    /// Scrollable vertical stack. Fills the remaining space unless sized with
    /// [`Ui::w`] / [`Ui::h`].
    pub fn scroll(&mut self, id: &str, f: impl FnOnce(&mut Ui)) {
        let id = self.id(id);
        let r = self.alloc_fill(vec2(0.0, self.row_h() * 3.0));
        self.scroll_in(id, r, f);
        self.last = r;
    }

    pub(crate) fn scroll_in(&mut self, id: Id, rect: Rect, f: impl FnOnce(&mut Ui)) {
        let sb = self.theme.scrollbar;
        let step = self.row_h() * 3.0;
        let mut off = self.state(id, Vec2::ZERO);
        // Bars are decided from last frame's content size.
        let prev = self.state(sub(id, SALT_CONTENT), Vec2::ZERO);
        let mut need_v = prev.y > rect.h + 0.5;
        let need_h = prev.x > rect.w - if need_v { sb } else { 0.0 } + 0.5;
        need_v |= need_h && prev.y > rect.h - sb + 0.5;
        let view = Rect::new(
            rect.x,
            rect.y,
            (rect.w - if need_v { sb } else { 0.0 }).max(0.0),
            (rect.h - if need_h { sb } else { 0.0 }).max(0.0),
        );
        self.push_clip(view);
        self.push_layout(Kind::Col, Rect::new(view.x - off.x, view.y - off.y, view.w, view.h));
        f(self);
        let l = self.pop_layout();
        self.pop_clip();
        let content = vec2(l.max.x - l.rect.x, l.max.y - l.rect.y);
        self.set_state(sub(id, SALT_CONTENT), content);

        // Handled after the content so nested scroll areas get the wheel first.
        let wheel = self.input.scroll;
        if wheel != Vec2::ZERO && self.hit(rect) {
            off.x -= wheel.x * step;
            off.y -= wheel.y * step;
            self.input.scroll = Vec2::ZERO;
        }
        if need_v {
            let track = Rect::new(view.right(), rect.y, sb, view.h);
            off.y = self.scrollbar(sub(id, SALT_VBAR), track, true, content.y, view.h, off.y);
        }
        if need_h {
            let track = Rect::new(rect.x, view.bottom(), view.w, sb);
            off.x = self.scrollbar(sub(id, SALT_HBAR), track, false, content.x, view.w, off.x);
        }
        off.x = off.x.min(content.x - view.w).max(0.0);
        off.y = off.y.min(content.y - view.h).max(0.0);
        self.set_state(id, off);
    }

    fn scrollbar(&mut self, id: Id, track: Rect, vertical: bool, content: f32, view: f32, mut off: f32) -> f32 {
        let t = self.theme;
        let len = if vertical { track.h } else { track.w };
        let thumb = (view / content.max(1.0) * len).max(8.0).min(len);
        let range = (content - view).max(1.0);
        let free = (len - thumb).max(1.0);
        let p = (off / range).clamp(0.0, 1.0) * (len - thumb);
        let tr = if vertical { Rect::new(track.x, track.y + p, track.w, thumb) } else { Rect::new(track.x + p, track.y, thumb, track.h) };
        let resp = self.interact(id, tr);
        if resp.held {
            off += if vertical { self.mouse_delta.y } else { self.mouse_delta.x } * range / free;
        }
        self.fill(track, t.panel);
        self.fill(tr, if resp.held { t.accent } else if resp.hovered { t.active } else { t.hover });
        off
    }

    /// Empty space of `px` along the layout direction.
    pub fn space(&mut self, px: f32) {
        self.alloc(vec2(px, px));
    }

    /// Thin divider line across the layout direction.
    pub fn separator(&mut self) {
        let horizontal = matches!(self.top().kind, Kind::Row | Kind::Flow);
        let r = self.alloc(vec2(1.0, if horizontal { self.row_h() } else { 1.0 }));
        self.fill(r, self.theme.border);
    }
}
