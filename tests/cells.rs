use drazui::*;

struct Row {
    name: String,
    count: i32,
    ratio: f64,
    done: bool,
    color: Color,
}

struct Fixture {
    ui: Ui,
    rows: Vec<Row>,
    sel: Option<usize>,
    changes: usize,
}

const COLS: [(&str, f32); 5] = [("Name", 80.0), ("Count", 60.0), ("Ratio", 60.0), ("Done", 40.0), ("Color", 100.0)];
// x inside each column, y inside the first / second row (1 px border + 14 px header)
const X: [f32; 5] = [20.0, 100.0, 160.0, 215.0, 290.0];
const Y: [f32; 2] = [20.0, 34.0];

impl Fixture {
    fn new() -> Fixture {
        let row = |name: &str| Row { name: name.into(), count: 5, ratio: 2.0, done: false, color: Color::hex(0xff8a00) };
        Fixture { ui: Ui::with_bitmap_font(), rows: vec![row("ab"), row("cd")], sel: None, changes: 0 }
    }

    fn frame(&mut self) {
        let Fixture { ui, rows, sel, changes } = self;
        ui.begin(400.0, 300.0);
        ui.table("t", &COLS, rows.len(), sel, |ui, r, c| {
            let row = &mut rows[r];
            *changes += match c {
                0 => ui.cell_text(&mut row.name),
                1 => ui.cell_num(&mut row.count),
                2 => ui.cell_num(&mut row.ratio),
                3 => ui.cell_bool(&mut row.done),
                _ => ui.cell_color(&mut row.color),
            } as usize;
        });
        ui.end();
    }

    fn click(&mut self, x: f32, y: f32) {
        self.ui.input.mouse = vec2(x, y);
        for down in [false, true, false] {
            self.ui.input.down = down;
            self.frame();
        }
    }

    fn double_click(&mut self, x: f32, y: f32) {
        self.click(x, y);
        self.click(x, y);
    }

    fn type_text(&mut self, s: &str) {
        self.ui.input.text.push_str(s);
        self.frame();
    }

    fn key(&mut self, k: Key) {
        self.ui.input.keys.push(k);
        self.frame();
    }

    /// Lets enough time pass that the next click does not count as a double-click.
    fn wait(&mut self) {
        self.ui.input.dt = 1.0;
        self.frame();
        self.ui.input.dt = 0.0;
    }
}

#[test]
fn single_click_selects_but_does_not_edit() {
    let mut f = Fixture::new();
    f.click(X[0], Y[1]);
    assert_eq!(f.sel, Some(1));
    assert!(!f.ui.wants_keyboard());
    f.type_text("x");
    assert_eq!(f.rows[1].name, "cd");

    // two slow clicks are not a double-click
    f.wait();
    f.click(X[0], Y[1]);
    assert!(!f.ui.wants_keyboard());
    f.click(X[3], Y[1]);
    f.wait();
    f.click(X[3], Y[1]);
    assert!(!f.rows[1].done);
    assert_eq!(f.changes, 0);
}

#[test]
fn text_cell() {
    let mut f = Fixture::new();
    f.double_click(X[0], Y[0]);
    assert!(f.ui.wants_keyboard());
    f.type_text("c");
    assert_eq!(f.rows[0].name, "ab", "not committed while typing");
    f.key(Key::Enter);
    assert_eq!(f.rows[0].name, "abc");
    assert_eq!(f.changes, 1);
    assert!(!f.ui.wants_keyboard());

    // Escape cancels
    f.wait();
    f.double_click(X[0], Y[0]);
    f.type_text("zzz");
    f.key(Key::Escape);
    assert_eq!(f.rows[0].name, "abc");

    // a click elsewhere commits
    f.wait();
    f.double_click(X[0], Y[1]);
    f.type_text("!");
    f.wait();
    f.click(300.0, 200.0);
    assert_eq!(f.rows[1].name, "cd!");
    assert_eq!(f.changes, 2);
}

#[test]
fn numeric_cells() {
    let mut f = Fixture::new();
    f.double_click(X[1], Y[0]);
    f.type_text("0");
    f.key(Key::Enter);
    assert_eq!(f.rows[0].count, 50);

    // not an integer: the old value stays
    f.wait();
    f.double_click(X[1], Y[0]);
    f.type_text(".5");
    f.key(Key::Enter);
    assert_eq!(f.rows[0].count, 50);

    // floats, with a decimal comma
    f.wait();
    f.double_click(X[2], Y[0]);
    f.type_text(",25");
    f.key(Key::Enter);
    assert_eq!(f.rows[0].ratio, 2.25);
    f.wait();
    f.double_click(X[2], Y[1]);
    f.ui.input.keys.extend([Key::Backspace; 1]);
    f.type_text("-1.5e2");
    f.key(Key::Enter);
    assert_eq!(f.rows[1].ratio, -150.0);
    assert_eq!(f.changes, 3);
}

#[test]
fn bool_cell() {
    let mut f = Fixture::new();
    f.double_click(X[3], Y[1]);
    assert!(f.rows[1].done && !f.rows[0].done);
    f.wait();
    f.double_click(X[3], Y[1]);
    assert!(!f.rows[1].done);
    assert_eq!(f.changes, 2);
}

#[test]
fn color_cell() {
    let mut f = Fixture::new();
    f.double_click(X[4], Y[0]);
    f.ui.input.keys.extend([Key::Backspace; 2]);
    f.type_text("ff");
    f.key(Key::Enter);
    assert_eq!(f.rows[0].color, Color::hex(0xff8aff));

    // invalid input keeps the old color
    f.wait();
    f.double_click(X[4], Y[0]);
    f.type_text("xyz");
    f.key(Key::Enter);
    assert_eq!(f.rows[0].color, Color::hex(0xff8aff));
    assert_eq!(f.changes, 1);
}

#[test]
fn color_text_roundtrip() {
    assert_eq!(Color::parse("#f80"), Some(Color::hex(0xff8800)));
    assert_eq!(Color::parse("3ddc5a"), Some(Color::hex(0x3ddc5a)));
    assert_eq!(Color::parse("#11223344"), Some(Color(0x11223344)));
    assert_eq!(Color::parse("#12"), None);
    assert_eq!(Color::parse("#+12345"), None);
    assert_eq!(Color::hex(0x0a0b0c).to_hex(), "#0a0b0c");
    assert_eq!(Color(0x0a0b0c80).to_hex(), "#0a0b0c80");
}
