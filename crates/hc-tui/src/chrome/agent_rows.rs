// Derived from herdr src/client/shell/agent_sidebar.rs (Apache-2.0).

use std::collections::HashMap;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Paragraph, Widget},
};

use super::agents::ordered_agent_pane_ids;
use super::render::put_text;
use super::snapshot::ClientShellSnapshot;
use super::spans::resolved_token_spans;
use super::status::{AgentStatus, status_color, status_icon, status_text};
use super::tokens::{self, ResolvedToken, ResolvedTokenKind};

pub(crate) struct AgentRow {
    pub pane_id: String,
    pub status: AgentStatus,
    pub focused: bool,
    pub rows: Vec<Vec<ResolvedToken>>,
    /// Nesting level under the row of the terminal this agent was started
    /// from; 0 for an ordinary row.
    pub depth: usize,
}

/// One entry of the agents panel: an agent, or (grouped by space) the
/// name of the space whose agents follow, as the Herdr app's grouped
/// agents list heads each group.
pub(crate) enum PanelRow {
    /// A space header; `gap` puts a blank line above every group but the first.
    Space {
        label: String,
        workspace_id: String,
        gap: bool,
        /// The space's agents are folded away.
        collapsed: bool,
    },
    Agent(AgentRow),
}

impl PanelRow {
    pub(super) fn lines(&self) -> usize {
        match self {
            PanelRow::Space { gap, .. } => 1 + usize::from(*gap),
            PanelRow::Agent(row) => row.rows.len(),
        }
    }
}

/// The panel's entries. With `agent_panel_space_headers` on, sorted by
/// space and not filtered to a view, a header with the space's name opens
/// each run of that space's agents, and a space in `folded` keeps only
/// its header. Otherwise the list stays flat: the reference's space order
/// has no headers, and by priority one space's agents are spread.
pub(crate) fn panel_rows(
    snapshot: &ClientShellSnapshot,
    config: &super::ClientShellConfig,
    folded: &std::collections::HashSet<String>,
) -> Vec<PanelRow> {
    let grouped = config.agent_panel_space_headers
        && config.agent_panel_sort == crate::config::AgentPanelSortConfig::Spaces
        && snapshot.agent_view_label.is_none();
    let mut rows = Vec::new();
    let mut current: Option<&str> = None;
    let ordered = ordered_agent_pane_ids(snapshot, config.agent_panel_sort);
    let depths = if grouped && config.nest_worker_terminals {
        grouped_depths(snapshot, &ordered)
    } else {
        vec![0; ordered.len()]
    };
    for (pane_id, depth) in ordered.into_iter().zip(depths) {
        let Some(mut row) = agent_row(snapshot, &pane_id, config, None, depth > 0) else {
            continue;
        };
        row.depth = depth;
        let mut hidden = false;
        if grouped {
            let workspace = snapshot
                .agents
                .iter()
                .find(|agent| agent.pane_id == pane_id)
                .and_then(|agent| {
                    snapshot
                        .workspaces
                        .iter()
                        .find(|workspace| workspace.workspace_id == agent.workspace_id)
                });
            if let Some(workspace) = workspace {
                let collapsed = folded.contains(&workspace.workspace_id);
                if current != Some(workspace.workspace_id.as_str()) {
                    rows.push(PanelRow::Space {
                        label: workspace.label.clone(),
                        workspace_id: workspace.workspace_id.clone(),
                        gap: current.is_some(),
                        collapsed,
                    });
                    current = Some(workspace.workspace_id.as_str());
                }
                hidden = collapsed;
            }
        }
        if !hidden {
            rows.push(PanelRow::Agent(row));
        }
    }
    rows
}

/// The nesting depth of each of `ordered` (pane ids in display order) under
/// the space header it falls below. A pane without an agent row or space
/// would be skipped by the panel, so it also breaks any nesting around it.
fn grouped_depths(snapshot: &ClientShellSnapshot, ordered: &[String]) -> Vec<usize> {
    let rows = ordered
        .iter()
        .map(|pane_id| {
            let agent = snapshot
                .agents
                .iter()
                .find(|agent| agent.pane_id == *pane_id);
            (
                agent.map(|agent| agent.workspace_id.as_str()),
                pane_id.as_str(),
                agent.and_then(|agent| agent.parent.as_deref()),
            )
        })
        .collect::<Vec<_>>();
    super::agent_nesting::depths_by_group(&rows)
}

/// A space header: a fold chevron and the space's name, dim, on the
/// panel's last line of the entry (the first line is the blank gap when
/// there is one). Returns the label line's rect, or `None` when the entry
/// was cut off before it.
pub(super) fn render_space_header(
    buffer: &mut Buffer,
    rect: Rect,
    (label, collapsed): (&str, bool),
    gap: bool,
    config: &super::ClientShellConfig,
) -> Option<Rect> {
    let y = rect.y + u16::from(gap);
    if y >= rect.bottom() {
        return None;
    }
    let style = Style::default()
        .fg(config.palette.overlay0)
        .add_modifier(Modifier::DIM);
    let chevron = if collapsed { '▸' } else { '▾' };
    put_text(
        buffer,
        rect.x,
        y,
        rect.width,
        &format!(" {chevron} {label}"),
        style,
    );
    Some(Rect::new(rect.x, y, rect.width, 1))
}

