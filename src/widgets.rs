use crate::editor::clamp_index;
use crate::geom::{vec2, Color, Rect, Vec2};
use crate::layout::Kind;
use crate::ui::{sub, Align, Cmd, Id, Resp, Ui};

pub(crate) struct Toast {
    text: String,
    color: Color,
    /// Seconds left.
    left: f32,
}

const SALT_POPUP: u64 = 0xD0;
const SALT_BODY: u64 = 0xD1;
const SALT_GRIP: u64 = 0x1_0000;

pub(crate) fn prev_char(s: &str, i: usize) -> usize {
    s[..i].char_indices().next_back().map_or(0, |(j, _)| j)
}

pub(crate) fn next_char(s: &str, i: usize) -> usize {
    i + s[i..].chars().next().map_or(0, char::len_utf8)
}

/// All value widgets bind two-way: they read the current value from the `&mut`
/// you pass in, write user changes straight back, and return `true` on change.
impl Ui {
    fn widget_bg(&self, r: &Resp) -> Color {
        let t = &self.theme;
        if r.held {
            t.active
        } else if r.hovered {
            t.hover
        } else {
            t.widget
        }
    }

    pub fn label(&mut self, text: &str) {
        self.label_colored(text, self.theme.text);
    }

    pub fn label_dim(&mut self, text: &str) {
        self.label_colored(text, self.theme.text_dim);
    }

    pub fn label_colored(&mut self, text: &str, color: Color) {
        let r = self.alloc(vec2(self.text_width(text), self.row_h()));
        self.text_in(r, text, color, Align::Left);
    }

    /// Returns true when clicked.
    pub fn button(&mut self, text: &str) -> bool {
        let id = self.id(text);
        let t = self.theme;
        let r = self.alloc(vec2(self.text_width(text) + t.pad.x * 2.0, t.row_h()));
        let resp = self.interact(id, r);
        self.fill(r, self.widget_bg(&resp));
        self.border(r, t.border);
        self.text_in(r.inset_xy(t.pad.x, 0.0), text, t.text, Align::Center);
        resp.clicked
    }

    /// Square box + label shared by checkbox and radio. Returns true when clicked.
    fn mark(&mut self, text: &str, on: bool, radio: bool) -> bool {
        let id = self.id(text);
        let t = self.theme;
        let bs = t.font_size + 2.0;
        let tw = self.text_width(text);
        let r = self.alloc(vec2(bs + if tw > 0.0 { t.pad.x + tw } else { 0.0 }, t.row_h()));
        let resp = self.interact(id, r);
        let b = Rect::new(r.x, r.y + ((r.h - bs) / 2.0).floor(), bs, bs);
        self.mark_box(b, on, radio, resp.hovered);
        self.text_in(Rect::new(b.right() + t.pad.x, r.y, r.w - bs - t.pad.x, r.h), text, t.text, Align::Left);
        resp.clicked
    }

    /// The square of a checkbox (tick) or radio button (dot).
    pub(crate) fn mark_box(&mut self, b: Rect, on: bool, radio: bool, hovered: bool) {
        let t = self.theme;
        let bs = b.w;
        self.fill(b, if hovered { t.hover } else { t.field });
        self.border(b, if hovered { t.accent } else { t.border });
        if on && radio {
            self.fill(b.inset((bs * 0.3).round()), t.accent);
        } else if on {
            let k = bs / 10.0;
            let p = |x: f32, y: f32| vec2(b.x + x * k, b.y + y * k);
            for dy in [0.0, 1.0] {
                self.line(p(2.0, 4.0 + dy), p(4.0, 6.0 + dy), t.accent);
                self.line(p(4.0, 6.0 + dy), p(7.0, 2.0 + dy), t.accent);
            }
        }
    }

    pub fn checkbox(&mut self, text: &str, value: &mut bool) -> bool {
        let clicked = self.mark(text, *value, false);
        if clicked {
            *value = !*value;
        }
        clicked
    }

    /// Radio button: sets `*value = option` when clicked.
    pub fn radio<T: PartialEq>(&mut self, text: &str, value: &mut T, option: T) -> bool {
        let changed = self.mark(text, *value == option, true) && *value != option;
        if changed {
            *value = option;
        }
        changed
    }

