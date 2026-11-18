# drazui

A lean, square, tiny immediate-mode UI for Rust.

- **No dependencies in the core.** A frame is a list of three primitives
  (`Rect`, `Text`, `Clip`), so a backend is a few lines.
- **Optional `soft` feature** (on by default): software rasterizer.
- **Optional `ttf` feature** (on by default): vector fonts, built-in Univers Condensed.
- **Optional `gpu` feature**: hardware-accelerated OpenGL renderer.
- **Two-way binding**: widgets take `&mut` values, show them and write changes back.
- **Resizable**: `theme.scale` zooms the whole UI; splits and table columns drag.

```
cargo run --example demo      # component gallery
cargo run --example tasks     # small task manager
```

## Usage

```rust
use drazui::*;

let mut ui = Ui::new();
let (mut name, mut on, mut unit) = (String::new(), false, 0usize);

// every frame
ui.input.mouse = vec2(mx, my);   // device pixels
ui.input.down = left_button;
ui.begin(width, height);

ui.dock(Side::Left, 120.0, |ui| {
    ui.label("Settings");
    ui.text_edit("name", &mut name);
    ui.checkbox("Enabled", &mut on);
    ui.select("unit", &mut unit, &["px", "mm"]);
    if ui.button("Apply") { /* ... */ }
});

ui.end();
for d in ui.draws() { /* Draw::Rect / Draw::Text / Draw::Clip */ }
```

With the `soft` feature, `soft::Canvas::render(&ui)` draws into a back buffer and
swaps it into `canvas.buf` (`0x00RRGGBB`) when the frame is complete. It returns
`false` and skips drawing when nothing changed since the last frame. `buf` is a buffer
you can hand to any window (see `examples/demo.rs` for minifb).

## Widgets

`label` `button` `checkbox` `radio` `select` `text_edit` `text_area` `number` `selectable`
`accordion` `tree_node` / `tree_leaf` `list` `table` `separator` `space`

`list` and `table` are virtualized: the row closure only runs for visible rows.

### Text area

```rust
ui.text_area("notes", &mut notes);    // multi-line String, lines separated by '\n'
```

A plain multi-line editor that fills the remaining space (size it with
`ui.w` / `ui.h`) and scrolls in both directions; lines are not wrapped. A
click places the caret; arrows, Home / End, Page Up / Page Down move it,
Enter starts a new line and Escape leaves the editor. Feed the keys through
`ui.input.keys` (`Key::Up`, `Key::Down`, `Key::PageUp`, `Key::PageDown` in
addition to the ones `text_edit` uses). There is no undo.

### Selection and clipboard

All text fields (`text_edit`, `text_area`, `number`, table cells while
editing) support selecting, cut, copy and paste:

- drag, Shift + click, Shift + arrows / Home / End / Page keys select;
  a double-click selects a word
- typing, Backspace, Delete and paste replace the selection

The core has no dependencies, so the backend supplies the modifier state, the
shortcuts and the system clipboard:

```rust
ui.input.shift = shift_is_down;
// Ctrl+A / C / X: push Key::SelectAll / Key::Copy / Key::Cut
// Ctrl+V:
ui.clipboard = system_clipboard_text();
ui.input.keys.push(Key::Paste);

// after ui.end()
if let Some(text) = ui.take_copied() { set_system_clipboard(text); }
```

Without that wiring `ui.clipboard` still works as a clipboard inside the
application. `examples/common/mod.rs` shows it for minifb (Windows clipboard
through the Win32 API) and miniquad.

### Number field

```rust
ui.number("count", &mut count, 1);    // [-][ 3 ][+], any integer type
ui.number("zoom", &mut zoom, 0.25);   // f32 / f64
```

The button on the left decrements by the step, the one on the right
increments; holding a button repeats. The number can also be typed (Enter or
a click elsewhere commits, Escape cancels, invalid input is discarded).
Integers saturate at their limits. There is no built-in range: clamp the
value after the call if you need one.

### Editable table cells

Cells show their value and switch to an in-place editor on double-click.
Enter or a click elsewhere commits, Escape cancels. Each cell binds two-way to
the `&mut` it gets and returns `true` when the value changed.

```rust
ui.table("items", &cols, items.len(), &mut selected, |ui, row, col| {
    let it = &mut items[row];
    match col {
        0 => ui.cell_text(&mut it.name),      // String
        1 => ui.cell_num(&mut it.count),      // any integer type
        2 => ui.cell_num(&mut it.price),      // f32 / f64, decimal comma accepted
        3 => ui.cell_bool(&mut it.active),    // toggles on double-click
        _ => ui.cell_color(&mut it.color),    // swatch + #RRGGBB / #RRGGBBAA
    };
});
```

Input that does not parse (letters in a number, a broken hex code) is
discarded and the old value stays. Double-clicks need `ui.input.dt`.

### Toolbar, menus, toasts, chips

```rust
ui.menubar(|ui| {
    ui.menu("File", |ui| {
        if ui.menu_item("Open...") { /* ... */ }
        ui.separator();
        ui.menu_check("Autosave", &mut autosave);   // bound, keeps the menu open
    });
});
ui.toolbar(|ui| {
    if ui.tool("Save") { ui.toast("Saved"); }       // flat button + toast
    ui.tool_toggle("Grid", &mut show_grid);          // bound toggle
});

ui.table(/* ... */);
ui.context_menu("row menu", |ui| {                   // right-click on the widget before
    if ui.menu_item("Remove") { /* ... */ }
});

ui.flow(|ui| {
    ui.chip_toggle("rust", &mut filter_rust);        // filter chip
    tags.retain(|t| !ui.chip_close(t));              // removable chips
    if ui.chip("+ add") { /* ... */ }
});
```

