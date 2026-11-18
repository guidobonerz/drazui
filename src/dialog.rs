//! Modal dialogs: drawn centered above everything else; while one is open the
//! rest of the UI does not react to the mouse.

use crate::geom::{vec2, Color, Rect, Vec2};
use crate::input::Key;
use crate::layout::{Kind, Layout, Size};
use crate::ui::{sub, Align, Cmd, Ui};

const SALT_SIZE: u64 = 0xF0;
/// Smallest content width of a dialog and smallest width of its buttons.
const MIN_W: f32 = 160.0;
const BUTTON_W: f32 = 56.0;
/// Opacity of the shade over the UI behind the dialog.
const SHADE: u8 = 110;

impl Ui {
    /// Opens the modal dialog `id`. Pair it with [`dialog`](Ui::dialog) or
    /// [`confirm`](Ui::confirm) using the same id in the same id scope.
    /// Only one dialog is open at a time; an open menu is closed.
    pub fn open_dialog(&mut self, id: &str) {
        self.modal = self.id(id);
        self.modal_opened = true;
        self.open = 0;
        // text fields behind the dialog must not keep the keyboard
        self.focus = 0;
    }

    /// Closes the open dialog, if any. Call it from the content of a
    /// [`dialog`](Ui::dialog), e.g. when one of its buttons is clicked.
    pub fn close_dialog(&mut self) {
        self.modal = 0;
    }

    /// True while a dialog is open.
    pub fn dialog_open(&self) -> bool {
        self.modal != 0
    }

    /// Space between the border of a dialog and its content.
    fn dialog_pad(&self) -> f32 {
        self.theme.margin + self.theme.pad.x
    }

    /// Declares modal dialog `id` with `f` as content; drawn only while it is
    /// open. It is sized to its content (measured invisibly in the first
    /// frame) and centered. Escape closes it.
    ///
    /// Call it once per frame, outside of other containers. Menus and
    /// [`select`](Ui::select)s do not work inside a dialog.
    pub fn dialog(&mut self, id: &str, title: &str, f: impl FnOnce(&mut Ui)) {
        let id = self.id(id);
        if self.modal != id {
            return;
        }
        // the key that opened the dialog must not close it again
        if !self.modal_opened && self.input.keys.contains(&Key::Escape) {
            self.modal = 0;
            return;
        }
        let t = self.theme;
        let (rh, p, screen) = (t.row_h(), self.dialog_pad(), self.screen);
        let size = self.state(sub(id, SALT_SIZE), Vec2::ZERO);
        let measured = size.x > 0.0;
        let x = ((screen.w - size.x) / 2.0).round().max(0.0);
        let y = ((screen.h - size.y) / 2.0).round().max(0.0);
        let dr = Rect::new(x, y, size.x, size.y);

        std::mem::swap(&mut self.cmds, &mut self.overlay);
        let clip = if measured { screen } else { Rect::ZERO };
        self.clips.push(clip);
        self.cmds.push(Cmd::Clip(self.dev(clip)));
        self.in_popup = true;
        // covers the whole screen: nothing behind the dialog is hit
        self.popup_cur = screen;

        self.fill(screen, Color::hex(0x000000).alpha(SHADE));
        self.fill(dr, t.panel);
        let head = Rect::new(dr.x, dr.y, dr.w, rh);
        self.fill(head, t.widget);
        self.text_in(head.inset_xy(p, 0.0), title, t.text, Align::Left);
        self.border(dr, t.accent);

        self.ids.push(id);
        let content = Rect::new(dr.x + p, dr.y + rh + p, dr.w - 2.0 * p, dr.h - rh - 2.0 * p);
        self.layouts.push(Layout::new(Kind::Col, content, t.gap));
        f(self);
        let l = self.pop_layout();
        self.ids.pop();
        let w = l.want.max(self.text_width(title)).max(MIN_W);
        self.set_state(sub(id, SALT_SIZE), vec2(w + 2.0 * p, l.max.y - l.rect.y + rh + 2.0 * p));

        self.in_popup = false;
        self.clips.pop();
        std::mem::swap(&mut self.cmds, &mut self.overlay);
    }

    /// Modal dialog `id` that asks a question: `text` (lines separated by
    /// `\n`) above a row of `buttons`. Returns the index of the chosen button
    /// once and closes the dialog. Enter chooses the first button, Escape the
    /// last one, so put the cancelling button last.
    ///
    /// ```
    /// # use drazui::*;
    /// # let mut ui = Ui::new();
    /// # ui.begin(640.0, 480.0);
    /// if ui.button("Delete") {
    ///     ui.open_dialog("delete");
    /// }
    /// if ui.confirm("delete", "Delete", "Really delete the file?", &["Delete", "Cancel"]) == Some(0) {
    ///     // delete it
    /// }
    /// # ui.end();
    /// ```
    pub fn confirm(&mut self, id: &str, title: &str, text: &str, buttons: &[&str]) -> Option<usize> {
        if self.modal != self.id(id) {
            return None;
        }
        let mut choice = None;
        if !self.modal_opened && !buttons.is_empty() {
            if self.input.keys.contains(&Key::Enter) {
                choice = Some(0);
            } else if self.input.keys.contains(&Key::Escape) {
                choice = Some(buttons.len() - 1);
            }
        }
        if choice.is_none() {
            let t = self.theme;
            let widest = |ui: &Ui, items: &mut dyn Iterator<Item = &str>| items.map(|s| ui.text_width(s)).fold(0.0, f32::max);
            // equally wide buttons, right-aligned
            let bw = (widest(self, &mut buttons.iter().copied()) + t.pad.x * 2.0).max(BUTTON_W);
            let row_w = bw * buttons.len() as f32 + t.gap * buttons.len().saturating_sub(1) as f32;
            let width = widest(self, &mut text.lines()).max(self.text_width(title)).max(MIN_W).max(row_w);
            self.dialog(id, title, |ui| {
                for line in text.lines() {
                    ui.label(line);
                }
                ui.space(t.margin);
                ui.row(|ui| {
                    if width - row_w > t.gap {
                        ui.alloc(vec2(width - row_w - t.gap, 0.0));
                    }
                    for (i, button) in buttons.iter().enumerate() {
                        if ui.w(Size::Px(bw)).button(button) {
                            choice = Some(i);
                        }
                    }
                });
            });
        }
        if choice.is_some() {
            self.modal = 0;
        }
        choice
    }
}
