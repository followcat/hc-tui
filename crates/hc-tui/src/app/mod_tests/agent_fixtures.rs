//! Fixtures for the grouped agents panel: apps grouped by space with
//! headers on, agents spread over spaces, and a left press at a rect.
use super::endpoint_nav::{app_multi, pending_attach};
use super::*;
use crate::sidebar::fixture::{agent, machine};
use hc_proto::terminal::Terminal;
use ratatui::layout::Rect;

/// A working agent of `space`, one tab.
pub(super) fn spaced(address: &str, space: &str, created_ms: u64) -> Terminal {
    let mut term = agent(address, "working");
    term.created_ms = created_ms;
    term.labels.insert("herdr.space".into(), space.into());
    term.labels.insert("herdr.tab".into(), "tab".into());
    term
}

/// An app grouped by space with headers on, drawn once so the hit map is
/// filled.
pub(super) fn grouped_app(
    terms: Vec<Terminal>,
    machines: Vec<hc_proto::machine::MachineView>,
) -> App {
    let mut app = app_multi(terms, machines);
    app.area = (120, 40);
    app.shell.agent_panel_sort = crate::config::AgentPanelSortConfig::Spaces;
    app.shell.agent_panel_space_headers = true;
    pending_attach(&mut app);
    app.refresh_projection();
    app
}

/// Space `S` holds `t3` and `t5`, space `U` holds `t9`; `t3` has focus.
pub(super) fn single() -> App {
    let mut app = grouped_app(
        vec![
            spaced("t3", "S", 0),
            spaced("t5", "S", 1),
            spaced("t9", "U", 2),
        ],
        vec![],
    );
    app.focused = Some("t3".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    app
}

/// The local `t1` in `one`, and `host-a`'s `t3` and `t5` in `S`; `t1` has
/// focus.
pub(super) fn multi() -> App {
    let mut app = grouped_app(
        vec![
            spaced("t1", "one", 0),
            spaced("host-a/t3", "S", 0),
            spaced("host-a/t5", "S", 1),
        ],
        vec![machine("host-a", "connected")],
    );
    app.focused = Some("t1".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    app
}

/// A left press at `rect`'s corner, through the whole mouse dispatch.
pub(super) fn press(app: &mut App, rect: Rect) {
    app.on_mouse(input::MouseInput {
        kind: input::MouseEventKind::Down(input::MouseButton::Left),
        col: rect.x,
        row: rect.y,
        mods: hc_vt::Mods::NONE,
        sgr_position: None,
    });
}

/// The id of the space labelled `label` in the active snapshot.
pub(super) fn workspace_of(app: &App, label: &str) -> String {
    app.projection
        .snapshot
        .workspaces
        .iter()
        .find(|workspace| workspace.label == label)
        .unwrap_or_else(|| panic!("no space {label}"))
        .workspace_id
        .clone()
}

/// The label line of the single-machine panel's header for `workspace_id`.
pub(super) fn header(app: &App, workspace_id: &str) -> Rect {
    app.hit_map
        .extra
        .agent_space_headers
        .iter()
        .find(|(_, id)| id == workspace_id)
        .unwrap_or_else(|| panic!("no header for {workspace_id}"))
        .0
}

/// The agent rows the single-machine panel drew, top to bottom.
pub(super) fn rows(app: &App) -> Vec<&str> {
    app.hit_map
        .agents
        .iter()
        .map(|(_, id)| id.as_str())
        .collect()
}

/// The local endpoint's folded spaces, sorted.
pub(super) fn local_folds(app: &App) -> Vec<String> {
    let mut folds = app
        .shell_state
        .collapsed_agent_spaces
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    folds.sort();
    folds
}