- `menubar` and `toolbar` dock to the top of the current layout, so call them
  before other content. `menu` also works outside a menubar as a drop-down button.
- `context_menu` needs `ui.input.right` (right mouse button); a right-click on a
  table or list row selects that row first. `open_popup` + `popup_menu` open a
  popup menu from code.
- `toast` / `toast_colored` show a message bottom-right for `ui.toast_secs`
  seconds; feed `ui.input.dt` (frame time) so they expire in real time.
- Menus are one level deep (no submenus).

### Dialogs

```rust
if ui.button("Quit") {
    ui.open_dialog("quit");                          // from anywhere in the same id scope
}
// declared once per frame, outside of other containers
match ui.confirm("quit", "Quit", "There are unsaved changes.\nQuit anyway?", &["Quit", "Cancel"]) {
    Some(0) => { /* quit */ }
    _ => {}                                          // None: no answer (yet)
}

ui.dialog("about", "About", |ui| {                   // any content
    ui.label("drazui");
    if ui.button("Close") { ui.close_dialog(); }
});
```

- A dialog is modal: it is centered above everything else, the UI behind it is
  shaded and does not react to the mouse. Only one dialog is open at a time.
- `confirm` returns the index of the chosen button once and closes the dialog.
  Enter chooses the first button, Escape the last one, so put the cancelling
  button last.
- `dialog` is sized to its content and closed by Escape or `close_dialog`;
  `dialog_open` tells whether a dialog is open.
- Menus and `select`s do not work inside a dialog.

## Layout

| Call | Effect |
| --- | --- |
| `col` / `row` / `flow` | vertical, horizontal, wrapping stack |
| `grid(&[Size::Px(40.0), Size::Fill], ..)` | columns; each widget takes the next cell |
| `dock(Side::Top, size, ..)` | cut a panel off one side |
| `split(id, vertical, initial, a, b)` | two panes with a draggable handle |
| `scroll(id, ..)` | scroll area |
| `pad` / `indent` / `frame` | margin, indentation, bordered panel |
| `ui.w(Size::Fill).button("x")` | size the next widget: `Auto`, `Px`, `Fill`, `Frac` |

Widgets are identified by their label; in loops with repeated labels wrap them
in `ui.push_id(i)` / `ui.pop_id()`.

## Theming

Every field of `ui.theme` is public and can change at any time.

```rust
ui.theme = Theme::dark();            // anthracite, black, green, orange (default)
ui.theme = Theme::light();
ui.theme = Theme::palette(bg, field, accent, select); // derive a theme from 4 colors
ui.theme.scale = 2.0;                // zoom everything
ui.theme.accent = Color::hex(0x00c8ff);
```

## Fonts

The default font is the built-in **Univers Condensed** (feature `ttf`, on by
default), at `theme.font_size` 11. `Ui::new()` uses it automatically and both
renderers take the font from the `Ui`, so there is nothing to set up.

```rust
// another .ttf, .ttc, .otf or .woff file (WOFF2 is not supported)
let font = drazui::soft::Font::open("C:/Windows/Fonts/segoeui.ttf")?; // or Font::load(bytes)
ui.set_font(Some(font));
ui.theme.font_size = 12.0;

// the monospace 8x8 bitmap font
let mut ui = Ui::with_bitmap_font();          // or: ui.set_font(None) + font_size 8
```

```
cargo run --example demo -- --font C:/Windows/Fonts/segoeui.ttf 12
cargo run --example demo -- --bitmap
```

Without the `ttf` feature (`default-features = false, features = ["soft"]`) the
font file is not compiled in and the bitmap font is the default.

## GPU rendering

Enable the `gpu` feature for a hardware-accelerated renderer (OpenGL through
[miniquad](https://docs.rs/miniquad)). The whole frame is one vertex buffer and
one glyph atlas texture, drawn with one draw call per clip rectangle.

```rust
// setup, inside miniquad::start
let mut ctx = miniquad::window::new_rendering_backend();
let mut renderer = drazui::gpu::Renderer::new(&mut *ctx);

// every frame, after ui.end()
let (w, h) = miniquad::window::screen_size();
ctx.begin_default_pass(Default::default());
renderer.render(&mut *ctx, &ui, w, h);
ctx.end_render_pass();
ctx.commit_frame();
```

```
cargo run --example demo --features gpu
cargo run --example demo --features gpu -- --soft     # software renderer for comparison
```

`examples/common/mod.rs` shows the complete miniquad event loop. For another GPU
API (wgpu, Vulkan, ...), `gpu::Mesh::build` gives you the vertices, indices,
clip batches and atlas pixels without touching OpenGL.

## Other backends

Render `ui.draws()` yourself and set `ui.measure` to your font's text width
function (`(text, pixel_size) -> width`).

## Custom widgets

```rust
let id = ui.id("my widget");
let rect = ui.alloc(vec2(40.0, ui.row_h()));
let resp = ui.interact(id, rect);
ui.fill(rect, if resp.hovered { ui.theme.hover } else { ui.theme.widget });
```
