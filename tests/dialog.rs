use drazui::*;

const QUESTION: &str = "Really quit?";

fn frame(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
    ui.begin(400.0, 300.0);
    f(ui);
    ui.end();
}

fn click(ui: &mut Ui, p: Vec2, mut f: impl FnMut(&mut Ui)) {
    ui.input.mouse = p;
    for down in [false, true, false] {
        ui.input.down = down;
        frame(ui, &mut f);
    }
}

fn shows(ui: &Ui, s: &str) -> bool {
    ui.draws().any(|d| matches!(d, Draw::Text { text, .. } if text == s))
}

/// Center of the text `s` in the last frame.
fn text_pos(ui: &Ui, s: &str) -> Vec2 {
    ui.draws()
        .find_map(|d| match d {
            Draw::Text { pos, text, size, .. } if text == s => Some(vec2(pos.x + 4.0, pos.y + size / 2.0)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{s} is not shown"))
}

/// A button behind the dialog, a button that opens it, and the dialog.
struct App {
    behind: u32,
    choices: Vec<usize>,
}

impl App {
    fn build(&mut self, ui: &mut Ui) {
        self.behind += ui.button("Behind") as u32;
        if ui.button("Ask") {
            ui.open_dialog("quit");
        }
        self.choices.extend(ui.confirm("quit", "Quit", QUESTION, &["Yes", "No", "Cancel"]));
    }
}

fn opened() -> (Ui, App) {
    let mut ui = Ui::with_bitmap_font();
    let mut app = App { behind: 0, choices: Vec::new() };
    frame(&mut ui, |ui| app.build(ui));
    assert!(!shows(&ui, QUESTION) && !ui.dialog_open(), "the dialog starts closed");
    let ask = text_pos(&ui, "Ask");
    click(&mut ui, ask, |ui| app.build(ui));
    // one frame measures the dialog, the next one shows it
    frame(&mut ui, |ui| app.build(ui));
    assert!(shows(&ui, QUESTION) && shows(&ui, "Cancel") && ui.dialog_open());
    (ui, app)
}

#[test]
fn a_button_answers_and_closes() {
    let (mut ui, mut app) = opened();
    let no = text_pos(&ui, "No");
    click(&mut ui, no, |ui| app.build(ui));
    assert_eq!(app.choices, [1]);
    frame(&mut ui, |ui| app.build(ui));
    assert!(!shows(&ui, QUESTION) && !ui.dialog_open());
    assert_eq!(app.choices, [1], "the answer is returned once");
}

#[test]
fn enter_is_the_first_button_and_escape_the_last() {
    for (key, choice) in [(Key::Enter, 0), (Key::Escape, 2)] {
        let (mut ui, mut app) = opened();
        ui.input.keys.push(key);
        frame(&mut ui, |ui| app.build(ui));
        assert_eq!(app.choices, [choice]);
        assert!(!ui.dialog_open());
    }
}

#[test]
fn the_ui_behind_is_blocked_while_the_dialog_is_open() {
    let (mut ui, mut app) = opened();
    let behind = text_pos(&ui, "Behind");
    click(&mut ui, behind, |ui| app.build(ui));
    assert!(app.behind == 0 && app.choices.is_empty() && ui.dialog_open());

    // and works again afterwards
    ui.input.keys.push(Key::Escape);
    frame(&mut ui, |ui| app.build(ui));
    click(&mut ui, behind, |ui| app.build(ui));
    assert_eq!(app.behind, 1);
}

#[test]
fn the_key_that_opens_the_dialog_does_not_answer_it() {
    let mut ui = Ui::with_bitmap_font();
    let mut choice = None;
    ui.input.keys.push(Key::Enter);
    frame(&mut ui, |ui| {
        ui.open_dialog("quit");
        choice = ui.confirm("quit", "Quit", QUESTION, &["Yes", "No"]);
    });
    assert!(choice.is_none() && ui.dialog_open());
}

#[test]
fn a_dialog_with_own_content_is_centered_and_closed_from_inside() {
    let mut ui = Ui::with_bitmap_font();
    let mut build = |ui: &mut Ui| {
        ui.dialog("about", "About", |ui| {
            ui.label("drazui");
            if ui.button("Close") {
                ui.close_dialog();
            }
        });
    };
    frame(&mut ui, |ui| {
        ui.open_dialog("about");
        build(ui);
    });
    frame(&mut ui, &mut build);
    assert!(shows(&ui, "About") && shows(&ui, "drazui"));
    let (title, close) = (text_pos(&ui, "About"), text_pos(&ui, "Close"));
    assert!(title.x > 100.0 && title.x < 200.0 && title.y > 100.0 && close.y < 200.0, "{title:?} {close:?}");

    click(&mut ui, close, &mut build);
    frame(&mut ui, &mut build);
    assert!(!shows(&ui, "drazui") && !ui.dialog_open());
}
