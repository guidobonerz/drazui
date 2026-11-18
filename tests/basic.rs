use drazui::*;

fn frame(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
    ui.begin(400.0, 300.0);
    f(ui);
    ui.end();
}

/// Press and release the mouse at `p` (device pixels), running `f` each frame.
fn click(ui: &mut Ui, p: Vec2, mut f: impl FnMut(&mut Ui)) {
    ui.input.mouse = p;
    for down in [false, true, false] {
        ui.input.down = down;
        frame(ui, &mut f);
    }
}

#[test]
fn button_click() {
    let mut ui = Ui::with_bitmap_font();
    let mut n = 0;
    click(&mut ui, vec2(5.0, 5.0), |ui| n += ui.button("ok") as i32);
    assert_eq!(n, 1);
    click(&mut ui, vec2(5.0, 50.0), |ui| n += ui.button("ok") as i32);
    assert_eq!(n, 1);
}

#[test]
fn scale_maps_input() {
    let mut ui = Ui::with_bitmap_font();
    ui.theme.scale = 3.0;
    let mut n = 0;
    // logical row height is 14 → the button covers device y 0..42
    click(&mut ui, vec2(5.0, 40.0), |ui| n += ui.button("ok") as i32);
    assert_eq!(n, 1);
}

#[test]
fn checkbox_and_radio_bind() {
    let mut ui = Ui::with_bitmap_font();
    let (mut on, mut mode) = (false, 0);
    let mut build = |ui: &mut Ui| {
        ui.checkbox("on", &mut on);
        ui.radio("a", &mut mode, 0);
        ui.radio("b", &mut mode, 1);
    };
    click(&mut ui, vec2(5.0, 5.0), &mut build);
    click(&mut ui, vec2(5.0, 16.0 * 2.0 + 5.0), &mut build);
    assert!(on);
    assert_eq!(mode, 1);
}

#[test]
fn text_edit_binds() {
    let mut ui = Ui::with_bitmap_font();
    let mut s = String::from("ab");
    click(&mut ui, vec2(390.0, 5.0), |ui| {
        ui.text_edit("t", &mut s);
    });
    assert!(ui.wants_keyboard());
    ui.input.text.push_str("cé");
    frame(&mut ui, |ui| assert!(ui.text_edit("t", &mut s)));
    assert_eq!(s, "abcé");
    ui.input.keys.extend([Key::Backspace, Key::Home, Key::Delete]);
    frame(&mut ui, |ui| assert!(ui.text_edit("t", &mut s)));
    assert_eq!(s, "bc");
    // outside changes show up too: the widget just reads the value
    s = "xyz".into();
    ui.input.keys.push(Key::Delete); // caret is still at the start
    frame(&mut ui, |ui| {
        ui.text_edit("t", &mut s);
    });
    assert_eq!(s, "yz");
}

#[test]
fn select_picks_item() {
    let mut ui = Ui::with_bitmap_font();
    let mut sel = 0;
    let mut changed = false;
    let mut build = |ui: &mut Ui| changed |= ui.select("s", &mut sel, &["one", "two", "three"]);
    click(&mut ui, vec2(5.0, 5.0), &mut build); // open
    click(&mut ui, vec2(5.0, 14.0 + 1.0 + 14.0 + 5.0), &mut build); // second item
    assert_eq!(sel, 1);
    assert!(changed);
}

#[test]
fn layouts() {
    let mut ui = Ui::with_bitmap_font();
    frame(&mut ui, |ui| {
        let mut r = [Rect::ZERO; 3];
        ui.row(|ui| {
            r[0] = ui.w(Size::Px(10.0)).alloc(vec2(0.0, 5.0));
            r[1] = ui.alloc(vec2(20.0, 8.0));
            r[2] = ui.w(Size::Fill).alloc(vec2(0.0, 5.0));
        });
        assert_eq!(r[0], Rect::new(0.0, 0.0, 10.0, 5.0));
        assert_eq!(r[1], Rect::new(12.0, 0.0, 20.0, 8.0));
        assert_eq!(r[2], Rect::new(34.0, 0.0, 366.0, 5.0));
        // the row is as high as its tallest child; the next item follows the gap
        assert_eq!(ui.alloc(vec2(1.0, 1.0)), Rect::new(0.0, 10.0, 400.0, 1.0));

        let mut g = vec![];
        ui.grid(&[Size::Px(98.0), Size::Fill, Size::Fill], |ui| {
            for _ in 0..4 {
                g.push(ui.alloc(vec2(1.0, 6.0)));
            }
        });
        assert_eq!(g[1], Rect::new(100.0, 13.0, 149.0, 6.0));
        assert_eq!(g[2].x, 251.0);
        assert_eq!(g[3], Rect::new(0.0, 21.0, 98.0, 6.0));
    });
}

#[cfg(feature = "soft")]
#[test]
fn canvas_is_double_buffered() {
    use drazui::soft::Canvas;
    let mut ui = Ui::with_bitmap_font();
    let mut canvas = Canvas::new(400, 300);
    let bg = Theme::dark().bg.0 >> 8;
    let draw = |ui: &mut Ui, canvas: &mut Canvas, text: &str| {
        frame(ui, |ui| ui.label(text));
        canvas.render(ui)
    };
    assert!(draw(&mut ui, &mut canvas, "one"));
    let first = canvas.buf.clone();
    assert!(first.iter().any(|p| *p != bg) && first[400 * 299] == bg);

    // an identical frame is not redrawn and the front buffer stays untouched
    assert!(!draw(&mut ui, &mut canvas, "one"));
    assert_eq!(canvas.buf, first);

    // a new frame is complete after the swap: nothing of the old one remains
    assert!(draw(&mut ui, &mut canvas, ""));
    assert!(canvas.buf.iter().all(|p| *p == bg));
    // ... also when going back, i.e. when drawing over the stale back buffer
    assert!(draw(&mut ui, &mut canvas, "one"));
    assert_eq!(canvas.buf, first);

    canvas.resize(200, 100);
    assert!(draw(&mut ui, &mut canvas, "one"));
    assert_eq!(canvas.buf.len(), 200 * 100);
    canvas.invalidate();
    assert!(draw(&mut ui, &mut canvas, "one"));
}

#[test]
fn list_selects_and_virtualizes() {
    let mut ui = Ui::with_bitmap_font();
    let mut sel = None;
    let mut built = 0;
    click(&mut ui, vec2(20.0, 1.0 + 14.0 * 3.0 + 4.0), |ui| {
        ui.list("l", 100_000, &mut sel, |ui, i| {
            built += 1;
            ui.label(&format!("{i}"));
        });
    });
    assert_eq!(sel, Some(3));
    assert!(built < 200, "only visible rows are built, got {built}");
}
