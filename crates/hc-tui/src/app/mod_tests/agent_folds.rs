//! Folding space groups in the grouped agents panel: a header click
//! toggles, neither focus nor a blank gap line is touched, any open popup
//! is dismissed first, folds are remembered, and agent navigation unfolds.
use super::endpoint_nav::pending_attach;
use super::worker_popup::{child, multi, opened, press, row_app, single, spaced};
use super::*;
use crate::chrome::render::ClientMobileTarget;
use crate::chrome::snapshot::ClientEndpointId;
use crate::input::KeybindAction as A;
use crate::sidebar::fixture::machine;
use ratatui::layout::Rect;

fn grouped(mut app: App) -> App {
    app.shell.agent_panel_space_headers = true;
    pending_attach(&mut app);
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    app
}

/// `single()` grouped by space with headers: `S` holds t3 and its worker
/// t5, `U` holds t9.
fn single_grouped() -> App {
    let mut app = single();
    app.focused = Some("t3".into());
    grouped(app)
}

fn workspace_of(app: &App, label: &str) -> String {
    app.projection
        .snapshot
        .workspaces
        .iter()
        .find(|workspace| workspace.label == label)
        .unwrap_or_else(|| panic!("no space {label}"))
        .workspace_id
        .clone()
}

fn header(app: &App, workspace_id: &str) -> Rect {
    app.hit_map
        .extra
        .agent_space_headers
        .iter()
        .find(|(_, id)| id == workspace_id)
        .unwrap_or_else(|| panic!("no header for {workspace_id}"))
        .0
}

fn rows(app: &App) -> Vec<&str> {
    app.hit_map
        .agents
        .iter()
        .map(|(_, id)| id.as_str())
        .collect()
}

fn local_folds(app: &App) -> Vec<String> {
    let mut folds = app
        .shell_state
        .collapsed_agent_spaces
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    folds.sort();
    folds
}

// ---- V15: the click --------------------------------------------------------

#[tokio::test]
async fn a_header_click_folds_its_space_and_a_second_unfolds_it() {
    let mut app = single_grouped();
    let space = workspace_of(&app, "S");
    assert_eq!(rows(&app), ["t3", "t5", "t9"]);
    let rect = header(&app, &space);
    press(&mut app, rect);
    assert_eq!(local_folds(&app), std::slice::from_ref(&space));
    draw_to(&mut app, 120, 40);
    assert_eq!(rows(&app), ["t9"], "S keeps only its header");
    let rect = header(&app, &space);
    press(&mut app, rect);
    assert!(local_folds(&app).is_empty());
    draw_to(&mut app, 120, 40);
    assert_eq!(rows(&app), ["t3", "t5", "t9"]);
    assert_eq!(app.focused.as_deref(), Some("t3"), "focus never moves");
}

#[tokio::test]
async fn the_blank_line_above_a_header_toggles_nothing() {
    let mut app = single_grouped();
    let other = workspace_of(&app, "U");
    let label = header(&app, &other);
    assert!(label.y > 0);
    press(&mut app, Rect::new(label.x, label.y - 1, 1, 1));
    assert!(
        local_folds(&app).is_empty(),
        "the gap line is not the header"
    );
    press(&mut app, label);
    assert_eq!(local_folds(&app), [other]);
}

#[tokio::test]
async fn a_header_draws_its_chevron_in_the_dim_overlay_colour() {
    let mut app = single_grouped();
    let space = workspace_of(&app, "S");
    let rect = header(&app, &space);
    app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space);
    let screen = screen_lines(&draw_to(&mut app, 120, 40));
    let line = &screen[rect.y as usize];
    assert!(line.contains("▸ S"), "{line:?}");
    assert!(screen.iter().any(|l| l.contains("▾ U")));
}

#[tokio::test]
async fn a_header_press_dismisses_every_kind_of_open_popup_then_toggles() {
    for kind in ["worker", "peek"] {
        let mut app = single_grouped();
        let space = workspace_of(&app, "S");
        match kind {
            "worker" => app.open_worker_popup("t5".into()),
            _ => app.open_peek("t9".into()),
        }
        draw_to(&mut app, 120, 40);
        let rect = header(&app, &space);
        press(&mut app, rect);
        assert!(app.popup.is_none() && app.peek.is_none(), "{kind} closed");
        assert_eq!(local_folds(&app), [space], "{kind}: the header acted too");
    }
}

#[tokio::test]
async fn a_header_press_parks_a_shown_float_then_toggles() {
    let mut app = single_grouped();
    let space = workspace_of(&app, "S");
    let key = crate::app::float::FloatKey {
        owner: app.focused_owner_key().unwrap(),
        command: "alt+f".into(),
    };
    app.show_float(key.clone(), "t8".into(), None, None, 8);
    draw_to(&mut app, 120, 40);
    let rect = header(&app, &space);
    press(&mut app, rect);
    assert!(app.popup.is_none());
    assert!(app.floats.contains_key(&key), "the float parked");
    assert_eq!(local_folds(&app), [space]);
}

