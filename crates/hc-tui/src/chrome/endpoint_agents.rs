// Derived from herdr src/client/shell/endpoint_agents.rs (Apache-2.0).

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use super::agent_folds::AgentFolds;
use super::endpoints::aggregate_agent_rows;
use super::render::{ShellHitMap, put_text};
use super::snapshot::{ClientEndpointId, ClientShellEndpoint};
use super::status::{status_color, status_icon};

pub(crate) fn render_collapsed(
    buffer: &mut Buffer,
    area: Rect,
    endpoints: &[ClientShellEndpoint],
    active_endpoint_id: &ClientEndpointId,
    config: &super::ClientShellConfig,
    hits: &mut ShellHitMap,
) {
    let rows = agent_rows(endpoints, active_endpoint_id, config, false);
    for (index, row) in rows.into_iter().take(area.height as usize).enumerate() {
        let rect = Rect::new(area.x, area.y + index as u16, area.width, 1);
        if row.agent.focused {
            buffer.set_style(rect, Style::default().bg(config.palette.active_row_bg));
        }
        let initial = row.machine_label.chars().next().unwrap_or('?');
        put_text(
            buffer,
            rect.x,
            rect.y,
            rect.width,
            &format!(
                "{initial}{}",
                status_icon(row.agent.status, config.status_indicators)
            ),
            Style::default()
                .fg(if row.stale {
                    config.palette.overlay0
                } else {
                    status_color(row.agent.status, &config.palette)
                })
                .add_modifier(if row.stale {
                    Modifier::DIM
                } else {
                    Modifier::empty()
                }),
        );
        hits.endpoint_agents
            .push((rect, row.endpoint_id, row.agent.pane_id));
    }
}

// Keep the pinned source renderer signature for later ports.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_expanded(
    buffer: &mut Buffer,
    area: Rect,
    agent_view_label: Option<&str>,
    endpoints: &[ClientShellEndpoint],
    active_endpoint_id: &ClientEndpointId,
    config: &super::ClientShellConfig,
    folds: AgentFolds<'_>,
    agent_scroll: &mut usize,
    reveal_focused_agent: &mut bool,
    hits: &mut ShellHitMap,
) {
    if !super::agents::render_agent_panel_header(buffer, area, agent_view_label, config, hits) {
        return;
    }
    let entries = panel_entries(
        endpoints,
        active_endpoint_id,
        agent_view_label,
        config,
        folds,
    );
    let reveal = std::mem::take(reveal_focused_agent);
    let target = reveal
        .then(|| {
            entries
                .iter()
                .position(|entry| matches!(entry, PanelEntry::Agent(row) if row.agent.focused))
        })
        .flatten();
    super::agents::render_agent_list(
        buffer,
        area,
        &entries,
        agent_view_label.map(|_| " no matching agents"),
        config,
        agent_scroll,
        hits,
        target,
        PanelEntry::lines,
        |buffer, rect, entry, hits| match entry {
            PanelEntry::Space {
                label,
                endpoint_id,
                workspace_id,
                gap,
                collapsed,
            } => {
                if let Some(line) = super::agents::render_space_header(
                    buffer,
                    rect,
                    (label, *collapsed),
                    *gap,
                    config,
                ) {
                    hits.extra.endpoint_agent_space_headers.push((
                        line,
                        endpoint_id.clone(),
                        workspace_id.clone(),
                    ));
                }
            }
            PanelEntry::Agent(row) => {
                super::agents::render_agent_row(buffer, rect, &row.agent, config);
                if row.stale {
                    buffer.set_style(
                        rect,
                        Style::default()
                            .fg(config.palette.overlay0)
                            .add_modifier(Modifier::DIM),
                    );
                }
                hits.endpoint_agents.push((
                    rect,
                    row.endpoint_id.clone(),
                    row.agent.pane_id.clone(),
                ));
            }
        },
    );
}

/// The position `agent_scroll` takes to show one machine's agent: its
/// index among the panel's rendered entries, space headers included.
/// `None` when the panel does not render that agent.
pub(crate) fn agent_entry_index(
    endpoints: &[ClientShellEndpoint],
    active_endpoint_id: &ClientEndpointId,
    agent_view_label: Option<&str>,
    config: &super::ClientShellConfig,
    folds: AgentFolds<'_>,
    endpoint_id: &ClientEndpointId,
    pane_id: &str,
) -> Option<usize> {
    panel_entries(
        endpoints,
        active_endpoint_id,
        agent_view_label,
        config,
        folds,
    )
    .iter()
    .position(|entry| {
        matches!(entry, PanelEntry::Agent(row)
            if &row.endpoint_id == endpoint_id && row.agent.pane_id == pane_id)
    })
}

