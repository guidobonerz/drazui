//! Editable table cells: shown as plain content, edited in place after a
//! double-click. Enter or a click elsewhere commits, Escape cancels.
//!
//! Call them from the cell closure of [`Ui::table`] (or the row closure of
//! [`Ui::list`]); each one binds two-way to the `&mut` value it is given and
//! returns true when the value changed.

use std::fmt::Display;
use std::str::FromStr;

use crate::geom::{vec2, Color, Rect};
use crate::input::Key;
use crate::layout::Size;
use crate::theme::Theme;
use crate::ui::{Align, Id, Ui};

impl Ui {
    /// The whole cell.
    fn cell_rect(&mut self) -> Rect {
        let rh = self.row_h();
        self.w(Size::Fill).alloc(vec2(0.0, rh))
    }

    /// Shows `text` in `r`; after a double-click edits a copy of it in place.
    /// Returns the edited text once it is committed.
    fn cell_edit(&mut self, id: Id, r: Rect, text: &str, align: Align) -> Option<String> {
        let t = self.theme;
        if self.edit != id {
            self.text_in(r, text, t.text, align);
            // No `interact` here: the press must stay with the row so that
            // single clicks keep selecting it.
            if self.dbl && self.hit(r) {
                self.edit = id;
                self.edit_buf = text.to_string();
                self.focus = id;
                self.caret = text.len();
                self.sel = self.caret;
                self.edit_scroll = 0.0;
            }
            return None;
        }
        let cancel = self.input.keys.contains(&Key::Escape);
        let commit = self.input.keys.contains(&Key::Enter);
        let mut buf = std::mem::take(&mut self.edit_buf);

        // The field covers the cell including its padding, so the text does
        // not move when editing starts. That needs the clip of the cell widened.
        let field = Rect::new(r.x - t.pad.x, r.y, r.w + 2.0 * t.pad.x, r.h);
        let cell_clip = if self.clips.len() >= 2 { self.clips.pop() } else { None };
        self.push_clip(field);
        self.theme.text = Theme::ink(t.field); // rows recolor text when selected
        self.text_edit_at(id, field, &mut buf, align == Align::Right);
        self.theme.text = t.text;
        self.pop_clip();
        if let Some(c) = cell_clip {
            self.push_clip(c);
        }

        if cancel {
            self.edit = 0;
            self.focus = 0;
            return None;
        }
        if commit || self.focus != id {
            self.edit = 0;
            return Some(buf);
        }
        self.edit_buf = buf;
        None
    }

    /// Editable text cell.
    pub fn cell_text(&mut self, value: &mut String) -> bool {
        let id = self.id("cell");
        let r = self.cell_rect();
        match self.cell_edit(id, r, value, Align::Left) {
            Some(s) if s != *value => {
                *value = s;
                true
            }
            _ => false,
        }
    }

    /// Editable numeric cell for any integer or float type (`i32`, `u64`,
    /// `f32`, `f64`, ...), right-aligned. Input that does not parse as `T` is
    /// discarded; a decimal comma is accepted.
    pub fn cell_num<T: Copy + PartialEq + Display + FromStr>(&mut self, value: &mut T) -> bool {
        let id = self.id("cell");
        let r = self.cell_rect();
        let text = value.to_string();
        let Some(s) = self.cell_edit(id, r, &text, Align::Right) else { return false };
        match s.trim().replace(',', ".").parse::<T>() {
            Ok(v) if v != *value => {
                *value = v;
                true
            }
            _ => false,
        }
    }

    /// Boolean cell: a check box that toggles on double-click.
    pub fn cell_bool(&mut self, value: &mut bool) -> bool {
        let r = self.cell_rect();
        let bs = self.theme.font_size + 2.0;
        let toggled = self.dbl && self.hit(r);
        if toggled {
            *value = !*value;
        }
        self.mark_box(Rect::new(r.x, r.y + ((r.h - bs) / 2.0).floor(), bs, bs), *value, false, false);
        toggled
    }

    /// Color cell: a swatch plus its hex code (`#RRGGBB` or `#RRGGBBAA`),
    /// which is edited as text. The swatch previews the input while typing.
    pub fn cell_color(&mut self, value: &mut Color) -> bool {
        let id = self.id("cell");
        let r = self.cell_rect();
        let t = self.theme;
        let bs = t.font_size + 2.0;
        let swatch = Rect::new(r.x, r.y + ((r.h - bs) / 2.0).floor(), bs, bs);
        let shown = if self.edit == id { Color::parse(&self.edit_buf).unwrap_or(*value) } else { *value };
        self.fill(swatch, shown);
        self.border(swatch, t.border);
        let x = swatch.right() + t.pad.x * 2.0;
        let rest = Rect::new(x, r.y, (r.right() - x).max(0.0), r.h);
        match self.cell_edit(id, rest, &value.to_hex(), Align::Left).as_deref().and_then(Color::parse) {
            Some(c) if c != *value => {
                *value = c;
                true
            }
            _ => false,
        }
    }
}
