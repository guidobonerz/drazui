//! drazui — a lean, square, tiny immediate-mode UI.
//!
//! The core has no dependencies and emits just three draw primitives
//! ([`Draw::Rect`], [`Draw::Text`], [`Draw::Clip`]), so a backend is a few lines.
//! The optional `soft` feature ships a software rasterizer with an 8x8 font.
//!
//! ```
//! use drazui::*;
//! let mut ui = Ui::new();
//! let (mut name, mut on) = (String::new(), false);
//! // every frame: feed `ui.input`, then
//! ui.begin(640.0, 480.0);
//! ui.label("Hello");
//! ui.text_edit("name", &mut name); // widgets bind two-way through `&mut`
//! ui.checkbox("Enabled", &mut on);
//! if ui.button("OK") { /* clicked */ }
//! ui.end();
//! for d in ui.draws() { /* render */ let _ = d; }
//! ```

#[cfg(feature = "ttf")]
mod font;
mod cells;
mod dialog;
mod editor;
mod geom;
mod input;
mod layout;
mod menu;
mod number;
mod theme;
mod ui;
mod widgets;

#[cfg(feature = "soft")]
pub mod soft;

#[cfg(feature = "gpu")]
pub mod gpu;

pub use geom::{vec2, Color, Rect, Vec2};
pub use input::{Input, Key};
pub use layout::{Side, Size};
pub use number::Num;
pub use theme::Theme;
pub use ui::{Align, Draw, Id, Resp, Ui};
