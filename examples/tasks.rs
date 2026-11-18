//! A small task manager: shows how app state is bound two-way to widgets.
//! The table and the detail panel edit the same `Task`, so a change in one
//! shows up in the other immediately.
//!
//! cargo run --example tasks

mod common;

use drazui::*;

const PRIOS: [&str; 3] = ["Low", "Normal", "High"];

struct Task {
    title: String,
    done: bool,
    prio: usize,
}

#[derive(PartialEq, Clone, Copy)]
enum Filter {
    All,
    Open,
    Done,
}

struct App {
    tasks: Vec<Task>,
    new_title: String,
    new_prio: usize,
    filter: Filter,
    /// Index into `tasks`.
    selected: Option<usize>,
}

impl App {
    fn new() -> App {
        let task = |title: &str, done, prio| Task { title: title.into(), done, prio };
        App {
            tasks: vec![
                task("Write the layout engine", true, 2),
                task("Add table and tree", true, 1),
                task("TrueType fonts", true, 1),
                task("Keyboard navigation", false, 1),
                task("Text selection + clipboard", false, 2),
                task("GPU backend", false, 0),
            ],
            new_title: String::new(),
            new_prio: 1,
            filter: Filter::All,
            selected: Some(3),
        }
    }
}

fn build(ui: &mut Ui, app: &mut App) {
    let t = ui.theme;
    let bar = t.row_h() + t.margin * 2.0;

    // ---- top: add a task ---------------------------------------------------
    ui.dock(Side::Top, bar, |ui| {
        let label_w = ui.text_width("New task") + t.pad.x;
        ui.grid(&[Size::Px(label_w), Size::Fill, Size::Px(64.0), Size::Px(36.0)], |ui| {
            ui.label("New task");
            ui.text_edit("new", &mut app.new_title);
            ui.select("new prio", &mut app.new_prio, &PRIOS);
            if ui.button("Add") && !app.new_title.trim().is_empty() {
                let title = std::mem::take(&mut app.new_title);
                app.tasks.push(Task { title, done: false, prio: app.new_prio });
                app.selected = Some(app.tasks.len() - 1);
                app.filter = Filter::All;
            }
        });
    });

    // ---- bottom: status ----------------------------------------------------
    ui.dock(Side::Bottom, bar, |ui| {
        let done = app.tasks.iter().filter(|t| t.done).count();
        ui.row(|ui| {
            if ui.button("Remove done") {
                // keep the selection on the same task
                let keep = app.selected.filter(|i| !app.tasks[*i].done);
                app.selected = keep.map(|i| app.tasks[..i].iter().filter(|t| !t.done).count());
                app.tasks.retain(|t| !t.done);
            }
            ui.label_dim(&format!("{} open, {} done", app.tasks.len() - done, done));
        });
    });

    // ---- right: details of the selected task -------------------------------
    let mut delete = None;
    ui.dock(Side::Right, 150.0, |ui| {
        let Some(i) = app.selected else {
            return ui.label_dim("No task selected");
        };
        let task = &mut app.tasks[i];
        ui.label_dim("Title");
        ui.text_edit("title", &mut task.title);
        ui.space(2.0);
        ui.checkbox("Done", &mut task.done);
        ui.space(2.0);
        ui.label_dim("Priority");
        for (p, name) in PRIOS.iter().enumerate() {
            ui.radio(name, &mut task.prio, p);
        }
        ui.space(2.0);
        if ui.button("Delete") {
            delete = Some(i);
        }
    });

    // ---- center: filter + table --------------------------------------------
    ui.pad(|ui| {
        ui.row(|ui| {
            ui.label_dim("Show");
            ui.radio("All", &mut app.filter, Filter::All);
            ui.radio("Open", &mut app.filter, Filter::Open);
            ui.radio("Done", &mut app.filter, Filter::Done);
        });

        // The table shows a filtered view, so map between rows and task indices.
        let filter = app.filter;
        let rows: Vec<usize> = (0..app.tasks.len())
            .filter(|i| match filter {
                Filter::All => true,
                Filter::Open => !app.tasks[*i].done,
                Filter::Done => app.tasks[*i].done,
            })
            .collect();
        let mut row = app.selected.and_then(|s| rows.iter().position(|i| *i == s));
        let selected_row = row;
        let tasks = &mut app.tasks;
        let check_w = t.font_size + 2.0 + t.pad.x * 2.0; // checkbox + cell padding
        let cols = [("", check_w), ("Task", 220.0), ("Priority", 50.0)];
        let changed = ui.table("tasks", &cols, rows.len(), &mut row, |ui, r, c| {
            let task = &mut tasks[rows[r]];
            match c {
                0 => {
                    ui.checkbox("", &mut task.done);
                }
                1 if task.done => ui.label_dim(&task.title),
                1 => ui.label(&task.title),
                _ if task.prio == 2 && selected_row != Some(r) => ui.label_colored(PRIOS[2], t.select),
                _ => ui.label(PRIOS[task.prio]),
            }
        });
        if changed {
            app.selected = row.map(|r| rows[r]);
        }
    });

    if let Some(i) = delete {
        app.tasks.remove(i);
        app.selected = None;
    }
}

fn main() {
    let mut ui = Ui::new();
    ui.theme.scale = 2.0;
    let mut app = App::new();
    common::run("drazui tasks", ui, move |ui| build(ui, &mut app));
}
