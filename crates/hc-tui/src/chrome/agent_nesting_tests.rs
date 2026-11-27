//! Nested worker rows: a worker under its parent's row in the grouped panel.

use crate::chrome::snapshot::ClientShellSnapshot;
use crate::config::{AgentPanelSortConfig, AgentSidebarToken};
use crate::project::project_with_options;
use crate::sidebar::fixture::{agent, view};
use crate::structure::ReadOptions;
use ratatui::{buffer::Buffer, layout::Rect};

const NEST: ReadOptions = ReadOptions { nest_workers: true };

/// `t3` (title "epic: 1") in space `S`, and its worker `t5` with no
/// labels, a name and a title of its own.
fn parent_and_worker(opts: ReadOptions) -> ClientShellSnapshot {
    let mut parent = agent("t3", "idle");
    parent.labels.insert("herdr.space".into(), "S".into());
    parent.title = "epic: 1".into();
    let mut worker = agent("t5", "idle");
    worker.parent = Some("t3".into());
    worker.name = Some("worker-1".into());
    worker.title = "TASK-1, TASK-2".into();
    worker.created_ms = 1;
    project_with_options(&view(vec![parent, worker], vec![]), Some("t3"), opts).snapshot
}

fn config(
    sort: AgentPanelSortConfig,
    headers: bool,
    nest: bool,
) -> crate::chrome::ClientShellConfig {
    let mut shell = crate::chrome::ClientShellConfig::from_config(
        &crate::config::Config::default(),
        None,
        None,
        std::path::PathBuf::new(),
    );
    shell.agent_panel_sort = sort;
    shell.agent_panel_space_headers = headers;
    shell.nest_worker_terminals = nest;
    shell
}

fn entries(
    snapshot: &ClientShellSnapshot,
    shell: &crate::chrome::ClientShellConfig,
) -> Vec<String> {
    crate::chrome::agents::panel_rows(snapshot, shell, &Default::default())
        .into_iter()
        .map(|row| match row {
            crate::chrome::agents::PanelRow::Space { label, gap, .. } => {
                format!("{}[{label}]", if gap { "~" } else { "" })
            }
            crate::chrome::agents::PanelRow::Agent(row) => {
                format!("{} d{}", row.pane_id, row.depth)
            }
        })
        .collect()
}

#[test]
fn the_grouped_panel_nests_a_worker_under_its_parent() {
    let spaces = AgentPanelSortConfig::Spaces;
    let snapshot = parent_and_worker(NEST);
    assert_eq!(
        entries(&snapshot, &config(spaces, true, true)),
        ["[S]", "t3 d0", "t5 d1"]
    );
    // The panel setting off: the same snapshot renders flat.
    assert_eq!(
        entries(&snapshot, &config(spaces, true, false)),
        ["[S]", "t3 d0", "t5 d0"]
    );
    // By priority, and without headers, nothing nests.
    for (sort, headers) in [(AgentPanelSortConfig::Priority, true), (spaces, false)] {
        assert!(
            entries(&snapshot, &config(sort, headers, true))
                .iter()
                .all(|row| row.ends_with("d0") || row.starts_with('[')),
            "{sort:?} {headers}"
        );
    }
    // Flag off all the way: the worker keeps its own fallback group.
    let flat = parent_and_worker(ReadOptions::default());
    assert_eq!(
        entries(&flat, &config(spaces, true, true)),
        ["[S]", "t3 d0", "~[proj]", "t5 d0"]
    );
}

#[test]
fn a_worker_of_a_parent_with_no_row_sits_flat_in_the_space_group() {
    let mut shell = crate::sidebar::fixture::term("t3");
    shell.labels.insert("herdr.space".into(), "S".into());
    shell.labels.insert("herdr.tab".into(), "shell".into());
    shell.created_ms = 2;
    let mut sibling = agent("t4", "idle");
    sibling.labels.insert("herdr.space".into(), "S".into());
    sibling.labels.insert("herdr.tab".into(), "work".into());
    sibling.created_ms = 1;
    let mut worker = agent("t5", "idle");
    worker.parent = Some("t3".into());
    worker.created_ms = 2;
    let snapshot = project_with_options(
        &view(vec![shell, sibling, worker], vec![]),
        Some("t4"),
        NEST,
    )
    .snapshot;
    assert_eq!(
        entries(&snapshot, &config(AgentPanelSortConfig::Spaces, true, true)),
        ["[S]", "t4 d0", "t5 d0"],
        "t5 is not nested under t4, which it was not started from"
    );
}

