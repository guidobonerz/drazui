use drazui::*;

struct Fixture {
    ui: Ui,
    text: String,
    changes: usize,
}

// Bitmap font: every character is 8 wide and a line is 11 high. The editor
// sits at the top-left, so its text starts at (5, 3).
impl Fixture {
    fn new(text: &str) -> Fixture {
        Fixture { ui: Ui::with_bitmap_font(), text: text.into(), changes: 0 }
    }

    fn frame(&mut self) {
        let Fixture { ui, text, changes } = self;
        ui.begin(400.0, 300.0);
        ui.size(Size::Px(200.0), Size::Px(80.0));
        *changes += ui.text_area("text", text) as usize;
        ui.end();
    }

    /// Clicks in front of character `col` of line `line`.
    fn click(&mut self, col: usize, line: usize) {
        self.ui.input.mouse = vec2(5.0 + 8.0 * col as f32, 8.0 + 11.0 * line as f32);
        for down in [false, true, false] {
            self.ui.input.down = down;
            self.frame();
        }
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

    fn shows(&self, line: &str) -> bool {
        self.ui.draws().any(|d| matches!(d, Draw::Text { text, .. } if text == line))
    }
}

#[test]
fn typing_makes_lines() {
    let mut f = Fixture::new("");
    f.type_text("x");
    assert_eq!(f.text, "", "needs focus");
    f.click(0, 0);
    assert!(f.ui.wants_keyboard());
    f.type_text("ab");
    f.keys(&[Key::Enter]);
    f.type_text("c");
    assert_eq!(f.text, "ab\nc");
    assert_eq!(f.changes, 3);
    f.keys(&[Key::Escape]);
    assert!(!f.ui.wants_keyboard());
}

#[test]
fn click_places_caret() {
    let mut f = Fixture::new("hello\nhi\nworld");
    f.click(2, 2);
    f.type_text("X");
    assert_eq!(f.text, "hello\nhi\nwoXrld");
    // right of a short line: its end; below the text: the last line
    f.click(10, 1);
    f.type_text("Y");
    assert_eq!(f.text, "hello\nhiY\nwoXrld");
    f.click(0, 5);
    f.type_text("Z");
    assert_eq!(f.text, "hello\nhiY\nZwoXrld");
}

#[test]
fn vertical_moves_keep_the_column() {
    let mut f = Fixture::new("hello\nhi\nworld");
    f.click(4, 0);
    f.keys(&[Key::Down]);
    f.type_text("X");
    assert_eq!(f.text, "hello\nhiX\nworld", "clamped to the short line");

    let mut f = Fixture::new("hello\nhi\nworld");
    f.click(4, 0);
    f.keys(&[Key::Down, Key::Down]);
    f.type_text("X");
    assert_eq!(f.text, "hello\nhi\nworlXd", "back in column 4");
    f.keys(&[Key::Up, Key::Up, Key::Up]);
    f.type_text("A");
    assert_eq!(f.text, "Ahello\nhi\nworlXd", "up on the first line goes to the start");
    f.keys(&[Key::PageDown]);
    f.type_text("E");
    assert_eq!(f.text, "Ahello\nhi\nworlXdE", "down past the last line goes to the end");
}

#[test]
fn home_end_and_joining_lines() {
    let mut f = Fixture::new("one\ntwo");
    f.click(1, 1);
    f.keys(&[Key::Home]);
    f.type_text("<");
    f.keys(&[Key::End]);
    f.type_text(">");
    assert_eq!(f.text, "one\n<two>");
    f.keys(&[Key::Home, Key::Backspace]);
    assert_eq!(f.text, "one<two>");
    f.keys(&[Key::Left, Key::Left, Key::Delete, Key::Right, Key::Delete]);
    assert_eq!(f.text, "oetwo>");
}

#[test]
fn multibyte_characters() {
    let mut f = Fixture::new("äö\nü");
    f.click(2, 0);
    f.keys(&[Key::Backspace, Key::Down]);
    f.type_text("ß");
    assert_eq!(f.text, "ä\nüß");
    // text replaced from outside while focused: the caret stays valid
    f.text = "é".into();
    f.keys(&[Key::Left]);
    f.type_text("x");
    assert_eq!(f.text, "xé");
}

#[test]
fn scrolls_to_the_caret() {
    let text: Vec<String> = (0..40).map(|i| format!("line {i}")).collect();
    let mut f = Fixture::new(&text.join("\n"));
    f.frame();
    assert!(f.shows("line 0") && !f.shows("line 39"), "only visible lines are drawn");
    f.click(0, 0);
    f.keys(&[Key::PageDown; 10]);
    f.frame();
    assert!(f.shows("line 39") && !f.shows("line 0"));
    f.keys(&[Key::PageUp; 10]);
    f.frame();
    assert!(f.shows("line 0") && !f.shows("line 39"));
}
