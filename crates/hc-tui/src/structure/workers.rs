//! Worker placement: a terminal started from another terminal (a worker)
//! with no placement of its own gets no tab. It is recorded on its parent's
//! space, so the agents panel can nest its row under the parent's and open
//! it in a popup.
use super::{HiddenWorker, Space};
use hc_proto::terminal::Terminal;
use std::collections::BTreeMap;

/// Where each already-placed terminal lives: its space's index in the
/// `spaces` vector and the id of the tab that holds it (for a hidden worker,
/// its parent's tab).
pub(super) type Placed = BTreeMap<String, (usize, String)>;

/// The placements the app document made, before any fallback runs.
pub(super) fn placed_in_spaces(spaces: &[Space]) -> Placed {
    let mut placed = Placed::new();
    for (at, space) in spaces.iter().enumerate() {
        for tab in &space.tabs {
            for terminal in tab.root.terminals() {
                placed.insert(terminal, (at, tab.id.clone()));
            }
        }
    }
    placed
}

/// Apply the worker rule to one terminal of the fallback loop. Returns
/// `true` when the terminal was handled, so the caller skips its own
/// fallback placement.
///
/// - A finished worker with no `herdr.space` label is hidden outright.
/// - A live one whose parent is already placed is recorded on the parent's
///   space as a [`HiddenWorker`], and placed where its parent is so its own
///   workers chain.
/// - Anything else (no parent, a parent that is not placed) is not
///   handled and falls back like any other terminal.
pub(super) fn hide_worker(terminal: &Terminal, spaces: &mut [Space], placed: &mut Placed) -> bool {
    let Some(parent) = terminal.parent.as_deref() else {
        return false;
    };
    if terminal.labels.contains_key("herdr.space") {
        return false;
    }
    if terminal.state == "finished" {
        return true;
    }
    let Some((at, tab)) = placed.get(parent).cloned() else {
        return false;
    };
    spaces[at].workers.push(HiddenWorker {
        address: terminal.address.clone(),
        parent: parent.to_owned(),
    });
    placed.insert(terminal.address.clone(), (at, tab));
    true
}

#[cfg(test)]
#[path = "workers_tests.rs"]
mod tests;
