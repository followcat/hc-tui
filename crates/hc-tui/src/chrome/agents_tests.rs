//! Space headers in the grouped agents panel.

use crate::project::project;
use crate::sidebar::fixture::{agent, view};

fn panel(
    snapshot: &crate::chrome::snapshot::ClientShellSnapshot,
    sort: crate::config::AgentPanelSortConfig,
    space_headers: bool,
) -> Vec<String> {
    let shell = shell_for(sort, space_headers);
    super::panel_rows(snapshot, &shell, &Default::default())
        .into_iter()
        .map(|row| match row {
            super::PanelRow::Space { label, gap, .. } => {
                format!("{}[{label}]", if gap { "~" } else { "" })
            }
            super::PanelRow::Agent(agent) => agent.pane_id,
        })
        .collect()
}

fn two_spaces() -> crate::chrome::snapshot::ClientShellSnapshot {
    let spaced = |address: &str, word: &str, space: &str| {
        let mut agent = agent(address, word);
        agent.labels.insert("herdr.space".into(), space.into());
        agent
    };
    let terms = vec![
        spaced("t1", "idle", "bambooworks"),
        spaced("t2", "working", "bambooworks"),
        spaced("t3", "blocked", "dev-workspace"),
    ];
    project(&view(terms, vec![]), Some("t1")).snapshot
}

#[test]
fn space_headers_head_each_space_of_the_grouped_agents_panel() {
    assert_eq!(
        panel(
            &two_spaces(),
            crate::config::AgentPanelSortConfig::Spaces,
            true
        ),
        ["[bambooworks]", "t1", "t2", "~[dev-workspace]", "t3"]
    );
}

#[test]
fn grouped_agents_panel_has_no_headers_unless_asked() {
    assert!(!crate::config::UiConfig::default().agent_panel_space_headers);
    assert_eq!(
        panel(
            &two_spaces(),
            crate::config::AgentPanelSortConfig::Spaces,
            false
        ),
        ["t1", "t2", "t3"]
    );
}

#[test]
fn priority_agents_panel_stays_flat_with_space_headers_on() {
    assert_eq!(
        panel(
            &two_spaces(),
            crate::config::AgentPanelSortConfig::Priority,
            true
        ),
        ["t3", "t2", "t1"]
    );
}

/// Space `S` (`t3`, `t5`) and space `U` (`t9`).
fn s_and_u() -> crate::chrome::snapshot::ClientShellSnapshot {
    let spaced = |address: &str, space: &str, created_ms: u64| {
        let mut term = agent(address, "idle");
        term.labels.insert("herdr.space".into(), space.into());
        term.created_ms = created_ms;
        term
    };
    let terms = vec![
        spaced("t3", "S", 0),
        spaced("t5", "S", 1),
        spaced("t9", "U", 2),
    ];
    project(&view(terms, vec![]), Some("t3")).snapshot
}

fn space_id(snapshot: &crate::chrome::snapshot::ClientShellSnapshot, label: &str) -> String {
    snapshot
        .workspaces
        .iter()
        .find(|workspace| workspace.label == label)
        .unwrap_or_else(|| panic!("no space {label}"))
        .workspace_id
        .clone()
}

fn shell_for(
    sort: crate::config::AgentPanelSortConfig,
    headers: bool,
) -> crate::chrome::ClientShellConfig {
    let mut shell = crate::chrome::ClientShellConfig::from_config(
        &crate::config::Config::default(),
        None,
        None,
        std::path::PathBuf::new(),
    );
    shell.agent_panel_sort = sort;
    shell.agent_panel_space_headers = headers;
    shell
}

/// The panel's rows with `folded` spaces folded: `▾`/`▸` mark a header's
/// state, `~` a blank gap above it.
fn folded_rows(
    snapshot: &crate::chrome::snapshot::ClientShellSnapshot,
    shell: &crate::chrome::ClientShellConfig,
    folded: &[&str],
) -> Vec<String> {
    let folded = folded.iter().map(|id| (*id).to_owned()).collect();
    super::panel_rows(snapshot, shell, &folded)
        .into_iter()
        .map(|row| match row {
            super::PanelRow::Space {
                label,
                gap,
                collapsed,
                ..
            } => format!(
                "{}{}[{label}]",
                if gap { "~" } else { "" },
                if collapsed { "▸" } else { "▾" }
            ),
            super::PanelRow::Agent(row) => row.pane_id,
        })
        .collect()
}

#[test]
fn a_folded_space_keeps_its_header_and_loses_its_agents() {
    use crate::config::AgentPanelSortConfig::{Priority, Spaces};
    let snapshot = s_and_u();
    let shell = shell_for(Spaces, true);
    let (s, u) = (space_id(&snapshot, "S"), space_id(&snapshot, "U"));
    assert_eq!(
        folded_rows(&snapshot, &shell, &[]),
        ["▾[S]", "t3", "t5", "~▾[U]", "t9"]
    );
    assert_eq!(
        folded_rows(&snapshot, &shell, &[&s]),
        ["▸[S]", "~▾[U]", "t9"]
    );
    assert_eq!(folded_rows(&snapshot, &shell, &[&s, &u]), ["▸[S]", "~▸[U]"]);
    // Without headers, or by priority, there is nothing to click: the fold
    // set is ignored.
    for (sort, headers) in [(Priority, true), (Spaces, false)] {
        let flat = shell_for(sort, headers);
        let mut rows = folded_rows(&snapshot, &flat, &[&s]);
        rows.sort();
        assert_eq!(rows, ["t3", "t5", "t9"], "{sort:?} {headers}");
    }
}

#[test]
fn a_header_draws_one_label_line_hit_below_its_gap() {
    use ratatui::{buffer::Buffer, layout::Rect};
    let snapshot = s_and_u();
    let shell = shell_for(crate::config::AgentPanelSortConfig::Spaces, true);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 30, 12));
    let mut hits = crate::chrome::render::ShellHitMap::default();
    let (mut scroll, mut reveal) = (0, false);
    let folded = std::iter::once(space_id(&snapshot, "S")).collect();
    super::render_agent_panel(
        &mut buffer,
        Rect::new(0, 0, 30, 12),
        &snapshot,
        &shell,
        &folded,
        &mut scroll,
        &mut reveal,
        &mut hits,
    );
    let lines = (0..12)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    let s_line = lines
        .iter()
        .position(|l| l.contains("▸ S"))
        .expect("folded S");
    let u_line = lines
        .iter()
        .position(|l| l.contains("▾ U"))
        .expect("open U");
    assert_eq!(u_line, s_line + 2, "a blank gap line sits above U's header");
    assert!(!lines.iter().any(|l| l.contains("t3")));
    let rects = hits
        .extra
        .agent_space_headers
        .iter()
        .map(|(rect, _)| (rect.y as usize, rect.height))
        .collect::<Vec<_>>();
    assert_eq!(rects, [(s_line, 1), (u_line, 1)], "label lines only");
    let chevron = &buffer[(1, s_line as u16)];
    assert_eq!(chevron.symbol(), "▸");
    assert_eq!(chevron.fg, shell.palette.overlay0);
}
