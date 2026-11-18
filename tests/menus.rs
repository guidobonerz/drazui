use drazui::*;

fn frame(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
    ui.begin(400.0, 300.0);
    f(ui);
    ui.end();
}

fn click_with(ui: &mut Ui, p: Vec2, right: bool, mut f: impl FnMut(&mut Ui)) {
    ui.input.mouse = p;
    for down in [false, true, false] {
        ui.input.down = down && !right;
        ui.input.right = down && right;
        frame(ui, &mut f);
    }
}

fn click(ui: &mut Ui, p: Vec2, f: impl FnMut(&mut Ui)) {
    click_with(ui, p, false, f);
}

fn shows(ui: &Ui, s: &str) -> bool {
    ui.draws().any(|d| matches!(d, Draw::Text { text, .. } if text == s))
}

#[test]
fn menubar_menu_item() {
    let mut ui = Ui::with_bitmap_font();
    let (mut opened, mut snap) = (0, false);
    let mut build = |ui: &mut Ui| {
        ui.menubar(|ui| {
            ui.menu("File", |ui| opened += ui.menu_item("Open") as i32);
            ui.menu("View", |ui| {
                ui.menu_check("Snap", &mut snap);
            });
        });
    };
    frame(&mut ui, &mut build);
    assert!(!shows(&ui, "Open"), "menu starts closed");

    click(&mut ui, vec2(5.0, 5.0), &mut build);
    frame(&mut ui, &mut build);
    assert!(shows(&ui, "Open"));

    // hovering the next header switches menus without a click
    ui.input.mouse = vec2(8.0 * 4.0 + 8.0 + 5.0, 5.0);
    frame(&mut ui, &mut build);
    frame(&mut ui, &mut build);
    assert!(shows(&ui, "Snap") && !shows(&ui, "Open"));

    // a check item toggles its value and keeps the menu open
    let item = vec2(ui.input.mouse.x, 14.0 + 1.0 + 5.0);
    click(&mut ui, item, &mut build);
    frame(&mut ui, &mut build);
    assert!(shows(&ui, "Snap"));

    // back to "File" by hovering; a normal item reports the click and closes the menu
    ui.input.mouse = vec2(5.0, 5.0);
    frame(&mut ui, &mut build);
    click(&mut ui, vec2(5.0, 14.0 + 1.0 + 5.0), &mut build);
    frame(&mut ui, &mut build);
    assert!(!shows(&ui, "Open"));
    assert_eq!(opened, 1);
    assert!(snap);
}

#[test]
fn context_menu_opens_on_right_click() {
    let mut ui = Ui::with_bitmap_font();
    let hits = std::cell::Cell::new(0);
    let mut build = |ui: &mut Ui| {
        ui.button("target");
        ui.context_menu("ctx", |ui| hits.set(hits.get() + ui.menu_item("Do it") as i32));
    };
    // a right-click next to the widget does nothing
    click_with(&mut ui, vec2(200.0, 100.0), true, &mut build);
    frame(&mut ui, &mut build);
    assert!(!shows(&ui, "Do it"));

    click_with(&mut ui, vec2(20.0, 5.0), true, &mut build);
    frame(&mut ui, &mut build);
    assert!(shows(&ui, "Do it"));

    click(&mut ui, vec2(25.0, 11.0), &mut build);
    frame(&mut ui, &mut build);
    assert_eq!(hits.get(), 1);
    assert!(!shows(&ui, "Do it"));

    // a click elsewhere closes it without triggering the item
    click_with(&mut ui, vec2(20.0, 5.0), true, &mut build);
    click(&mut ui, vec2(300.0, 200.0), &mut build);
    frame(&mut ui, &mut build);
    assert_eq!(hits.get(), 1);
    assert!(!shows(&ui, "Do it"));
}

#[test]
fn toast_expires() {
    let mut ui = Ui::with_bitmap_font();
    ui.toast_secs = 1.0;
    ui.toast("saved");
    ui.input.dt = 0.4;
    frame(&mut ui, |_| {});
    assert!(shows(&ui, "saved"));
    frame(&mut ui, |_| {});
    assert!(shows(&ui, "saved"));
    frame(&mut ui, |_| {}); // 1.2 s elapsed after this frame
    frame(&mut ui, |_| {});
    assert!(!shows(&ui, "saved"));
}

#[test]
fn chips_and_tools() {
    let mut ui = Ui::with_bitmap_font();
    let (mut on, mut tool, mut tags) = (false, false, vec!["a", "b"]);
    let mut build = |ui: &mut Ui| {
        ui.toolbar(|ui| {
            ui.tool_toggle("T", &mut tool);
        });
        ui.chip_toggle("chip", &mut on);
        ui.row(|ui| tags.retain(|t| !ui.chip_close(t)));
    };
    click(&mut ui, vec2(8.0, 8.0), &mut build); // toolbar: 4 px margin
    click(&mut ui, vec2(5.0, 22.0 + 5.0), &mut build);
    // second row: chip "a" is 8 + 8 + 6 + 4 = 26 wide, its cross sits at the right end
    click(&mut ui, vec2(5.0, 38.0 + 5.0), &mut build); // on the text: no removal
    click(&mut ui, vec2(20.0, 38.0 + 5.0), &mut build); // on the cross
    assert!(tool);
    assert!(on);
    assert_eq!(tags, ["b"]);
}
