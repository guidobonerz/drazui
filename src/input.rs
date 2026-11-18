use crate::geom::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Backspace,
    Delete,
    Enter,
    Escape,
    /// Select the whole text of the focused field (Ctrl + A).
    SelectAll,
    /// Copy the selection to [`Ui::clipboard`](crate::Ui::clipboard) (Ctrl + C).
    Copy,
    /// Like `Copy`, and delete the selection (Ctrl + X).
    Cut,
    /// Insert [`Ui::clipboard`](crate::Ui::clipboard) (Ctrl + V).
    Paste,
}

/// Fill this in before every [`Ui::begin`](crate::Ui::begin).
/// `text`, `keys` and `scroll` are per-frame events and are cleared by `end`.
#[derive(Default)]
pub struct Input {
    /// Mouse position in device pixels.
    pub mouse: Vec2,
    /// Left mouse button state.
    pub down: bool,
    /// Right mouse button state (opens context menus).
    pub right: bool,
    /// Shift key state: arrows, Home / End and clicks extend the selection.
    pub shift: bool,
    /// Seconds since the previous frame (drives toasts). 0 is treated as 1/60.
    pub dt: f32,
    /// Wheel movement in lines (positive y = scroll up).
    pub scroll: Vec2,
    /// Characters typed this frame.
    pub text: String,
    /// Keys pressed this frame (including repeats).
    pub keys: Vec<Key>,
}
