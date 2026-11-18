//! Gallery of all components.
//!
//! cargo run --example demo
//! cargo run --example demo -- --font C:/Windows/Fonts/segoeui.ttf [size]
//! cargo run --example demo -- --bitmap

mod common;

use drazui::*;

const SCALES: [f32; 4] = [1.0, 1.5, 2.0, 3.0];
const TAGS: [&str; 5] = ["rust", "ui", "tiny", "square", "fast"];

struct Node(&'static str, &'static [Node]);

const TREE: &[Node] = &[
    Node(
        "src",
        &[
            Node("geom.rs", &[]),
            Node("layout.rs", &[]),
            Node("menu.rs", &[]),
            Node("theme.rs", &[]),
            Node("ui.rs", &[]),
            Node("widgets.rs", &[]),
            Node("backend", &[Node("soft.rs", &[]), Node("font.rs", &[])]),
        ],
    ),
    Node("examples", &[Node("demo.rs", &[]), Node("tasks.rs", &[])]),
    Node("Cargo.toml", &[]),
    Node("README.md", &[]),
];

struct File {
    name: String,
    size: u32,
    ratio: f64,
    done: bool,
    color: Color,
}

struct App {
    theme: usize,
    scale: usize,
    name: String,
    count: i32,
    zoom: f64,
    unit: usize,
    snap: bool,
    grid: bool,
    mode: u8,
    clicks: u32,
    file: &'static str,
    item: Option<usize>,
    row: Option<usize>,
    files: Vec<File>,
    filters: [bool; 5],
    tags: Vec<&'static str>,
    notes: String,
    first: bool,
}

impl App {
    fn new() -> App {
        App {
            theme: 0,
            scale: 1,
            name: "drazui".into(),
            count: 3,
            zoom: 1.0,
            unit: 0,
            snap: true,
            grid: false,
            mode: 0,
            clicks: 0,
            file: "ui.rs",
            item: Some(2),
            row: Some(1),
            files: (0..1000)
                .map(|i: u32| File {
                    name: format!("file_{i:04}.rs"),
                    size: i * 37 % 900 + 3,
                    ratio: (i * 13 % 100) as f64 / 100.0,
                    done: i % 3 == 0,
                    color: [Color::hex(0x3ddc5a), Color::hex(0xff8a00), Color::hex(0x2a2d30), Color::hex(0x4c9aff)][i as usize % 4],
                })
                .collect(),
            filters: [true, false, true, false, false],
            tags: TAGS.to_vec(),
            notes: "Notes\n\nA multi-line text area: click to place the caret,\nthen type. Arrows, Home / End and Page Up / Down move,\nEnter starts a new line, Escape leaves the editor.\n".into(),
            first: true,
        }
    }

    fn theme(&self, font_size: f32) -> Theme {
        let mut t = if self.theme == 0 { Theme::dark() } else { Theme::light() };
        t.scale = SCALES[self.scale];
        t.font_size = font_size;
        t
    }
}

fn tree(ui: &mut Ui, nodes: &'static [Node], sel: &mut &'static str) {
    for n in nodes {
        let clicked = if n.1.is_empty() {
            ui.tree_leaf(n.0, *sel == n.0)
        } else {
            ui.tree_node(n.0, *sel == n.0, |ui| tree(ui, n.1, sel))
        };
        if clicked {
            *sel = n.0;
        }
    }
}

