//! Toolbar, menubar, menus and popup (context) menus.

use crate::geom::{vec2, Rect, Vec2};
use crate::input::Key;
use crate::layout::{Kind, Layout, Side};
use crate::ui::{sub, Align, Cmd, Id, Ui};

const SALT_SIZE: u64 = 0xE0;
const SALT_POS: u64 = 0xE1;
const MIN_W: f32 = 48.0;

impl Ui {
    // ---- toolbar -----------------------------------------------------------

    /// Horizontal bar docked to the top of the current layout. Put any
    /// widgets in it; [`tool`](Ui::tool) gives flat toolbar buttons.
    pub fn toolbar(&mut self, f: impl FnOnce(&mut Ui)) {
        let t = self.theme;
        self.dock(Side::Top, t.row_h() + t.margin * 2.0, |ui| ui.row(f));
    }

    fn flat(&mut self, text: &str, on: bool) -> bool {
        let id = self.id(text);
        let t = self.theme;
        let r = self.alloc(vec2(self.text_width(text) + t.pad.x * 2.0, t.row_h()));
        let resp = self.interact(id, r);
        if resp.held {
            self.fill(r, t.active);
        } else if resp.hovered {
            self.fill(r, t.hover);
        } else if on {
            self.fill(r, t.widget);
        }
        if on {
            self.fill(Rect::new(r.x, r.bottom() - 2.0, r.w, 2.0), t.accent);
        }
        self.text_in(r.inset_xy(t.pad.x, 0.0), text, t.text, Align::Center);
        resp.clicked
    }

    /// Flat toolbar button. Returns true when clicked.
    pub fn tool(&mut self, text: &str) -> bool {
        self.flat(text, false)
    }

    /// Flat toolbar toggle bound to `value`.
    pub fn tool_toggle(&mut self, text: &str, value: &mut bool) -> bool {
        let clicked = self.flat(text, *value);
        if clicked {
            *value = !*value;
        }
        clicked
    }

    // ---- menus -------------------------------------------------------------

    /// Menu bar docked to the top of the current layout; fill it with
    /// [`menu`](Ui::menu)s. While one menu is open, hovering another opens it.
    pub fn menubar(&mut self, f: impl FnOnce(&mut Ui)) {
        let h = self.row_h();
        self.dock_with(Side::Top, h, 0.0, |ui| {
            ui.push_id("menubar");
            ui.row(|ui| {
                ui.top().gap = 0.0;
                f(ui);
            });
            ui.pop_id();
        });
    }

    /// Drop-down menu: a flat header that opens a popup with `f` as content
    /// (typically [`menu_item`](Ui::menu_item)s). Works in a menubar or anywhere else.
    pub fn menu(&mut self, label: &str, f: impl FnOnce(&mut Ui)) {
        let id = self.id(label);
        let group = *self.ids.last().unwrap();
        let t = self.theme;
        let r = self.alloc(vec2(self.text_width(label) + t.pad.x * 2.0, t.row_h()));
        let resp = self.interact(id, r);
        if resp.clicked {
            if self.open == id {
                self.open = 0;
            } else {
                self.open = id;
                self.open_group = group;
                self.just_opened = id;
            }
        } else if self.open != 0 && self.open != id && self.open_group == group && self.hit(r) {
            // another menu of this bar is open: follow the mouse
            self.open = id;
            self.just_opened = id;
        }
        if self.open == id {
            self.fill(r, t.active);
        } else if resp.hovered {
            self.fill(r, t.hover);
        }
        self.text_in(r.inset_xy(t.pad.x, 0.0), label, t.text, Align::Center);
        self.popup(id, vec2(r.x, r.bottom()), r, f);
    }

    /// Entry of a menu or popup. Returns true when clicked and closes the menu.
    pub fn menu_item(&mut self, text: &str) -> bool {
        let clicked = self.selectable(text, false);
        if clicked {
            self.open = 0;
        }
        clicked
    }

