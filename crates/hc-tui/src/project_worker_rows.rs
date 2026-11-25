//! Agent rows: one member's row, and the rows of the hidden workers that
//! nest under it. A hidden worker has no tab and no pane, so its row carries
//! its parent's workspace and tab ids and is the only place it shows.
use super::*;
use crate::structure::Space;

/// The agent row for `term` in `workspace_id`/`tab_id`. `record` is the
/// agent record selected for `term`, if any; `worker_popup` marks a hidden
/// worker's row.
pub(super) fn agent_row(
    term: &Terminal,
    workspace_id: &str,
    tab_id: &str,
    record: Option<AgentRecord<'_>>,
    focused: bool,
    worker_popup: bool,
) -> ClientShellAgent {
    let status = record.map_or(AgentStatus::Unknown, |record| record.state);
    let record = record.and_then(|selected| selected.status(term));
    ClientShellAgent {
        pane_id: term.address.clone(),
        workspace_id: workspace_id.to_owned(),
        tab_id: tab_id.to_owned(),
        name: term.name.as_deref().map(clean),
        display_agent: record.map(|s| clean(&s.app)).filter(|a| !a.is_empty()),
        agent: record.map(|s| clean(&s.app)).filter(|a| !a.is_empty()),
        // `title` is the reported status title;
        // `terminal_title` is the program's OSC title —
        // distinct slots, never swapped.
        title: record.map(|s| clean(&s.title)).filter(|t| !t.is_empty()),
        terminal_title: if term.title.is_empty() {
            None
        } else {
            Some(clean(&term.title))
        },
        terminal_title_stripped: crate::chrome::title_text::stripped_terminal_title(
            &crate::sidebar::clean(&term.title),
        ),
        agent_status: status,
        // Presentation supplies a rank from origin clocks;
        // timestamps and turns are not chronology facts.
        state_change_seq: 0,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused,
        worker_popup,
    }
}

/// Push the rows of the hidden workers of `parent` — in creation order,
/// each followed by its own workers — onto `agents` and `agent_order`.
/// Every worker gets a row, a live reporter or not: with no tab, the row is
/// how it is reached. A worker with no status record is `Unknown`.
pub(super) fn worker_agents(
    space: &Space,
    parent: &str,
    (workspace_id, tab_id): (&str, &str),
    term_by_id: &BTreeMap<&str, &Terminal>,
    agents: &mut Vec<ClientShellAgent>,
    agent_order: &mut Vec<String>,
) {
    for hidden in space.workers.iter().filter(|w| w.parent == parent) {
        let Some(term) = term_by_id.get(hidden.address.as_str()).copied() else {
            continue;
        };
        agents.push(agent_row(
            term,
            workspace_id,
            tab_id,
            agent_record(term),
            false,
            true,
        ));
        agent_order.push(term.address.clone());
        worker_agents(
            space,
            &hidden.address,
            (workspace_id, tab_id),
            term_by_id,
            agents,
            agent_order,
        );
    }
}