/// One entry of the multi-machine agents panel: an agent, or (grouped by
/// space) the name of the space whose agents follow.
enum PanelEntry {
    /// A space header; `gap` puts a blank line above every group but the first.
    Space {
        label: String,
        endpoint_id: ClientEndpointId,
        workspace_id: String,
        gap: bool,
        /// The space's agents are folded away.
        collapsed: bool,
    },
    Agent(EndpointAgentRow),
}

impl PanelEntry {
    fn lines(&self) -> usize {
        match self {
            PanelEntry::Space { gap, .. } => 1 + usize::from(*gap),
            PanelEntry::Agent(row) => row.agent.rows.len(),
        }
    }
}

/// The panel's entries across machines. With `agent_panel_space_headers`
/// on, sorted by space and not filtered to a view, a header with the
/// space's name opens each run of one machine's space, as the Herdr app
/// heads its grouped list: the machine is not named, and a space of the
/// same name on another machine opens a group of its own. A space in
/// `folds` keeps only its header.
fn panel_entries(
    endpoints: &[ClientShellEndpoint],
    active_endpoint_id: &ClientEndpointId,
    agent_view_label: Option<&str>,
    config: &super::ClientShellConfig,
    folds: AgentFolds<'_>,
) -> Vec<PanelEntry> {
    let grouped = super::agent_folds::agent_panel_grouped(config, agent_view_label);
    let mut entries = Vec::new();
    let mut current: Option<(ClientEndpointId, String)> = None;
    let nest = grouped && config.nest_worker_terminals;
    for row in agent_rows(endpoints, active_endpoint_id, config, nest) {
        let mut collapsed = false;
        if grouped && let Some(label) = row.space_label.clone() {
            let group = (row.endpoint_id.clone(), row.workspace_id.clone());
            collapsed = folds.is_folded(&row.endpoint_id, &row.workspace_id);
            if current.as_ref() != Some(&group) {
                entries.push(PanelEntry::Space {
                    label,
                    endpoint_id: row.endpoint_id.clone(),
                    workspace_id: row.workspace_id.clone(),
                    gap: current.is_some(),
                    collapsed,
                });
                current = Some(group);
            }
        }
        if !collapsed {
            entries.push(PanelEntry::Agent(row));
        }
    }
    entries
}

struct EndpointAgentRow {
    endpoint_id: ClientEndpointId,
    machine_label: String,
    stale: bool,
    workspace_id: String,
    /// The label of the agent's space on its machine, when the machine's
    /// snapshot lists that space.
    space_label: Option<String>,
    agent: super::agents::AgentRow,
}

/// Every agent row across machines in panel order. With `nest`, a worker's
/// row is nested under its parent's, per machine and space; the depths are
/// settled before the rows are built because a nested row's label differs.
fn agent_rows(
    endpoints: &[ClientShellEndpoint],
    active_endpoint_id: &ClientEndpointId,
    config: &super::ClientShellConfig,
    nest: bool,
) -> Vec<EndpointAgentRow> {
    let aggregated = aggregate_agent_rows(endpoints, active_endpoint_id, config.agent_panel_sort);
    let depths = if nest {
        let keyed = aggregated
            .iter()
            .map(|row| {
                (
                    (&row.endpoint.endpoint_id, row.agent.workspace_id.as_str()),
                    row.agent.pane_id.as_str(),
                    row.agent.parent.as_deref(),
                )
            })
            .collect::<Vec<_>>();
        super::agent_nesting::depths_by_group(&keyed)
    } else {
        vec![0; aggregated.len()]
    };
    aggregated
        .into_iter()
        .zip(depths)
        .filter_map(|(row, depth)| {
            let mut agent = super::agents::agent_row(
                row.endpoint.snapshot,
                &row.agent.pane_id,
                config,
                Some(row.endpoint.label),
                depth > 0,
            )?;
            agent.depth = depth;
            agent.focused &= row.endpoint.endpoint_id == active_endpoint_id;
            Some(EndpointAgentRow {
                endpoint_id: row.endpoint.endpoint_id.clone(),
                machine_label: row.endpoint.label.to_owned(),
                stale: row.endpoint.stale(),
                workspace_id: row.agent.workspace_id.clone(),
                space_label: row
                    .endpoint
                    .snapshot
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.workspace_id == row.agent.workspace_id)
                    .map(|workspace| workspace.label.clone()),
                agent,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "endpoint_agents_tests.rs"]
mod tests;