pub(crate) fn agent_row(
    snapshot: &ClientShellSnapshot,
    pane_id: &str,
    config: &super::ClientShellConfig,
    machine: Option<&str>,
    nested: bool,
) -> Option<AgentRow> {
    let agent = snapshot
        .agents
        .iter()
        .find(|agent| agent.pane_id == pane_id)?;
    let workspace = snapshot
        .workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == agent.workspace_id)?;
    let tab = snapshot.tabs.iter().find(|tab| tab.tab_id == agent.tab_id);
    let pane = snapshot
        .panes
        .iter()
        .find(|pane| pane.pane_id == agent.pane_id);
    let tab_count = snapshot
        .tabs
        .iter()
        .filter(|candidate| candidate.workspace_id == agent.workspace_id)
        .count();
    let tab_label = tab
        .filter(|tab| tab_count > 1 || tab.custom_label)
        .map(|tab| tab.label.as_str());
    let agent_label = agent
        .display_agent
        .as_deref()
        .or(agent.name.as_deref())
        .or(agent.agent.as_deref())
        .or(agent.title.as_deref());
    let labels = agent
        .state_labels
        .iter()
        .cloned()
        .collect::<HashMap<_, _>>();
    let tokens = agent.tokens.iter().cloned().collect::<HashMap<_, _>>();
    let state_text = labels
        .get(status_text(agent.agent_status))
        .map(String::as_str)
        .unwrap_or_else(|| sidebar_status_text(agent.agent_status));
    let canonical_agent = agent
        .agent
        .as_deref()
        .and_then(super::detect::parse_agent_label);
    // A nested worker row reads as the worker's own name, as the Herdr app
    // shows it, instead of the title its program set.
    let own_name = agent.name.as_deref().filter(|_| nested);
    let rows = tokens::agent_rows(
        &config.agents,
        tokens::AgentTokenContext {
            machine,
            workspace: &workspace.label,
            tab: tab_label,
            pane: own_name.or_else(|| {
                agent
                    .title
                    .as_deref()
                    .or_else(|| pane.and_then(|pane| pane.label.as_deref()))
            }),
            agent_label,
            terminal_title: own_name.or(agent.terminal_title.as_deref()),
            terminal_title_stripped: own_name.or(agent.terminal_title_stripped.as_deref()),
            canonical_agent,
            tokens: &tokens,
        },
        state_text,
    );
    Some(AgentRow {
        pane_id: agent.pane_id.clone(),
        status: agent.agent_status,
        focused: agent.focused,
        rows,
        depth: 0,
    })
}

pub(crate) fn render_agent_row(
    buffer: &mut Buffer,
    rect: Rect,
    row: &AgentRow,
    config: &super::ClientShellConfig,
) {
    let palette = &config.palette;
    let row_style = if row.focused {
        Style::default().bg(palette.active_row_bg)
    } else {
        Style::default()
    };
    let name_style = if row.focused {
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(palette.subtext0)
            .add_modifier(Modifier::BOLD)
    };
    let status_style = Style::default().fg(status_color(row.status, palette));
    let secondary = Style::default().fg(palette.overlay0);
    let icon = (
        status_icon(row.status, config.status_indicators),
        Style::default().fg(status_color(row.status, palette)),
    );
    let rows = if row.rows.is_empty() {
        vec![vec![ResolvedToken {
            kind: ResolvedTokenKind::StateIcon,
            style: Default::default(),
        }]]
    } else {
        row.rows.clone()
    };
    for (index, tokens) in rows.iter().take(rect.height as usize).enumerate() {
        // A nested row hangs under its parent's: the first line starts with
        // a connector, the later lines are pushed in by the same depth.
        let (lead, indent) = match (index, row.depth) {
            (0, 0) => (" ".to_owned(), 1),
            (0, depth) => (format!(" {}", "  ".repeat(depth - 1)), 2 * depth + 1),
            (_, depth) => (" ".repeat(3 + 2 * depth), 3 + 2 * depth),
        };
        let mut spans = vec![ratatui::text::Span::raw(lead)];
        if index == 0 && row.depth > 0 {
            spans.push(ratatui::text::Span::styled("└ ", secondary));
        }
        spans.extend(resolved_token_spans(
            tokens,
            icon,
            status_style,
            name_style,
            secondary,
            secondary,
            palette,
            rect.width.saturating_sub(indent as u16) as usize,
        ));
        Paragraph::new(Line::from(spans)).style(row_style).render(
            Rect::new(rect.x, rect.y + index as u16, rect.width, 1),
            buffer,
        );
    }
}

fn sidebar_status_text(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Blocked => "blocked",
        AgentStatus::Done => "done",
        AgentStatus::Working => "working",
        AgentStatus::Waiting => "waiting",
        AgentStatus::Idle | AgentStatus::Unknown => "idle",
    }
}
