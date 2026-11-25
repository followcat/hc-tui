//! Workers that have no tab (`ui.nest_worker_terminals`): their agent row
//! is the only way to reach them, and activating it opens the worker over
//! the current tab without moving focus.
use super::*;

impl App {
    /// Whether `pane_id` is the row of a worker with no tab and no pane,
    /// on any endpoint. Every place that activates an agent asks this
    /// first: such an id must never become `self.focused`, because no pane
    /// exists to focus.
    pub(super) fn worker_popup_agent(&self, pane_id: &str) -> bool {
        let on = |snapshot: &crate::chrome::snapshot::ClientShellSnapshot| {
            snapshot
                .agents
                .iter()
                .any(|agent| agent.worker_popup && agent.pane_id == pane_id)
        };
        on(&self.projection.snapshot)
            || self
                .projection
                .endpoints
                .iter()
                .filter_map(|endpoint| endpoint.snapshot.as_deref())
                .any(on)
    }

    /// A click on an agent row: open a worker's row in place, focus any
    /// other agent's pane.
    pub(super) fn activate_agent(&mut self, pane_id: &str) {
        if self.worker_popup_agent(pane_id) {
            self.open_worker(pane_id.to_owned());
        } else {
            self.focus_pane(pane_id.to_owned());
        }
    }

    /// Open the worker `pane_id` for an activated row. For now this is the
    /// watch-only peek.
    pub(super) fn open_worker(&mut self, pane_id: String) {
        self.open_peek(pane_id);
    }
}