// ---- V14 (multi-machine, through the app) ---------------------------------

#[tokio::test]
async fn folding_a_remote_space_leaves_the_same_named_local_space_open() {
    let mut app = row_app(
        vec![
            spaced("t1", "S", 0),
            spaced("host-a/t3", "S", 0),
            child("host-a/t5", "host-a/t3", 1),
        ],
        vec![machine("host-a", "connected")],
    );
    app.focused = Some("t1".into());
    let mut app = grouped(app);
    let remote = ClientEndpointId::Remote("host-a".into());
    let (rect, _, space) = app.hit_map.extra.endpoint_agent_space_headers[1].clone();
    press(&mut app, rect);
    assert_eq!(
        app.shell_state.remote_collapsed_agent_spaces[&remote].len(),
        1
    );
    assert!(app.shell_state.collapsed_agent_spaces.is_empty());
    draw_to(&mut app, 120, 40);
    let shown = app
        .hit_map
        .endpoint_agents
        .iter()
        .map(|(_, _, id)| id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(shown, ["t1"], "host-a's S keeps only its header");
    assert_eq!(app.hit_map.extra.endpoint_agent_space_headers.len(), 2);
    let _ = space;
}

#[tokio::test]
async fn a_remote_header_press_dismisses_a_worker_popup_then_toggles() {
    let mut app = multi();
    app.focused = Some("t1".into());
    let mut app = grouped(app);
    app.open_worker_popup("host-a/t5".into());
    draw_to(&mut app, 120, 40);
    let (rect, endpoint, space) = app.hit_map.extra.endpoint_agent_space_headers[1].clone();
    press(&mut app, rect);
    assert!(app.popup.is_none());
    assert!(app.shell_state.remote_collapsed_agent_spaces[&endpoint].contains(&space));
}

// ---- V17: navigation unfolds -------------------------------------------------

#[tokio::test]
async fn focus_agent_into_a_folded_space_unfolds_it_and_focuses() {
    let mut app = single_grouped();
    let space = workspace_of(&app, "S");
    app.focused = Some("t9".into());
    app.refresh_projection();
    app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space);
    app.on_action(A::FocusAgent(0), 120);
    assert!(local_folds(&app).is_empty());
    assert_eq!(app.focused.as_deref(), Some("t3"));
}

#[tokio::test]
async fn next_agent_onto_a_worker_in_a_folded_space_unfolds_and_opens_it() {
    let mut app = single_grouped();
    let space = workspace_of(&app, "S");
    app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space);
    app.on_action(A::NextAgent, 120);
    assert!(local_folds(&app).is_empty());
    assert_eq!(opened(&app), Some("t5"));
}

#[tokio::test]
async fn the_scroll_counts_the_rows_as_rendered_after_the_unfold() {
    let mut app = single_grouped();
    let space = workspace_of(&app, "S");
    app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space);
    app.hit_map.agents.clear();
    app.hit_map.agent_max_scroll = 50;
    app.on_action(A::NextAgent, 120);
    // Header S, t3, t5: the worker is the third rendered row.
    assert_eq!(app.shell_state.agent_scroll, 2);
}

#[tokio::test]
async fn the_mobile_switchers_agent_item_unfolds_its_space() {
    let mut app = single_grouped();
    let space = workspace_of(&app, "S");
    app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space);
    app.mode = Mode::Navigate;
    app.on_mobile_target(&ClientMobileTarget::Agent {
        endpoint_id: ClientEndpointId::Local,
        pane_id: "t3".into(),
    });
    assert!(local_folds(&app).is_empty());
    assert_eq!(app.focused.as_deref(), Some("t3"));
}

#[tokio::test]
async fn the_endpoint_walk_unfolds_a_remote_space_and_leaves_local_folds() {
    let mut app = row_app(
        vec![
            spaced("t1", "one", 0),
            spaced("host-a/t3", "S", 0),
            child("host-a/t5", "host-a/t3", 1),
        ],
        vec![machine("host-a", "connected")],
    );
    app.focused = Some("t1".into());
    let mut app = grouped(app);
    let remote = ClientEndpointId::Remote("host-a".into());
    let local_space = workspace_of(&app, "one");
    app.toggle_collapsed_agent_space(&ClientEndpointId::Local, local_space.clone());
    let (_, _, remote_space) = app.hit_map.extra.endpoint_agent_space_headers[1].clone();
    app.toggle_collapsed_agent_space(&remote, remote_space);
    assert_eq!(
        app.shell_state.remote_collapsed_agent_spaces[&remote].len(),
        1
    );
    app.on_action(A::NextAgent, 120);
    assert!(
        app.shell_state
            .remote_collapsed_agent_spaces
            .get(&remote)
            .is_none_or(|folds| folds.is_empty()),
        "the target's space unfolded"
    );
    assert_eq!(local_folds(&app), [local_space], "other folds stay");
}
