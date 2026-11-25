//! Hit areas the shell's own render pass does not produce: the app draws
//! them over it. Kept apart from `ShellHitMap`'s flat list so that struct,
//! and `render.rs`, stay small.
use ratatui::layout::Rect;

/// The `↗ keep` and `×` buttons in a worker popup's title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PopupButtons {
    pub keep: Rect,
    pub close: Rect,
}

/// Hit areas drawn by the app over the shell, rebuilt every frame.
#[derive(Debug, Default, Clone)]
pub(crate) struct ExtraHits {
    /// Set only while a worker popup is drawn.
    pub popup_buttons: Option<PopupButtons>,
}
