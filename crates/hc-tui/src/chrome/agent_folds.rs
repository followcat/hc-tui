//! Folded space groups of the grouped agents panel: a folded space keeps
//! its header and loses its agent rows.
use std::collections::{HashMap, HashSet};

use super::snapshot::ClientEndpointId;

/// The workspace ids folded on the local endpoint and on each remote one.
#[derive(Clone, Copy)]
pub(crate) struct AgentFolds<'a> {
    pub local: &'a HashSet<String>,
    pub remote: &'a HashMap<ClientEndpointId, HashSet<String>>,
}

impl AgentFolds<'_> {
    /// Whether `workspace_id` of `endpoint` is folded.
    pub(crate) fn is_folded(&self, endpoint: &ClientEndpointId, workspace_id: &str) -> bool {
        if endpoint.is_local() {
            self.local.contains(workspace_id)
        } else {
            self.remote
                .get(endpoint)
                .is_some_and(|folded| folded.contains(workspace_id))
        }
    }
}

/// Whether the agents panel heads its spaces: the option is on and the
/// panel is sorted by space. Only then do folds mean anything.
pub(crate) fn space_headers_shown(config: &super::ClientShellConfig) -> bool {
    config.agent_panel_space_headers
        && config.agent_panel_sort == crate::config::AgentPanelSortConfig::Spaces
}

/// Whether the panel draws space headers right now: headers shown and no
/// agent view filtering the list.
pub(crate) fn agent_panel_grouped(
    config: &super::ClientShellConfig,
    agent_view_label: Option<&str>,
) -> bool {
    space_headers_shown(config) && agent_view_label.is_none()
}
