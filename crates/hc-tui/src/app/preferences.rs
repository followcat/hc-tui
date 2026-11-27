// Derived from herdr src/client/shell/config.rs (Apache-2.0).
//! Rewriting the endpoint's chrome-preferences file from live state.
//! Every persisted field is collected — the file always holds the
//! complete current choices, not just the one that changed — and
//! `Option` fields serialize only the choices the user made by hand.
//! A failed write is a notice, never a panic.
use super::*;
use crate::chrome::snapshot::ClientEndpointId;
use crate::preferences::{ClientChromePreferences, ClientRemoteCollapsedGroups};

/// A set's members in file order.
fn sorted(set: &std::collections::HashSet<String>) -> Vec<String> {
    let mut members = set.iter().cloned().collect::<Vec<_>>();
    members.sort();
    members
}

/// The non-empty per-machine sets as the file's entries, ordered by machine.
fn remote_saved(
    sets: &std::collections::HashMap<ClientEndpointId, std::collections::HashSet<String>>,
) -> Vec<ClientRemoteCollapsedGroups> {
    let mut saved = sets
        .iter()
        .filter_map(|(endpoint_id, members)| {
            let ClientEndpointId::Remote(profile_id) = endpoint_id else {
                return None;
            };
            let collapsed_groups = sorted(members);
            (!collapsed_groups.is_empty()).then(|| ClientRemoteCollapsedGroups {
                profile_id: profile_id.to_string(),
                collapsed_groups,
            })
        })
        .collect::<Vec<_>>();
    saved.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
    saved
}

impl App {
    pub(super) fn persist_chrome_preferences(&mut self) {
        let Some(path) = self.shell.preferences_path.clone() else {
            return;
        };
        let state = &self.shell_state;
        let collapsed_groups = sorted(&state.collapsed_groups);
        let remote_collapsed_groups = remote_saved(&state.remote_collapsed_groups);
        let collapsed_agent_spaces = sorted(&state.collapsed_agent_spaces);
        let remote_collapsed_agent_spaces = remote_saved(&state.remote_collapsed_agent_spaces);
        let preferences = ClientChromePreferences {
            sidebar_width: self.sidebar_width_manual.then_some(self.sidebar_width),
            sidebar_section_split: self
                .sidebar_section_split_manual
                .then_some(self.shell_state.sidebar_section_split),
            sidebar_collapsed: self
                .sidebar_collapsed_manual
                .then_some(self.shell_state.sidebar_collapsed),
            agent_panel_sort: self
                .agent_panel_sort_manual
                .then_some(self.shell.agent_panel_sort),
            collapsed_groups,
            remote_collapsed_groups,
            collapsed_agent_spaces,
            remote_collapsed_agent_spaces,
        };
        if let Err(error) = crate::preferences::store(&path, &preferences) {
            self.show_notice("Client shell state", &error);
        }
    }
}