fn build(ui: &mut Ui, app: &mut App) {
    let bar = ui.row_h() + ui.theme.margin * 2.0;

    ui.menubar(|ui| {
        ui.menu("File", |ui| {
            if ui.menu_item("New") {
                ui.toast("New file");
            }
            if ui.menu_item("Open...") {
                ui.toast("Open clicked");
            }
            ui.separator();
            if ui.menu_item("Quit") {
                ui.toast_colored("Quit is not wired up", ui.theme.select);
            }
        });
        ui.menu("View", |ui| {
            ui.menu_check("Snap to grid", &mut app.snap);
            ui.menu_check("Show grid", &mut app.grid);
            ui.separator();
            if ui.menu_item("Dark theme") {
                app.theme = 0;
            }
            if ui.menu_item("Light theme") {
                app.theme = 1;
            }
        });
        ui.menu("Help", |ui| {
            if ui.menu_item("About") {
                ui.toast("drazui: lean, square, tiny");
            }
        });
    });

    ui.toolbar(|ui| {
        if ui.tool("New") {
            ui.toast("New file");
        }
        if ui.tool("Save") {
            ui.toast(&format!("Saved \"{}\"", app.name));
        }
        ui.separator();
        ui.tool_toggle("Snap", &mut app.snap);
        ui.tool_toggle("Grid", &mut app.grid);
        ui.separator();
        ui.label("Theme");
        ui.select("theme", &mut app.theme, &["Dark", "Light"]);
        ui.label("Scale");
        ui.select("scale", &mut app.scale, &["1x", "1.5x", "2x", "3x"]);
        if ui.button("Click me") {
            app.clicks += 1;
        }
    });

    ui.dock(Side::Bottom, bar, |ui| {
        let row = app.row.map_or("-".into(), |r| app.files[r].name.clone());
        ui.label_dim(&format!(
            "clicks {} | name \"{}\" | mode {} | tree {} | table {} | list {:?}",
            app.clicks, app.name, app.mode, app.file, row, app.item
        ));
    });

    ui.dock(Side::Left, 110.0, |ui| {
        ui.scroll("tree", |ui| {
            if app.first {
                ui.set_open("src", true);
            }
            tree(ui, TREE, &mut app.file);
        });
    });
    app.first = false;

    ui.split(
        "main",
        false,
        190.0,
        |ui| {
            ui.pad(|ui| {
                ui.scroll("form", |ui| {
                    ui.accordion("Inputs", true, |ui| {
                        ui.grid(&[Size::Px(40.0), Size::Fill], |ui| {
                            ui.label("Name");
                            ui.text_edit("name", &mut app.name);
                            ui.label("Count");
                            ui.number("count", &mut app.count, 1);
                            ui.label("Zoom");
                            ui.number("zoom", &mut app.zoom, 0.25);
                            ui.label("Unit");
                            ui.select("unit", &mut app.unit, &["Pixel", "Millimeter", "Inch"]);
                        });
                        ui.checkbox("Snap to grid", &mut app.snap);
                        ui.checkbox("Show grid", &mut app.grid);
                        ui.row(|ui| {
                            ui.radio("Draw", &mut app.mode, 0);
                            ui.radio("Edit", &mut app.mode, 1);
                            ui.radio("View", &mut app.mode, 2);
                        });
                    });
                    ui.accordion("Chips", true, |ui| {
                        ui.label_dim("toggle");
                        ui.flow(|ui| {
                            for (tag, on) in TAGS.iter().zip(&mut app.filters) {
                                ui.chip_toggle(tag, on);
                            }
                        });
                        ui.label_dim("removable");
                        ui.flow(|ui| {
                            app.tags.retain(|tag| !ui.chip_close(tag));
                            if ui.chip("+ reset") {
                                app.tags = TAGS.to_vec();
                            }
                        });
                    });
                    ui.accordion("Layouts", false, |ui| {
                        ui.label_dim("row");
                        ui.row(|ui| {
                            ui.button("A");
                            ui.button("B");
                            ui.w(Size::Fill).button("Fill");
                        });
                        ui.label_dim("grid");
                        ui.grid(&[Size::Fill; 3], |ui| {
                            for i in 1..=6 {
                                ui.button(&format!("Cell {i}"));
                            }
                        });
                        ui.label_dim("flow");
                        ui.flow(|ui| {
                            for s in ["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta"] {
                                ui.button(s);
                            }
                        });
                        ui.label_dim("frame");
                        ui.frame(|ui| {
                            ui.label("Framed content");
                            ui.w(Size::Px(60.0)).button("Sized");
                        });
                    });
                });
            });
        },
        |ui| {
            ui.split(
                "right",
                true,
                130.0,
                |ui| {
                    let files = &mut app.files;
                    // every cell is editable in place after a double-click
                    let cols = [("Name", 90.0), ("Size kB", 50.0), ("Ratio", 40.0), ("Done", 34.0), ("Color", 70.0)];
                    ui.table("files", &cols, files.len(), &mut app.row, |ui, r, c| {
                        let f = &mut files[r];
                        match c {
                            0 => ui.cell_text(&mut f.name),
                            1 => ui.cell_num(&mut f.size),
                            2 => ui.cell_num(&mut f.ratio),
                            3 => ui.cell_bool(&mut f.done),
                            _ => ui.cell_color(&mut f.color),
                        };
                    });
                    // right-click a row: it gets selected and this menu opens
                    ui.context_menu("file menu", |ui| {
                        let Some(r) = app.row else { return };
                        if ui.menu_item("Toggle done") {
                            files[r].done = !files[r].done;
                        }
                        if ui.menu_item("Remove") {
                            let gone = files.remove(r);
                            app.row = None;
                            ui.toast_colored(&format!("Removed {}", gone.name), ui.theme.select);
                        }
                    });
                },
                |ui| {
                    ui.split(
                        "bottom",
                        false,
                        120.0,
                        |ui| {
                            ui.list("items", 200, &mut app.item, |ui, i| ui.label(&format!("List item {i}")));
                        },
                        |ui| {
                            ui.text_area("notes", &mut app.notes);
                        },
                    );
                },
            );
        },
    );

    // theme and scale changes apply from the next frame on
    ui.theme = app.theme(ui.theme.font_size);
}

fn main() {
    let mut app = App::new();
    let mut ui = Ui::new();
    ui.theme = app.theme(ui.theme.font_size);
    common::run("drazui demo", ui, move |ui| build(ui, &mut app));
}