    /// Flat clickable row, highlighted when `selected`. Returns true when clicked.
    pub fn selectable(&mut self, text: &str, selected: bool) -> bool {
        let id = self.id(text);
        let t = self.theme;
        let r = self.alloc(vec2(self.text_width(text) + t.pad.x * 2.0, t.row_h()));
        let resp = self.interact(id, r);
        if selected {
            self.fill(r, t.select);
        } else if resp.hovered {
            self.fill(r, t.hover);
        }
        self.text_in(r.inset_xy(t.pad.x, 0.0), text, if selected { t.select_text } else { t.text }, Align::Left);
        resp.clicked
    }

    /// Drop-down bound to the index `selected` into `items`.
    pub fn select(&mut self, id: &str, selected: &mut usize, items: &[&str]) -> bool {
        let id = self.id(id);
        let t = self.theme;
        let rh = t.row_h();
        let widest = items.iter().map(|s| self.text_width(s)).fold(0.0, f32::max);
        let r = self.alloc(vec2(widest + t.pad.x * 2.0 + rh, rh));
        let resp = self.interact(id, r);
        if resp.clicked {
            self.open = if self.open == id { 0 } else { id };
        }
        let is_open = self.open == id;
        self.fill(r, self.widget_bg(&resp));
        self.border(r, if is_open { t.accent } else { t.border });
        let cur = items.get(*selected).copied().unwrap_or("");
        self.text_in(Rect::new(r.x + t.pad.x, r.y, r.w - rh - t.pad.x, rh), cur, t.text, Align::Left);
        self.arrow(Rect::new(r.right() - rh, r.y, rh, rh), true, t.text_dim);

        let mut changed = false;
        if is_open {
            let h = (items.len() as f32 * rh + 2.0).min((self.screen.h * 0.6).max(rh * 3.0));
            let below = self.screen.bottom() - r.bottom();
            let y = if h > below && r.y > below { (r.y - h).max(0.0) } else { r.bottom() };
            let pr = Rect::new(r.x, y, r.w, h);
            if self.pressed && !pr.contains(self.mouse) && !r.contains(self.mouse) {
                self.open = 0;
            } else {
                // Draw into the overlay so the popup ends up above everything.
                std::mem::swap(&mut self.cmds, &mut self.overlay);
                self.clips.push(self.screen);
                self.cmds.push(Cmd::Clip(self.dev(self.screen)));
                self.in_popup = true;
                self.popup_cur = pr;
                self.fill(pr, t.panel);
                self.border(pr, t.accent);
                self.ids.push(id);
                self.scroll_in(sub(id, SALT_POPUP), pr.inset(1.0), |ui| {
                    ui.top().gap = 0.0;
                    for (i, item) in items.iter().enumerate() {
                        ui.push_id(i);
                        if ui.selectable(item, i == *selected) {
                            changed = *selected != i;
                            *selected = i;
                            ui.open = 0;
                        }
                        ui.pop_id();
                    }
                });
                self.ids.pop();
                self.in_popup = false;
                self.clips.pop();
                std::mem::swap(&mut self.cmds, &mut self.overlay);
            }
        }
        changed
    }

    /// Single-line text field bound to `value`.
    ///
    /// A click places the caret, dragging or Shift + click selects, a
    /// double-click selects a word. Keys: Left / Right / Home / End (with
    /// [`Input::shift`](crate::Input::shift) they select), Backspace, Delete,
    /// [`Key::SelectAll`](crate::Key::SelectAll), `Copy`, `Cut` and `Paste`
    /// (see [`Ui::clipboard`]); Enter or Escape leave the field.
    pub fn text_edit(&mut self, id: &str, value: &mut String) -> bool {
        let id = self.id(id);
        let r = self.alloc(vec2(120.0, self.row_h()));
        self.text_edit_at(id, r, value, false)
    }

