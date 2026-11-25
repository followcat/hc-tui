//! Folding space groups in the agents panel: a header click toggles its
//! space, folds are remembered per machine, and agent navigation onto an
//! agent in a folded space unfolds it first.
use super::*;
use crate::chrome::agent_folds::AgentFolds;
use crate::chrome::snapshot::{ClientEndpointId, ClientShellSnapshot};
use std::collections::HashSet;

impl App {
    /// The fold state the panel renders and counts rows with.
    pub(super) fn agent_folds(&self) -> AgentFolds<'_> {
        AgentFolds {
            local: &self.shell_state.collapsed_agent_spaces,
            remote: &self.shell_state.remote_collapsed_agent_spaces,
        }
    }

    fn agent_folds_mut(&mut self, endpoint_id: &ClientEndpointId) -> &mut HashSet<String> {
        if endpoint_id.is_local() {
            &mut self.shell_state.collapsed_agent_spaces
        } else {
            self.shell_state
                .remote_collapsed_agent_spaces
                .entry(endpoint_id.clone())
                .or_default()
        }
    }

    /// A header click: fold or unfold `workspace_id` of `endpoint_id`, and
    /// remember it.
    pub(in crate::app) fn toggle_collapsed_agent_space(
        &mut self,
        endpoint_id: &ClientEndpointId,
        workspace_id: String,
    ) {
        let folds = self.agent_folds_mut(endpoint_id);
        if !folds.remove(&workspace_id) {
            folds.insert(workspace_id);
        }
        self.persist_chrome_preferences();
    }

    /// The workspace of agent `pane_id` on `endpoint_id`.
    fn agent_workspace(&self, endpoint_id: &ClientEndpointId, pane_id: &str) -> Option<String> {
        let find = |snapshot: &ClientShellSnapshot| {
            snapshot
                .agents
                .iter()
                .find(|agent| agent.pane_id == pane_id)
                .map(|agent| agent.workspace_id.clone())
        };
        self.projection
            .endpoints
            .iter()
            .find(|endpoint| &endpoint.endpoint_id == endpoint_id)
            .and_then(|endpoint| endpoint.snapshot.as_deref())
            .and_then(find)
            .or_else(|| {
                (endpoint_id == &self.projection.active_endpoint_id)
                    .then(|| find(&self.projection.snapshot))
                    .flatten()
            })
    }

    /// Agent navigation onto `pane_id` reveals it: unfold its space, so the
    /// focused row is never invisible.
    pub(super) fn unfold_agent_space_of(&mut self, endpoint_id: &ClientEndpointId, pane_id: &str) {
        let Some(workspace_id) = self.agent_workspace(endpoint_id, pane_id) else {
            return;
        };
        if self.agent_folds_mut(endpoint_id).remove(&workspace_id) {
            self.persist_chrome_preferences();
        }
    }

    /// Scroll the single-machine agents list to `pane_id`'s row, counted in
    /// the rows as rendered now (folded spaces included); `fallback` stands
    /// in when the panel does not render it.
    pub(super) fn scroll_to_agent_row(&mut self, pane_id: &str, fallback: usize) {
        let row = crate::chrome::agents::panel_rows(
            &self.projection.snapshot,
            &self.shell,
            &self.shell_state.collapsed_agent_spaces,
        )
        .iter()
        .position(|row| matches!(row, crate::chrome::agents::PanelRow::Agent(agent) if agent.pane_id == pane_id))
        .unwrap_or(fallback);
        self.shell_state.agent_scroll = row.min(self.hit_map.agent_max_scroll);
    }
}
