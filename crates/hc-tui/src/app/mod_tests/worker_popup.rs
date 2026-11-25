//! Worker terminals that get no tab: creation never labels one, and the
//! creation re-read lists the same terminals the projection does.
use super::endpoint_nav::{app_multi, pending_attach};
use super::*;
use crate::chrome::render::ClientMobileTarget;
use crate::chrome::status::AgentStatus;
use crate::input::KeybindAction as A;
use crate::sidebar::fixture::{agent, machine};
use hc_proto::terminal::Terminal;

fn worker(address: &str, parent: &str, created_ms: u64) -> Terminal {
    let mut term = crate::sidebar::fixture::term(address);
    term.created_ms = created_ms;
    term.parent = Some(parent.into());
    term.name = Some("bwau-1".into());
    term
}

fn nesting(terms: Vec<Terminal>) -> App {
    let mut app = app_with_terms(terms);
    app.shell.nest_worker_terminals = true;
    app.focused = Some("t3".into());
    app.refresh_projection();
    app
}

fn promoted(app: &App) -> Vec<String> {
    let creation = app.creation(None, false).unwrap();
    creation
        .promotion
        .expect("the unlabelled space is promoted")
        .members
        .into_iter()
        .map(|member| member.terminal)
        .collect()
}

#[tokio::test]
async fn a_new_tab_labels_no_hidden_worker() {
    let app = nesting(vec![
        crate::sidebar::fixture::term("t3"),
        worker("t5", "t3", 1),
    ]);
    assert_eq!(
        app.projection.snapshot.workspaces.len(),
        1,
        "the worker is hidden in its parent's space"
    );
    assert_eq!(promoted(&app), ["t3"]);
}

#[tokio::test]
async fn flag_off_a_new_tab_still_sees_the_worker_as_its_own_space() {
    let mut app = nesting(vec![
        crate::sidebar::fixture::term("t3"),
        worker("t5", "t3", 1),
    ]);
    app.shell.nest_worker_terminals = false;
    app.refresh_projection();
    assert_eq!(promoted(&app), ["t3", "t5"]);
}

#[tokio::test]
async fn the_creation_re_read_drops_what_the_projection_drops() {
    // A popup-owned terminal is not projected, so its worker is an orphan
    // there and sits in the space as a tab. The re-read must agree.
    let mut popup = crate::sidebar::fixture::term("t7");
    popup.created_ms = 1;
    popup.labels.insert(popup::POPUP_LABEL.into(), "1".into());
    let app = nesting(vec![
        crate::sidebar::fixture::term("t3"),
        popup,
        worker("t8", "t7", 2),
    ]);
    assert_eq!(promoted(&app), ["t3", "t8"]);
}

#[tokio::test]
async fn a_blocked_worker_lights_its_parents_tab_and_space() {
    let mut parent = crate::sidebar::fixture::agent("t3", "working");
    parent.created_ms = 0;
    let mut child = crate::sidebar::fixture::agent("t5", "blocked");
    child.created_ms = 1;
    child.parent = Some("t3".into());
    let app = nesting(vec![parent, child]);
    let snapshot = &app.projection.snapshot;
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.tabs[0].agent_status, AgentStatus::Blocked);
    assert_eq!(snapshot.workspaces[0].agent_status, AgentStatus::Blocked);
}

/// An app grouped by space, so a worker's row nests under its parent's.
fn row_app(terms: Vec<Terminal>, machines: Vec<hc_proto::machine::MachineView>) -> App {
    let mut app = app_multi(terms, machines);
    app.shell.nest_worker_terminals = true;
    app.shell.agent_panel_sort = crate::config::AgentPanelSortConfig::Spaces;
    pending_attach(&mut app);
    app.refresh_projection();
    app
}

fn spaced(address: &str, space: &str, created_ms: u64) -> Terminal {
    let mut term = agent(address, "working");
    term.created_ms = created_ms;
    term.labels.insert("herdr.space".into(), space.into());
    term.labels.insert("herdr.tab".into(), "tab".into());
    term
}

fn child(address: &str, parent: &str, created_ms: u64) -> Terminal {
    let mut term = agent(address, "blocked");
    term.created_ms = created_ms;
    term.parent = Some(parent.into());
    term.name = Some("bwau-1".into());
    term
}

