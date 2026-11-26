//! Hit areas kept apart from `ShellHitMap`'s flat list so that struct, and
//! `render.rs`, stay small.
use ratatui::layout::Rect;

use super::snapshot::ClientEndpointId;

/// The agents panel's space header hits, rebuilt each frame.
#[derive(Debug, Clone, Default)]
pub(crate) struct ExtraHits {
    /// Each agent space header's label line (never the blank gap above it)
    /// in the single-machine panel, with its workspace id.
    pub agent_space_headers: Vec<(Rect, String)>,
    /// The same in the multi-machine panel, with the machine.
    pub endpoint_agent_space_headers: Vec<(Rect, ClientEndpointId, String)>,
}