    /// Checkable menu entry bound to `value`. The menu stays open.
    pub fn menu_check(&mut self, text: &str, value: &mut bool) -> bool {
        let id = self.id(text);
        let t = self.theme;
        let bs = t.font_size + 2.0;
        let r = self.alloc(vec2(bs + self.text_width(text) + t.pad.x * 3.0, t.row_h()));
        let resp = self.interact(id, r);
        if resp.clicked {
            *value = !*value;
        }
        if resp.hovered {
            self.fill(r, t.hover);
        }
        let b = Rect::new(r.x + t.pad.x, r.y + ((r.h - bs) / 2.0).floor(), bs, bs);
        self.mark_box(b, *value, false, false);
        self.text_in(Rect::new(b.right() + t.pad.x, r.y, r.right() - b.right() - t.pad.x * 2.0, r.h), text, t.text, Align::Left);
        resp.clicked
    }

    // ---- popup menus -------------------------------------------------------

    /// Opens the popup menu `id` at the mouse position. Pair it with
    /// [`popup_menu`](Ui::popup_menu) using the same id in the same id scope.
    pub fn open_popup(&mut self, id: &str) {
        let id = self.id(id);
        self.set_state(sub(id, SALT_POS), self.mouse);
        self.open = id;
        self.open_group = 0;
        self.just_opened = id;
    }

    /// Declares the content of popup menu `id`; drawn only while it is open.
    pub fn popup_menu(&mut self, id: &str, f: impl FnOnce(&mut Ui)) {
        let id = self.id(id);
        let pos = self.state(sub(id, SALT_POS), Vec2::ZERO);
        self.popup(id, pos, Rect::ZERO, f);
    }

    /// Popup menu that opens on a right-click on the previous widget or container.
    pub fn context_menu(&mut self, id: &str, f: impl FnOnce(&mut Ui)) {
        if self.rpressed && self.hit(self.last) {
            self.open_popup(id);
        }
        self.popup_menu(id, f);
    }

    /// Draws popup `id` at `pos` while it is open. It is sized to its content
    /// (measured invisibly in the first frame) and drawn above everything else.
    /// A press outside of the popup and `anchor`, or Escape, closes it.
    fn popup(&mut self, id: Id, pos: Vec2, anchor: Rect, f: impl FnOnce(&mut Ui)) {
        if self.open != id {
            return;
        }
        let t = self.theme;
        let size = self.state(sub(id, SALT_SIZE), Vec2::ZERO);
        let measured = size.x > 0.0;
        let x = pos.x.min(self.screen.right() - size.x).max(0.0);
        let y = pos.y.min(self.screen.bottom() - size.y).max(0.0);
        let pr = Rect::new(x, y, size.x, size.y);
        let outside = (self.pressed || self.rpressed) && !pr.contains(self.mouse) && !anchor.contains(self.mouse);
        if self.just_opened != id && (outside || self.input.keys.contains(&Key::Escape)) {
            self.open = 0;
            return;
        }
        std::mem::swap(&mut self.cmds, &mut self.overlay);
        let clip = if measured { self.screen } else { Rect::ZERO };
        self.clips.push(clip);
        self.cmds.push(Cmd::Clip(self.dev(clip)));
        self.in_popup = true;
        if measured {
            self.popup_cur = pr;
        }
        self.fill(pr, t.panel);
        self.border(pr, t.accent);
        self.ids.push(id);
        self.layouts.push(Layout::new(Kind::Col, pr.inset(1.0), 0.0));
        f(self);
        let l = self.pop_layout();
        self.ids.pop();
        self.set_state(sub(id, SALT_SIZE), vec2(l.want.max(MIN_W) + 2.0, l.max.y - l.rect.y + 2.0));
        self.in_popup = false;
        self.clips.pop();
        std::mem::swap(&mut self.cmds, &mut self.overlay);
    }
}