fn peeked(app: &App) -> Option<&str> {
    app.peek.as_ref().map(|peek| peek.terminal.as_str())
}

fn press(app: &mut App, rect: ratatui::layout::Rect) {
    app.mouse_press(
        &input::MouseInput {
            kind: input::MouseEventKind::Down(input::MouseButton::Left),
            col: rect.x,
            row: rect.y,
            mods: hc_vt::Mods::NONE,
            sgr_position: None,
        },
        None,
    );
}

fn single() -> App {
    row_app(
        vec![
            spaced("t3", "S", 0),
            child("t5", "t3", 1),
            spaced("t9", "U", 2),
        ],
        vec![],
    )
}

#[tokio::test]
async fn a_click_on_a_worker_row_opens_it_and_leaves_focus_alone() {
    let mut app = single();
    app.focused = Some("t3".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    let rect = app
        .hit_map
        .agents
        .iter()
        .find(|(_, id)| id == "t5")
        .unwrap()
        .0;
    press(&mut app, rect);
    assert_eq!(peeked(&app), Some("t5"));
    assert_eq!(app.focused.as_deref(), Some("t3"));
}

#[tokio::test]
async fn a_click_on_an_ordinary_row_still_focuses_its_pane() {
    let mut app = single();
    app.focused = Some("t3".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    let rect = app
        .hit_map
        .agents
        .iter()
        .find(|(_, id)| id == "t9")
        .unwrap()
        .0;
    press(&mut app, rect);
    assert!(app.peek.is_none());
    assert_eq!(app.focused.as_deref(), Some("t9"));
}

#[tokio::test]
async fn focus_agent_and_next_agent_open_a_worker_without_focusing_it() {
    let mut app = single();
    app.focused = Some("t3".into());
    app.refresh_projection();
    let ids = crate::chrome::agents::ordered_agent_pane_ids(
        &app.projection.snapshot,
        app.shell.agent_panel_sort,
    );
    assert_eq!(ids, ["t3", "t5", "t9"]);
    app.on_action(A::FocusAgent(1), 120);
    assert_eq!(peeked(&app), Some("t5"));
    assert_eq!(app.focused.as_deref(), Some("t3"));
    app.close_peek();
    app.on_action(A::NextAgent, 120);
    assert_eq!(peeked(&app), Some("t5"));
    assert_eq!(app.focused.as_deref(), Some("t3"));
}

#[tokio::test]
async fn the_mobile_switchers_agent_item_opens_a_worker_and_leaves_navigate() {
    let mut app = single();
    app.focused = Some("t3".into());
    app.refresh_projection();
    app.mode = Mode::Navigate;
    app.on_mobile_target(&ClientMobileTarget::Agent {
        endpoint_id: crate::chrome::snapshot::ClientEndpointId::Local,
        pane_id: "t5".into(),
    });
    assert_eq!(peeked(&app), Some("t5"));
    assert_eq!(app.mode, Mode::Normal);
    assert!(app.navigate_target.is_none());
    assert_eq!(app.focused.as_deref(), Some("t3"));
}

fn multi() -> App {
    row_app(
        vec![
            spaced("t1", "one", 0),
            spaced("host-a/t3", "S", 0),
            child("host-a/t5", "host-a/t3", 1),
        ],
        vec![machine("host-a", "connected")],
    )
}

#[tokio::test]
async fn a_click_on_a_remote_worker_row_opens_its_peek_and_keeps_focus() {
    let mut app = multi();
    app.focused = Some("t1".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    let rect = app
        .hit_map
        .endpoint_agents
        .iter()
        .find(|(_, _, id)| id == "host-a/t5")
        .expect("the remote worker row is drawn")
        .0;
    press(&mut app, rect);
    assert_eq!(peeked(&app), Some("host-a/t5"));
    assert_eq!(app.focused.as_deref(), Some("t1"));
}

#[tokio::test]
async fn the_endpoint_keyboard_walk_opens_a_remote_worker_without_focusing_it() {
    let mut app = multi();
    app.focused = Some("host-a/t3".into());
    app.refresh_projection();
    app.on_action(A::NextAgent, 120);
    assert_eq!(peeked(&app), Some("host-a/t5"));
    assert_eq!(app.focused.as_deref(), Some("host-a/t3"));
}
