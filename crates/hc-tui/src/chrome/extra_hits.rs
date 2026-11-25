//! Hit areas kept apart from `ShellHitMap`'s flat list so that struct, and
//! `render.rs`, stay small: the agent space headers, which the shell's render
//! pass fills, and the worker popup's buttons, which the app draws over it.
use ratatui::layout::Rect;

use super::snapshot::ClientEndpointId;

/// The `↗ keep` and `×` buttons in a worker popup's title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PopupButtons {
    pub keep: Rect,
    pub close: Rect,
}

/// The extra hit areas, rebuilt every frame: the space headers come from the
/// shell render, `popup_buttons` from the app.
#[derive(Debug, Default, Clone)]
pub(crate) struct ExtraHits {
    /// Set only while a worker popup is drawn.
    pub popup_buttons: Option<PopupButtons>,
    /// Each agent space header's label line (never the blank gap above it)
    /// in the single-machine panel, with its workspace id.
    pub agent_space_headers: Vec<(Rect, String)>,
    /// The same in the multi-machine panel, with the machine.
    pub endpoint_agent_space_headers: Vec<(Rect, ClientEndpointId, String)>,
}