#[test]
fn only_a_worker_with_no_tab_nests_never_a_labelled_child() {
    let labelled = |address: &str, tab: &str, created_ms: u64| {
        let mut term = agent(address, "idle");
        term.labels.insert("herdr.space".into(), "S".into());
        term.labels.insert("herdr.tab".into(), tab.into());
        term.created_ms = created_ms;
        term
    };
    let mut child = labelled("t4", "two", 1);
    child.parent = Some("t3".into());
    let snapshot = project_with_options(
        &view(vec![labelled("t3", "one", 0), child], vec![]),
        Some("t3"),
        NEST,
    )
    .snapshot;
    assert_eq!(
        entries(&snapshot, &config(AgentPanelSortConfig::Spaces, true, true)),
        ["[S]", "t3 d0", "t4 d0"]
    );
}

fn line(buffer: &Buffer, width: u16) -> String {
    (0..width)
        .map(|x| buffer[(x, 0)].symbol())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn a_nested_row_draws_a_connector_and_reads_as_the_workers_name() {
    let mut shell = config(AgentPanelSortConfig::Spaces, true, true);
    shell.agents.rows = vec![vec![
        AgentSidebarToken::StateIcon,
        AgentSidebarToken::TerminalTitleStripped,
    ]];
    let render = |snapshot: &ClientShellSnapshot, shell: &crate::chrome::ClientShellConfig| {
        crate::chrome::agents::panel_rows(snapshot, shell, &Default::default())
            .into_iter()
            .filter_map(|row| match row {
                crate::chrome::agents::PanelRow::Agent(row) => {
                    let mut buffer = Buffer::empty(Rect::new(0, 0, 30, 1));
                    crate::chrome::agents::render_agent_row(
                        &mut buffer,
                        Rect::new(0, 0, 30, 1),
                        &row,
                        shell,
                    );
                    let icon =
                        crate::chrome::status::status_icon(row.status, shell.status_indicators);
                    Some((row.pane_id, icon, buffer))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let rows = render(&parent_and_worker(NEST), &shell);
    let (parent, worker) = (&rows[0], &rows[1]);
    assert_eq!(line(&parent.2, 30), format!(" {} epic: 1", parent.1));
    assert_eq!(line(&worker.2, 30), format!(" └ {} worker-1", worker.1));
    assert_eq!(worker.2[(1, 0)].fg, shell.palette.overlay0);
    // Flag off: the worker keeps the title its program set.
    shell.nest_worker_terminals = false;
    let flat = render(&parent_and_worker(NEST), &shell);
    assert_eq!(
        line(&flat[1].2, 30),
        format!(" {} TASK-1, TASK-2", flat[1].1)
    );
}

#[test]
fn deeper_rows_hang_further_in_and_later_lines_follow() {
    let mut shell = config(AgentPanelSortConfig::Spaces, true, true);
    shell.agents.rows = vec![
        vec![AgentSidebarToken::StateIcon, AgentSidebarToken::Workspace],
        vec![AgentSidebarToken::Agent],
    ];
    let snapshot = parent_and_worker(NEST);
    let mut row =
        match crate::chrome::agents::panel_rows(&snapshot, &shell, &Default::default()).remove(2) {
            crate::chrome::agents::PanelRow::Agent(row) => row,
            _ => unreachable!(),
        };
    row.depth = 2;
    let mut buffer = Buffer::empty(Rect::new(0, 0, 30, 2));
    crate::chrome::agents::render_agent_row(&mut buffer, Rect::new(0, 0, 30, 2), &row, &shell);
    assert_eq!(buffer[(3, 0)].symbol(), "└");
    let second = (0..30).map(|x| buffer[(x, 1)].symbol()).collect::<String>();
    assert!(second.starts_with("       "), "{second:?}");
}