    /// `right`: text that fits is right-aligned (number fields).
    pub(crate) fn text_edit_at(&mut self, id: Id, r: Rect, value: &mut String, right: bool) -> bool {
        let t = self.theme;
        let resp = self.interact(id, r);
        let inner = r.inset_xy(t.pad.x, 0.0);
        // A press clears the focus before the widgets run, so a click into the
        // field that already had it still needs its scroll offset.
        let was_focused = self.focus == id || (resp.pressed && self.prev_focus == id);
        let mut scroll = if was_focused { self.edit_scroll } else { 0.0 };
        // free space left of right-aligned text (1 px stays for the caret)
        let free = |ui: &Ui, s: &str| if right { (inner.w - ui.text_width(s) - 1.0).max(0.0) } else { 0.0 };
        let drag = !resp.pressed && resp.held && self.focus == id && self.mouse_delta != Vec2::ZERO;
        if resp.pressed || drag {
            // caret to the character boundary nearest to the mouse
            let at = self.index_at(value, self.mouse.x - inner.x - free(self, value) + scroll);
            self.click_caret(id, value, at, resp.pressed);
        }
        let mut changed = false;
        if self.focus == id {
            changed = self.edit_text(id, value, false, 0).0;
        }
        let focused = self.focus == id;
        let (c, a) = if focused { (clamp_index(value, self.caret), clamp_index(value, self.sel)) } else { (0, 0) };
        let cx = self.text_width(&value[..c]);
        if focused {
            // keep the caret visible
            scroll = scroll.max(cx - inner.w + 1.0).min(cx).max(0.0);
            self.edit_scroll = scroll;
        }
        let x0 = inner.x + free(self, value) - scroll;
        self.fill(r, t.field);
        self.border(r, if focused { t.accent } else if resp.hovered { t.active } else { t.border });
        self.push_clip(r.inset(1.0));
        if a != c {
            let ax = self.text_width(&value[..a]);
            self.fill(Rect::new(x0 + ax.min(cx), r.y + 2.0, (ax - cx).abs(), r.h - 4.0), self.selection_color());
        }
        self.text(vec2(x0, r.y + t.pad.y), value, t.text);
        if focused {
            self.fill(Rect::new(x0 + cx, r.y + 2.0, 1.0, r.h - 4.0), t.accent);
        }
        self.pop_clip();
        changed
    }

    /// Header row with a fold arrow, shared by accordion and tree.
    fn fold_row(&mut self, id: Id, label: &str, header: bool, selected: bool, open: Option<&mut bool>) -> bool {
        let t = self.theme;
        let rh = t.row_h();
        let r = self.alloc(vec2(rh + self.text_width(label) + t.pad.x, rh));
        let resp = self.interact(id, r);
        if header {
            self.fill(r, self.widget_bg(&resp));
            self.border(r, t.border);
        } else if selected {
            self.fill(r, t.select);
        } else if resp.hovered {
            self.fill(r, t.hover);
        }
        let ink = if selected { t.select_text } else { t.text };
        if let Some(open) = open {
            if resp.clicked {
                *open = !*open;
            }
            self.arrow(Rect::new(r.x, r.y, rh, rh), *open, if selected { ink } else { t.accent });
        }
        self.text_in(Rect::new(r.x + rh, r.y, r.w - rh - t.pad.x, rh), label, ink, Align::Left);
        resp.clicked
    }

    // ---- chips ------------------------------------------------------------

    /// Returns (clicked, close clicked).
    fn chip_impl(&mut self, text: &str, on: bool, close: bool) -> (bool, bool) {
        let id = self.id(text);
        let t = self.theme;
        let s = t.font_size - 2.0; // size of the close cross
        let xw = if close { s + t.pad.x } else { 0.0 };
        let r = self.alloc(vec2(self.text_width(text) + t.pad.x * 2.0 + xw, t.row_h()));
        let resp = self.interact(id, r);
        let b = r.inset_xy(0.0, 1.0);
        let (bg, ink) = if on {
            (if resp.hovered { t.accent.mix(t.text, 0.2) } else { t.accent }, t.field)
        } else {
            (t.bg.mix(t.accent, if resp.hovered { 0.4 } else { 0.22 }), t.text)
        };
        self.fill(b, bg);
        self.text_in(Rect::new(b.x + t.pad.x, b.y, b.w - t.pad.x * 2.0 - xw, b.h), text, ink, Align::Left);
        let mut closed = false;
        if close {
            let (x, y) = (b.right() - t.pad.x - s, b.y + ((b.h - s) / 2.0).floor());
            let over = resp.hovered && self.mouse.x >= x - t.pad.x / 2.0;
            closed = resp.clicked && over;
            let c = if over { t.select } else { ink };
            self.line(vec2(x, y), vec2(x + s - 1.0, y + s - 1.0), c);
            self.line(vec2(x, y + s - 1.0), vec2(x + s - 1.0, y), c);
        }
        (resp.clicked && !closed, closed)
    }

    /// Small tag. Returns true when clicked.
    pub fn chip(&mut self, text: &str) -> bool {
        self.chip_impl(text, false, false).0
    }

    /// Filter chip bound to `value`: highlighted while on, toggles on click.
    pub fn chip_toggle(&mut self, text: &str, value: &mut bool) -> bool {
        let clicked = self.chip_impl(text, *value, false).0;
        if clicked {
            *value = !*value;
        }
        clicked
    }

