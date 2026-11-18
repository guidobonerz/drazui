use drazui::*;

struct Fixture {
    ui: Ui,
    line: String,
    area: String,
}

// Bitmap font, 8 px per character. A single-line field on top (text starts at
// x = 4, row center y = 7) and a text area below it (text starts at x = 5,
// line `n` is centered at y = 22 + 11 n).
impl Fixture {
    fn new(line: &str, area: &str) -> Fixture {
        let mut ui = Ui::with_bitmap_font();
        ui.theme.gap = 0.0;
        Fixture { ui, line: line.into(), area: area.into() }
    }

    fn frame(&mut self) {
        let Fixture { ui, line, area } = self;
        ui.begin(400.0, 300.0);
        ui.w(Size::Px(120.0)).text_edit("line", line);
        ui.size(Size::Px(200.0), Size::Px(80.0)).text_area("area", area);
        ui.end();
    }

    fn line_pos(col: usize) -> Vec2 {
        vec2(4.0 + 8.0 * col as f32, 7.0)
    }

    fn area_pos(col: usize, line: usize) -> Vec2 {
        vec2(5.0 + 8.0 * col as f32, 22.0 + 11.0 * line as f32)
    }

    fn click(&mut self, at: Vec2) {
        self.drag(at, at);
    }

    fn drag(&mut self, from: Vec2, to: Vec2) {
        self.ui.input.mouse = from;
        self.frame();
        self.ui.input.down = true;
        self.frame();
        self.ui.input.mouse = to;
        self.frame();
        self.ui.input.down = false;
        self.frame();
    }

    fn type_text(&mut self, s: &str) {
        self.ui.input.text.push_str(s);
        self.frame();
    }

    fn keys(&mut self, keys: &[Key]) {
        for k in keys {
            self.ui.input.keys.push(*k);
            self.frame();
        }
    }

    fn shift_keys(&mut self, keys: &[Key]) {
        self.ui.input.shift = true;
        self.keys(keys);
        self.ui.input.shift = false;
    }
}

#[test]
fn shift_arrows_copy_cut_paste() {
    let mut f = Fixture::new("hello world", "");
    f.click(Fixture::line_pos(5));
    f.shift_keys(&[Key::Right; 6]);
    f.keys(&[Key::Copy]);
    assert_eq!(f.ui.take_copied(), Some(" world"));
    assert_eq!(f.ui.take_copied(), None, "reported once");
    assert_eq!(f.line, "hello world");

    f.keys(&[Key::Cut]);
    assert_eq!(f.line, "hello");
    assert_eq!(f.ui.take_copied(), Some(" world"));
    f.keys(&[Key::Home, Key::Paste]);
    assert_eq!(f.line, " worldhello");

    // nothing selected: copy and cut leave text and clipboard alone
    f.keys(&[Key::Copy, Key::Cut]);
    assert_eq!(f.ui.take_copied(), None);
    assert_eq!(f.ui.clipboard, " world");
    assert_eq!(f.line, " worldhello");
}

#[test]
fn typing_and_paste_replace_the_selection() {
    let mut f = Fixture::new("hello world", "");
    f.click(Fixture::line_pos(3));
    f.keys(&[Key::SelectAll]);
    f.type_text("x");
    assert_eq!(f.line, "x");

    f.shift_keys(&[Key::Home]);
    f.ui.clipboard = "one\r\ntwo".into();
    f.keys(&[Key::Paste]);
    assert_eq!(f.line, "one two", "line breaks become spaces in a single-line field");
}

#[test]
fn mouse_selects() {
    let mut f = Fixture::new("hello world", "");
    f.drag(Fixture::line_pos(0), Fixture::line_pos(5));
    f.keys(&[Key::Backspace]);
    assert_eq!(f.line, " world", "drag");

    f.line = "hello world".into();
    f.click(Fixture::line_pos(2));
    f.ui.input.shift = true;
    f.click(Fixture::line_pos(5));
    f.ui.input.shift = false;
    f.keys(&[Key::Delete]);
    assert_eq!(f.line, "he world", "shift + click");

    f.line = "hello world".into();
    f.click(Fixture::line_pos(8));
    f.click(Fixture::line_pos(8));
    f.type_text("X");
    assert_eq!(f.line, "hello X", "double-click selects the word");
}

#[test]
fn arrows_collapse_the_selection() {
    let mut f = Fixture::new("hello world", "");
    f.click(Fixture::line_pos(0));
    f.shift_keys(&[Key::Right; 5]);
    f.keys(&[Key::Right]);
    f.type_text("!");
    assert_eq!(f.line, "hello! world");

    f.shift_keys(&[Key::Left; 3]);
    f.keys(&[Key::Left]);
    f.type_text("?");
    assert_eq!(f.line, "hel?lo! world");
}

#[test]
fn text_area_selects_across_lines() {
    let mut f = Fixture::new("", "one\ntwo\nthree");
    f.click(Fixture::area_pos(1, 0));
    f.shift_keys(&[Key::Down, Key::Down]);
    f.keys(&[Key::Cut]);
    assert_eq!(f.area, "ohree");
    assert_eq!(f.ui.clipboard, "ne\ntwo\nt");

    f.keys(&[Key::End]);
    f.ui.clipboard = "\r\na\tb\r\n".into();
    f.keys(&[Key::Paste]);
    assert_eq!(f.area, "ohree\na b\n", "line breaks are kept, tabs become spaces");

    f.area = "one\ntwo\nthree".into();
    f.drag(Fixture::area_pos(0, 0), Fixture::area_pos(3, 1));
    f.keys(&[Key::Backspace]);
    assert_eq!(f.area, "\nthree", "drag");

    f.keys(&[Key::SelectAll, Key::Enter]);
    assert_eq!(f.area, "\n", "Enter replaces the selection");
}

#[test]
fn selection_belongs_to_the_focused_field() {
    let mut f = Fixture::new("hello", "world");
    f.click(Fixture::line_pos(0));
    f.keys(&[Key::SelectAll]);
    // moving to the other field starts without a selection there
    f.click(Fixture::area_pos(2, 0));
    f.keys(&[Key::Backspace]);
    assert_eq!((f.line.as_str(), f.area.as_str()), ("hello", "wrld"));
}
