//! Number field with a spinner button on each side: `[-][ 42 ][+]`.

use std::fmt::Display;
use std::str::FromStr;

use crate::geom::{vec2, Rect};
use crate::input::Key;
use crate::ui::{sub, Id, Ui};

/// Seconds a spinner button is held before it starts repeating, and the
/// interval between repeats.
const REPEAT_DELAY: f32 = 0.4;
const REPEAT_EVERY: f32 = 0.05;

/// Numeric types usable with [`Ui::number`]: all integers, `f32` and `f64`.
pub trait Num: Copy + PartialEq + Display + FromStr {
    /// `self ± by`. Integers saturate at their limits, floats are rounded to
    /// six decimals so that repeated steps do not accumulate binary noise.
    fn step(self, by: Self, up: bool) -> Self;
}

macro_rules! int {
    ($($t:ty)*) => {$(
        impl Num for $t {
            fn step(self, by: Self, up: bool) -> Self {
                if up { self.saturating_add(by) } else { self.saturating_sub(by) }
            }
        }
    )*};
}
int!(i8 i16 i32 i64 isize u8 u16 u32 u64 usize);

macro_rules! float {
    ($($t:ty)*) => {$(
        impl Num for $t {
            fn step(self, by: Self, up: bool) -> Self {
                let v = self as f64 + if up { by as f64 } else { -(by as f64) };
                ((v * 1e6).round() / 1e6) as $t
            }
        }
    )*};
}
float!(f32 f64);

impl Ui {
    /// Square `-` / `+` button. Fires on press and repeats while held.
    fn spin_button(&mut self, id: Id, r: Rect, plus: bool) -> bool {
        let t = self.theme;
        let resp = self.interact(id, r);
        let mut fire = resp.pressed;
        if resp.pressed {
            self.set_state(id, vec2(0.0, 0.0));
        } else if resp.held {
            let dt = if self.input.dt > 0.0 { self.input.dt } else { 1.0 / 60.0 };
            let mut held = self.state(id, vec2(0.0, 0.0)).x + dt;
            if held >= REPEAT_DELAY {
                held -= REPEAT_EVERY;
                fire = true;
            }
            self.set_state(id, vec2(held, 0.0));
        }
        let bg = if resp.held {
            t.active
        } else if resp.hovered {
            t.hover
        } else {
            t.widget
        };
        self.fill(r, bg);
        self.border(r, t.border);
        let arm = (t.font_size / 3.0).floor();
        let (cx, cy) = ((r.x + r.w / 2.0).floor(), (r.y + r.h / 2.0).floor());
        self.fill(Rect::new(cx - arm, cy, arm * 2.0 + 1.0, 1.0), t.text);
        if plus {
            self.fill(Rect::new(cx, cy - arm, 1.0, arm * 2.0 + 1.0), t.text);
        }
        fire
    }

    /// Number field bound to `value`, with a decrement button on the left and
    /// an increment button on the right that change it by `step`.
    ///
    /// The number can also be typed: Enter or a click elsewhere commits,
    /// Escape cancels, input that does not parse as `T` is discarded (a
    /// decimal comma is accepted). Works for every integer type, `f32` and
    /// `f64`. Returns true when the value changed.
    pub fn number<T: Num>(&mut self, id: &str, value: &mut T, step: T) -> bool {
        let id = self.id(id);
        let rh = self.row_h();
        let r = self.alloc(vec2(120.0, rh));
        let bw = rh.min(r.w / 3.0);
        let old = *value;

        let down = self.spin_button(sub(id, 1), Rect::new(r.x, r.y, bw, r.h), false);
        let up = self.spin_button(sub(id, 2), Rect::new(r.right() - bw, r.y, bw, r.h), true);

        // The field shares its side borders with the buttons.
        let field = Rect::new(r.x + bw - 1.0, r.y, (r.w - 2.0 * bw + 2.0).max(0.0), r.h);
        let editing = self.edit == id;
        let cancel = editing && self.input.keys.contains(&Key::Escape);
        let mut buf = if editing { std::mem::take(&mut self.edit_buf) } else { value.to_string() };
        self.text_edit_at(id, field, &mut buf, true);
        let focused = self.focus == id;
        if editing && !cancel && focused {
            self.edit_buf = buf;
        } else if editing {
            // Enter, Escape or focus lost (also by pressing a spinner button)
            self.edit = 0;
            if !cancel {
                if let Ok(v) = buf.trim().replace(',', ".").parse::<T>() {
                    *value = v;
                }
            }
        } else if focused {
            self.edit = id;
            self.edit_buf = buf;
        }

        if down || up {
            *value = value.step(step, up);
        }
        *value != old
    }
}
