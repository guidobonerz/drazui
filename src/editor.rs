//! Text editing shared by all text fields (caret, selection, clipboard) and
//! the multi-line text area.

use crate::geom::{vec2, Color, Rect, Vec2};
use crate::input::Key;
use crate::ui::{sub, Id, Ui};
use crate::widgets::{next_char, prev_char};

const SALT_SCROLL: u64 = 0xE0;
const SALT_COLUMN: u64 = 0xE1;
/// Space above the first and below the last line.
const TOP: f32 = 2.0;

/// Start and end (without the line break) of the line containing byte `at`.
fn line_at(s: &str, at: usize) -> (usize, usize) {
    let start = s[..at].rfind('\n').map_or(0, |i| i + 1);
    let end = s[at..].find('\n').map_or(s.len(), |i| at + i);
    (start, end)
}

/// Start and end of the word (letters, digits, `_`) around byte `at`.
fn word_at(s: &str, at: usize) -> (usize, usize) {
    let word = |ch: char| ch.is_alphanumeric() || ch == '_';
    let start = s[..at].char_indices().rev().take_while(|(_, ch)| word(*ch)).last().map_or(at, |(i, _)| i);
    let end = s[at..].char_indices().find(|(_, ch)| !word(*ch)).map_or(s.len(), |(i, _)| at + i);
    (start, end)
}