    /// Removable chip. Returns true when its close cross is clicked.
    pub fn chip_close(&mut self, text: &str) -> bool {
        self.chip_impl(text, false, true).1
    }

    // ---- toasts -----------------------------------------------------------

    /// Shows a short message in the bottom-right corner for
    /// [`toast_secs`](Ui::toast_secs). Click a toast to dismiss it.
    pub fn toast(&mut self, text: &str) {
        self.toast_colored(text, self.theme.accent);
    }

    /// Toast with a custom marker color, e.g. `theme.select` for warnings.
    pub fn toast_colored(&mut self, text: &str, color: Color) {
        self.toasts.push(Toast { text: text.into(), color, left: self.toast_secs });
    }

    pub(crate) fn draw_toasts(&mut self) {
        if self.toasts.is_empty() {
            return;
        }
        const MARK: f32 = 3.0;
        let t = self.theme;
        let dt = if self.input.dt > 0.0 { self.input.dt } else { 1.0 / 60.0 };
        let (m, rh, screen) = (t.margin, t.row_h(), self.screen);
        self.cmds.push(Cmd::Clip(self.dev(screen)));
        let mut toasts = std::mem::take(&mut self.toasts);
        let mut y = screen.bottom() - m;
        toasts.retain_mut(|toast| {
            let w = (self.text_width(&toast.text) + t.pad.x * 2.0 + MARK).min(screen.w - 2.0 * m);
            let r = Rect::new(screen.right() - m - w, y - rh, w, rh);
            y -= rh + t.gap;
            self.fill(r, t.widget);
            self.border(r, t.border);
            self.fill(Rect::new(r.x, r.y, MARK, r.h), toast.color);
            self.text_in(Rect::new(r.x + MARK + t.pad.x, r.y, r.w - MARK - t.pad.x * 2.0, rh), &toast.text, t.text, Align::Left);
            toast.left -= dt;
            toast.left > 0.0 && !(self.pressed && r.contains(self.mouse))
        });
        self.toasts = toasts;
    }

    fn fold_state(&mut self, id: Id, default_open: bool) -> bool {
        self.state(id, vec2(default_open as u8 as f32, 0.0)).x > 0.5
    }

    /// Collapsible section. Returns whether it is open.
    pub fn accordion(&mut self, title: &str, default_open: bool, f: impl FnOnce(&mut Ui)) -> bool {
        let id = self.id(title);
        let mut open = self.fold_state(id, default_open);
        if self.fold_row(id, title, true, false, Some(&mut open)) {
            self.set_state(id, vec2(open as u8 as f32, 0.0));
        }
        if open {
            let m = self.theme.margin;
            self.ids.push(id);
            self.container(Kind::Col, [m; 4], &[], f);
            self.ids.pop();
        }
        open
    }

    /// Tree node with children. Clicking toggles it; returns true when clicked,
    /// so the caller can track the selection. Nodes start closed, see
    /// [`Ui::set_open`].
    pub fn tree_node(&mut self, label: &str, selected: bool, f: impl FnOnce(&mut Ui)) -> bool {
        let id = self.id(label);
        let mut open = self.fold_state(id, false);
        let clicked = self.fold_row(id, label, false, selected, Some(&mut open));
        if clicked {
            self.set_state(id, vec2(open as u8 as f32, 0.0));
        }
        if open {
            self.ids.push(id);
            self.indent(f);
            self.ids.pop();
        }
        clicked
    }

    /// Tree node without children. Returns true when clicked.
    pub fn tree_leaf(&mut self, label: &str, selected: bool) -> bool {
        let id = self.id(label);
        self.fold_row(id, label, false, selected, None)
    }

    /// Virtualized, scrollable list bound to `selected`. `row` draws the
    /// content of one row (in a horizontal layout) and is only called for
    /// visible rows.
    pub fn list(&mut self, id: &str, rows: usize, selected: &mut Option<usize>, mut row: impl FnMut(&mut Ui, usize)) -> bool {
        let id = self.id(id);
        let t = self.theme;
        let r = self.alloc_fill(vec2(0.0, t.row_h() * 3.0));
        self.fill(r, t.field);
        self.border(r, t.border);
        let changed = self.rows(id, r.inset(1.0), &[], rows, selected, |ui, r, _| row(ui, r));
        self.last = r;
        changed
    }

