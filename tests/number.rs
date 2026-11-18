use drazui::*;

struct Fixture {
    ui: Ui,
    int: i32,
    small: u8,
    float: f64,
    changes: usize,
}

// Each field is 120 wide and 14 high (bitmap font), stacked without a gap:
// minus button on the left, plus button on the right, text in between.
const MINUS: f32 = 5.0;
const TEXT: f32 = 100.0; // right of the right-aligned number: caret at its end
const PLUS: f32 = 115.0;

impl Fixture {
    fn new() -> Fixture {
        let mut ui = Ui::with_bitmap_font();
        ui.theme.gap = 0.0; // rows directly below each other
        Fixture { ui, int: 5, small: 1, float: 0.1, changes: 0 }
    }

    fn frame(&mut self) {
        let Fixture { ui, int, small, float, changes } = self;
        ui.begin(400.0, 300.0);
        ui.w(Size::Px(120.0));
        *changes += ui.number("int", int, 1) as usize;
        ui.w(Size::Px(120.0));
        *changes += ui.number("small", small, 2) as usize;
        ui.w(Size::Px(120.0));
        *changes += ui.number("float", float, 0.1) as usize;
        ui.end();
    }

    fn click(&mut self, x: f32, row: usize) {
        self.ui.input.mouse = vec2(x, row as f32 * 14.0 + 5.0);
        for down in [false, true, false] {
            self.ui.input.down = down;
            self.frame();
        }
    }

    fn type_text(&mut self, s: &str) {
        self.ui.input.text.push_str(s);
        self.frame();
    }

    fn key(&mut self, k: Key) {
        self.ui.input.keys.push(k);
        self.frame();
    }
}

#[test]
fn buttons_step() {
    let mut f = Fixture::new();
    f.click(PLUS, 0);
    f.click(PLUS, 0);
    assert_eq!(f.int, 7);
    f.click(MINUS, 0);
    assert_eq!(f.int, 6);
    assert_eq!(f.changes, 3);
    assert!(!f.ui.wants_keyboard());
}

#[test]
fn integers_saturate() {
    let mut f = Fixture::new();
    f.click(MINUS, 1);
    assert_eq!(f.small, 0);
    assert_eq!(f.changes, 1);
    f.click(MINUS, 1);
    assert_eq!(f.small, 0);
    assert_eq!(f.changes, 1);
}

#[test]
fn floats_stay_clean() {
    let mut f = Fixture::new();
    f.click(PLUS, 2);
    f.click(PLUS, 2);
    assert_eq!(f.float, 0.3);
    f.click(MINUS, 2);
    assert_eq!(f.float, 0.2);
}

#[test]
fn holding_repeats() {
    let mut f = Fixture::new();
    f.ui.input.mouse = vec2(PLUS, 5.0);
    f.frame();
    f.ui.input.down = true;
    f.ui.input.dt = 0.1;
    f.frame();
    assert_eq!(f.int, 6, "steps on press");
    f.frame();
    f.frame();
    f.frame();
    assert_eq!(f.int, 6, "no repeat before the delay");
    for _ in 0..4 {
        f.frame();
    }
    assert!(f.int > 6, "repeats while held");
    let v = f.int;
    f.ui.input.down = false;
    f.frame();
    f.frame();
    assert_eq!(f.int, v);
}

#[test]
fn typing() {
    let mut f = Fixture::new();
    f.ui.input.dt = 0.5; // slow frames: repeated clicks are not double-clicks
    f.click(TEXT, 0);
    assert!(f.ui.wants_keyboard());
    f.type_text("0");
    assert_eq!(f.int, 5, "not committed while typing");
    f.key(Key::Enter);
    assert_eq!(f.int, 50);

    // Escape cancels, invalid input is discarded
    f.click(TEXT, 0);
    f.type_text("9");
    f.key(Key::Escape);
    assert_eq!(f.int, 50);
    f.click(TEXT, 0);
    f.type_text("x");
    f.key(Key::Enter);
    assert_eq!(f.int, 50);

    // a spinner button commits the typed text, then steps
    f.click(TEXT, 0);
    f.type_text("0");
    f.click(PLUS, 0);
    assert_eq!(f.int, 501);

    // decimal comma
    f.click(TEXT, 2);
    f.ui.input.keys.extend([Key::Backspace; 2]);
    f.type_text(",75");
    f.key(Key::Enter);
    assert_eq!(f.float, 0.75);
}