/// `i` moved back to a valid character boundary inside `s`.
pub(crate) fn clamp_index(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

impl Ui {
    /// Background of selected text.
    pub(crate) fn selection_color(&self) -> Color {
        self.theme.accent.mix(self.theme.field, 0.65)
    }

    /// Byte index in `line` of the character boundary nearest to `x`.
    pub(crate) fn index_at(&self, line: &str, x: f32) -> usize {
        let mut prev = 0.0;
        for (i, ch) in line.char_indices() {
            let w = self.text_width(&line[..i + ch.len_utf8()]);
            if x < (prev + w) / 2.0 {
                return i;
            }
            prev = w;
        }
        line.len()
    }

    /// Caret after moving `by` lines from `c`, staying near the column `x`.
    fn caret_vertical(&self, s: &str, c: usize, by: i32, x: f32) -> usize {
        let (mut start, mut end) = line_at(s, c);
        for _ in 0..by.abs() {
            if by < 0 {
                if start == 0 {
                    return 0;
                }
                (start, end) = line_at(s, start - 1);
            } else {
                if end == s.len() {
                    return s.len();
                }
                (start, end) = line_at(s, end + 1);
            }
        }
        start + self.index_at(&s[start..end], x)
    }

    /// Mouse press (`pressed`) or drag at byte `at` of the text `s` of the
    /// field `id`. A press focuses the field and places the caret, with Shift
    /// it extends the selection and as a double-click it selects a word; a
    /// drag moves the caret and leaves the other end of the selection.
    pub(crate) fn click_caret(&mut self, id: Id, s: &str, at: usize, pressed: bool) {
        if !pressed {
            self.caret = at;
            return;
        }
        let extend = self.input.shift && self.prev_focus == id;
        self.focus = id;
        self.set_state(sub(id, SALT_COLUMN), vec2(-1.0, 0.0));
        if self.dbl {
            (self.sel, self.caret) = word_at(s, at);
        } else {
            self.caret = at;
            if !extend {
                self.sel = at;
            }
        }
    }

    /// Applies this frame's keys and typed text to the focused field `id`.
    /// `multi`: Enter inserts a line break and the vertical keys move the
    /// caret (`page` lines for Page Up / Down). Returns (text changed, caret
    /// moved or text changed).
    pub(crate) fn edit_text(&mut self, id: Id, value: &mut String, multi: bool, page: i32) -> (bool, bool) {
        let col_id = sub(id, SALT_COLUMN);
        // the text may have been replaced from outside since the last frame
        let mut c = clamp_index(value, self.caret);
        let mut a = clamp_index(value, self.sel);
        let shift = self.input.shift;
        let (mut changed, mut moved) = (false, false);
        for key in std::mem::take(&mut self.input.keys) {
            moved = true;
            let (lo, hi) = (a.min(c), a.max(c));
            let lines = match key {
                Key::Up => -1,
                Key::Down => 1,
                Key::PageUp => -page,
                Key::PageDown => page,
                _ => 0,
            };
            if lines != 0 {
                if multi {
                    // vertical moves keep the column they started from
                    let mut x = self.state(col_id, vec2(-1.0, 0.0)).x;
                    if x < 0.0 {
                        x = self.text_width(&value[line_at(value, c).0..c]);
                        self.set_state(col_id, vec2(x, 0.0));
                    }
                    c = self.caret_vertical(value, c, lines, x);
                    if !shift {
                        a = c;
                    }
                }
                continue;
            }
            self.set_state(col_id, vec2(-1.0, 0.0));
            let to = match key {
                // without Shift the arrows first collapse a selection
                Key::Left if lo < hi && !shift => Some(lo),
                Key::Right if lo < hi && !shift => Some(hi),
                Key::Left => Some(prev_char(value, c)),
                Key::Right => Some(next_char(value, c)),
                Key::Home => Some(line_at(value, c).0),
                Key::End => Some(line_at(value, c).1),
                _ => None,
            };
            if let Some(to) = to {
                c = to;
                if !shift {
                    a = c;
                }
                continue;
            }
            // the remaining keys replace a range of the text
            let put = match key {
                Key::SelectAll => {
                    (a, c) = (0, value.len());
                    None
                }
                Key::Copy | Key::Cut if lo < hi => {
                    self.clipboard = value[lo..hi].to_string();
                    self.copied = true;
                    (key == Key::Cut).then(|| (lo..hi, String::new()))
                }
                Key::Paste => {
                    let text: String = self
                        .clipboard
                        .chars()
                        .filter_map(|ch| match ch {
                            '\n' if multi => Some('\n'),
                            '\n' | '\t' => Some(' '),
                            ch if ch.is_control() => None,
                            ch => Some(ch),
                        })
                        .collect();
                    (!text.is_empty()).then_some((lo..hi, text))
                }
                Key::Backspace | Key::Delete if lo < hi => Some((lo..hi, String::new())),
                Key::Backspace if c > 0 => Some((prev_char(value, c)..c, String::new())),
                Key::Delete if c < value.len() => Some((c..next_char(value, c), String::new())),
                Key::Enter if multi => Some((lo..hi, "\n".to_string())),
                Key::Enter | Key::Escape => {
                    self.focus = 0;
                    None
                }
                _ => None,
            };
            if let Some((range, text)) = put {
                c = range.start + text.len();
                a = c;
                value.replace_range(range, &text);
                changed = true;
            }
        }
        let typed: String = std::mem::take(&mut self.input.text).chars().filter(|ch| !ch.is_control()).collect();
        if !typed.is_empty() {
            let lo = a.min(c);
            value.replace_range(lo..a.max(c), &typed);
            c = lo + typed.len();
            a = c;
            changed = true;
            moved = true;
            self.set_state(col_id, vec2(-1.0, 0.0));
        }
        self.caret = c;
        self.sel = a;
        (changed, moved)
    }

    /// Multi-line text editor bound to `value` (lines are separated by `\n`).
    /// Fills the remaining space unless sized with [`Ui::w`] / [`Ui::h`] and
    /// scrolls in both directions; long lines are not wrapped.
    ///
    /// Mouse and keys work as in [`Ui::text_edit`], plus Up / Down, Page Up /
    /// Page Down and Enter for a new line. Escape leaves the editor. There is
    /// no undo. Returns true when the text changed.
    pub fn text_area(&mut self, id: &str, value: &mut String) -> bool {
        let id = self.id(id);
        let scroll_id = sub(id, SALT_SCROLL);
        let t = self.theme;
        let lh = t.font_size + t.pad.y;
        let r = self.alloc_fill(vec2(0.0, lh * 3.0 + 2.0 * TOP + 2.0));
        let inner = r.inset(1.0);
        let resp = self.interact(id, r);
        self.fill(r, t.field);

        let (mut changed, mut moved) = (false, false);
        if self.focus == id {
            let page = ((inner.h / lh).floor() as i32 - 1).max(1);
            (changed, moved) = self.edit_text(id, value, true, page);
        }
        let value: &str = value;

        let mut starts = vec![0];
        starts.extend(value.match_indices('\n').map(|(i, _)| i + 1));
        let n = starts.len();
        let line = |i: usize| &value[starts[i]..starts.get(i + 1).map_or(value.len(), |e| e - 1)];
        let width = (0..n).map(|i| self.text_width(line(i))).fold(0.0, f32::max);
        // line index and x offset of the byte `at`
        let locate = |ui: &Ui, at: usize| {
            let i = starts.partition_point(|&s| s <= at) - 1;
            (i, ui.text_width(&value[starts[i]..at]))
        };
        // scrolls the caret into view (assuming both scrollbars are shown)
        let reveal = |ui: &mut Ui| {
            let (i, x) = locate(ui, clamp_index(value, ui.caret));
            let sb = t.scrollbar;
            let (cx, cy) = (t.pad.x + x, TOP + i as f32 * lh);
            let mut off = ui.state(scroll_id, Vec2::ZERO);
            off.x = off.x.min(cx - t.pad.x).max(cx + t.pad.x + 1.0 - (inner.w - sb)).max(0.0);
            off.y = off.y.min(cy - TOP).max(cy + lh + TOP - (inner.h - sb)).max(0.0);
            ui.set_state(scroll_id, off);
        };
        if moved && self.focus == id {
            reveal(self);
        }

        let mut dragged = false;
        self.scroll_in(scroll_id, inner, |ui| {
            let (ox, oy) = {
                let l = ui.top();
                (l.rect.x, l.rect.y)
            };
            ui.advance(Rect::new(ox, oy, width + 2.0 * t.pad.x + 1.0, n as f32 * lh + 2.0 * TOP));
            let view = ui.clip();
            // presses on the scrollbars are outside the view
            let press = resp.pressed && ui.hit(view);
            // a drag past the edge keeps selecting (and scrolling) while the mouse rests
            let drag = !resp.pressed && resp.held && ui.focus == id && (ui.mouse_delta != Vec2::ZERO || !view.contains(ui.mouse));
            if press || drag {
                let i = (((ui.mouse.y - oy - TOP) / lh).floor().max(0.0) as usize).min(n - 1);
                let at = starts[i] + ui.index_at(line(i), ui.mouse.x - ox - t.pad.x);
                ui.click_caret(id, value, at, press);
                dragged = drag;
            }
            let focused = ui.focus == id;
            let (c, a) = (clamp_index(value, ui.caret), clamp_index(value, ui.sel));
            let (lo, hi) = (a.min(c), a.max(c));
            let sel_bg = ui.selection_color();
            // only the visible lines are drawn
            let first = ((view.y - oy - TOP) / lh).floor().max(0.0) as usize;
            let last = (((view.bottom() - oy - TOP) / lh).ceil().max(0.0) as usize).min(n);
            for i in first..last {
                let (x, y) = (ox + t.pad.x, oy + TOP + i as f32 * lh);
                let text = line(i);
                let (ls, le) = (starts[i], starts[i] + text.len());
                if focused && lo < hi && lo <= le && hi > ls {
                    let x0 = ui.text_width(&value[ls..lo.max(ls)]);
                    // a selected line break shows as half a character
                    let x1 = ui.text_width(&value[ls..hi.min(le)]) + if hi > le { t.font_size / 2.0 } else { 0.0 };
                    ui.fill(Rect::new(x + x0, y, x1 - x0, lh), sel_bg);
                }
                ui.text(vec2(x, y + (lh - t.font_size) / 2.0), text, t.text);
            }
            if focused {
                let (i, x) = locate(ui, c);
                ui.fill(Rect::new(ox + t.pad.x + x, oy + TOP + i as f32 * lh + 1.0, 1.0, lh - 2.0), t.accent);
            }
        });
        if dragged {
            reveal(self);
        }

        let focused = self.focus == id;
        self.border(r, if focused { t.accent } else if resp.hovered { t.active } else { t.border });
        self.last = r;
        changed
    }
}