    /// Virtualized table bound to `selected`. `cols` are `(title, initial
    /// width)`; columns are resizable by dragging the header separators and the
    /// last one stretches. `cell(ui, row, col)` draws one cell.
    pub fn table(
        &mut self,
        id: &str,
        cols: &[(&str, f32)],
        rows: usize,
        selected: &mut Option<usize>,
        cell: impl FnMut(&mut Ui, usize, usize),
    ) -> bool {
        let id = self.id(id);
        let t = self.theme;
        let rh = t.row_h();
        let r = self.alloc_fill(vec2(0.0, rh * 4.0));
        self.fill(r, t.field);
        let inner = r.inset(1.0);
        let head = Rect::new(inner.x, inner.y, inner.w, rh);
        let body = Rect::new(inner.x, inner.y + rh, inner.w, (inner.h - rh).max(0.0));
        let body_id = sub(id, SALT_BODY);
        let off_x = self.state(body_id, Vec2::ZERO).x;
        let mut widths = Vec::with_capacity(cols.len());
        for (i, c) in cols.iter().enumerate() {
            widths.push(self.state(sub(id, i as u64), vec2(c.1, 0.0)).x);
        }

        self.push_clip(head);
        self.fill(head, t.widget);
        let mut x = head.x - off_x;
        for (i, (title, _)) in cols.iter().enumerate() {
            let w = if i + 1 == cols.len() { widths[i].max(head.right() - x) } else { widths[i] };
            self.text_in(Rect::new(x + t.pad.x, head.y, w - t.pad.x * 2.0, rh), title, t.text_dim, Align::Left);
            let resp = self.interact(sub(id, SALT_GRIP + i as u64), Rect::new(x + w - 3.0, head.y, 5.0, rh));
            if resp.held {
                widths[i] = (widths[i] + self.mouse_delta.x).max(12.0);
                self.set_state(sub(id, i as u64), vec2(widths[i], 0.0));
            }
            let grip = if resp.held || resp.hovered { t.accent } else { t.border };
            self.fill(Rect::new(x + w - 1.0, head.y, 1.0, rh), grip);
            x += w;
        }
        self.pop_clip();
        self.border(r, t.border);
        let changed = self.rows(body_id, body, &widths, rows, selected, cell);
        self.last = r;
        changed
    }

    fn rows(
        &mut self,
        id: Id,
        rect: Rect,
        widths: &[f32],
        n: usize,
        selected: &mut Option<usize>,
        mut cell: impl FnMut(&mut Ui, usize, usize),
    ) -> bool {
        let t = self.theme;
        let rh = t.row_h();
        let mut changed = false;
        self.scroll_in(id, rect, |ui| {
            let (ox, oy, view_w) = {
                let l = ui.top();
                (l.rect.x, l.rect.y, l.rect.w)
            };
            let total_w = widths.iter().sum::<f32>().max(view_w);
            ui.advance(Rect::new(ox, oy, total_w, n as f32 * rh));
            // only rows inside the clip rect are built
            let clip = ui.clip();
            let first = ((clip.y - oy) / rh).floor().max(0.0) as usize;
            let last = (((clip.bottom() - oy) / rh).ceil().max(0.0) as usize).min(n);
            let ncol = widths.len().max(1);
            for r in first..last {
                let row_id = sub(id, r as u64 + 1);
                let rr = Rect::new(ox, oy + r as f32 * rh, total_w, rh);
                let resp = ui.interact(row_id, rr);
                // a right-click selects too, so context menus act on that row
                if resp.clicked || (ui.rpressed && ui.hit(rr)) {
                    changed |= *selected != Some(r);
                    *selected = Some(r);
                }
                let is_sel = *selected == Some(r);
                if is_sel {
                    ui.fill(rr, t.select);
                    ui.theme.text = t.select_text;
                } else if resp.hovered {
                    ui.fill(rr, t.hover);
                } else if r % 2 == 1 {
                    ui.fill(rr, t.stripe);
                }
                let mut x = ox;
                for c in 0..ncol {
                    let w = if c + 1 == ncol { total_w - (x - ox) } else { widths[c] };
                    let cr = Rect::new(x + t.pad.x, rr.y, (w - t.pad.x * 2.0).max(0.0), rh);
                    ui.push_clip(cr);
                    ui.push_layout(Kind::Row, cr);
                    ui.ids.push(sub(row_id, c as u64));
                    cell(ui, r, c);
                    ui.ids.pop();
                    ui.pop_layout();
                    ui.pop_clip();
                    x += w;
                }
                ui.theme.text = t.text;
            }
        });
        changed
    }
}
