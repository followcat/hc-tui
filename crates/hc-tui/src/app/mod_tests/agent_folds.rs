//! Folding space groups in the grouped agents panel: a header click
//! toggles, neither focus nor a blank gap line is touched, folds are
//! remembered per machine, and agent navigation unfolds.
use super::agent_fixtures::{
    grouped_app, header, local_folds, multi, press, rows, single, spaced, workspace_of,
};
use super::*;
use crate::chrome::render::ClientMobileTarget;
use crate::chrome::snapshot::ClientEndpointId;
use crate::input::KeybindAction as A;
use crate::sidebar::fixture::machine;
use ratatui::layout::Rect;

// ---- the click --------------------------------------------------------------

#[tokio::test]
async fn a_header_click_folds_its_space_and_a_second_unfolds_it() {
    let mut app = single();
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
    let mut app = single();
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
async fn a_folded_header_draws_a_right_chevron_and_an_open_one_a_down_chevron() {
    let mut app = single();
    let space = workspace_of(&app, "S");
    let rect = header(&app, &space);
    app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space);
    let screen = screen_lines(&draw_to(&mut app, 120, 40));
    let line = &screen[rect.y as usize];
    assert!(line.contains("▸ S"), "{line:?}");
    assert!(screen.iter().any(|l| l.contains("▾ U")));
}

// ---- multi-machine, through the app -------------------------------------------

#[tokio::test]
async fn folding_a_remote_space_leaves_the_same_named_local_space_open() {
    let mut app = grouped_app(
        vec![
            spaced("t1", "S", 0),
            spaced("host-a/t3", "S", 0),
            spaced("host-a/t5", "S", 1),
        ],
        vec![machine("host-a", "connected")],
    );
    app.focused = Some("t1".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    let remote = ClientEndpointId::Remote("host-a".into());
    let (rect, _, _) = app.hit_map.extra.endpoint_agent_space_headers[1].clone();
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
}

// ---- navigation unfolds ---------------------------------------------------------

#[tokio::test]
async fn focus_agent_into_a_folded_space_unfolds_it_and_focuses() {
    let mut app = single();
    let space = workspace_of(&app, "S");
    app.focused = Some("t9".into());
    app.refresh_projection();
    app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space);
    app.on_action(A::FocusAgent(0), 120);
    assert!(local_folds(&app).is_empty());
    assert_eq!(app.focused.as_deref(), Some("t3"));
}

#[tokio::test]
async fn next_agent_within_a_folded_space_unfolds_it_and_focuses() {
    let mut app = single();
    let space = workspace_of(&app, "S");
    app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space);
    app.on_action(A::NextAgent, 120);
    assert!(local_folds(&app).is_empty());
    assert_eq!(app.focused.as_deref(), Some("t5"));
}

#[tokio::test]
async fn the_scroll_counts_the_rows_as_rendered_after_the_unfold() {
    let mut app = grouped_app(
        vec![
            spaced("t3", "S", 0),
            spaced("t5", "S", 1),
            spaced("t9", "U", 2),
            spaced("t11", "V", 3),
        ],
        vec![],
    );
    app.focused = Some("t9".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    for label in ["S", "U"] {
        let space = workspace_of(&app, label);
        app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space);
    }
    app.hit_map.agents.clear();
    app.hit_map.agent_max_scroll = 50;
    app.on_action(A::NextAgent, 120);
    // Headers S and U (both folded), header V, then t11: the folded t3, t5
    // and t9 take no rows.
    assert_eq!(app.shell_state.agent_scroll, 3);
    app.hit_map.agents.clear();
    app.on_action(A::PreviousAgent, 120);
    // U unfolds for the move onto t9, S stays folded: headers S, U, then t9.
    assert_eq!(app.shell_state.agent_scroll, 2);
    assert_eq!(local_folds(&app), [workspace_of(&app, "S")]);
}

#[tokio::test]
async fn the_mobile_switchers_agent_item_unfolds_its_space() {
    let mut app = single();
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
    let mut app = multi();
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

#[tokio::test]
async fn agent_indexes_ignore_folds() {
    let mut app = single();
    let space = workspace_of(&app, "S");
    app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space.clone());
    app.on_action(A::FocusAgent(2), 120);
    assert_eq!(app.focused.as_deref(), Some("t9"), "index 2 is still t9");
    assert_eq!(
        local_folds(&app),
        [space],
        "an agent below the fold unfolds nothing"
    );
    app.on_action(A::FocusAgent(1), 120);
    assert_eq!(
        app.focused.as_deref(),
        Some("t5"),
        "index 1 is the folded t5"
    );
    assert!(local_folds(&app).is_empty());
}

#[tokio::test]
async fn navigation_leaves_a_saved_fold_alone_unless_the_panel_shows_headers() {
    use crate::config::AgentPanelSortConfig::{Priority, Spaces};
    for (sort, headers) in [(Spaces, false), (Priority, true), (Priority, false)] {
        let mut app = single();
        app.shell.agent_panel_sort = sort;
        app.shell.agent_panel_space_headers = headers;
        let space = workspace_of(&app, "S");
        app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space.clone());
        for index in 0..3 {
            app.on_action(A::FocusAgent(index), 120);
        }
        app.on_action(A::NextAgent, 120);
        assert_eq!(
            local_folds(&app),
            std::slice::from_ref(&space),
            "{sort:?} headers={headers}: the fold stays"
        );
    }
}
