//! Space folds and worker popups: with `nest_worker_terminals` on, a press
//! on a space header goes through any open popup, and agent navigation onto
//! a worker in a folded space unfolds it and opens the worker.
use super::agent_fixtures::{header, local_folds, workspace_of};
use super::worker_popup::{multi, opened, press, single};
use super::*;
use crate::chrome::snapshot::ClientEndpointId;
use crate::input::KeybindAction as A;

fn single_focused() -> App {
    let mut app = single();
    app.focused = Some("t3".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    app
}

#[tokio::test]
async fn a_header_press_dismisses_every_kind_of_open_popup_then_toggles() {
    for kind in ["worker", "peek"] {
        let mut app = single_focused();
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
    let mut app = single_focused();
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

#[tokio::test]
async fn a_remote_header_press_dismisses_a_worker_popup_then_toggles() {
    let mut app = multi();
    app.focused = Some("t1".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    app.open_worker_popup("host-a/t5".into());
    draw_to(&mut app, 120, 40);
    let (rect, endpoint, space) = app.hit_map.extra.endpoint_agent_space_headers[1].clone();
    press(&mut app, rect);
    assert!(app.popup.is_none());
    assert!(app.shell_state.remote_collapsed_agent_spaces[&endpoint].contains(&space));
}

#[tokio::test]
async fn next_agent_onto_a_worker_in_a_folded_space_unfolds_and_opens_it() {
    let mut app = single_focused();
    let space = workspace_of(&app, "S");
    app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space);
    app.on_action(A::NextAgent, 120);
    assert!(local_folds(&app).is_empty());
    assert_eq!(opened(&app), Some("t5"));
}

#[tokio::test]
async fn opening_a_worker_leaves_a_saved_fold_alone_unless_the_panel_shows_headers() {
    use crate::chrome::render::ClientMobileTarget;
    use crate::config::AgentPanelSortConfig::{Priority, Spaces};
    for (sort, headers) in [(Spaces, false), (Priority, true), (Priority, false)] {
        let mut app = single_focused();
        app.shell.agent_panel_sort = sort;
        app.shell.agent_panel_space_headers = headers;
        let space = workspace_of(&app, "S");
        app.toggle_collapsed_agent_space(&ClientEndpointId::Local, space.clone());
        let mut opened_by_index = false;
        for index in 0..3 {
            app.on_action(A::FocusAgent(index), 120);
            opened_by_index |= opened(&app) == Some("t5");
        }
        assert!(opened_by_index, "{sort:?} headers={headers}: t5 opened");
        app.close_popup();
        app.mode = Mode::Navigate;
        app.on_mobile_target(&ClientMobileTarget::Agent {
            endpoint_id: ClientEndpointId::Local,
            pane_id: "t5".into(),
        });
        assert_eq!(opened(&app), Some("t5"), "the switcher opens the worker");
        assert_eq!(
            local_folds(&app),
            std::slice::from_ref(&space),
            "{sort:?} headers={headers}: the fold stays"
        );
    }
}

#[tokio::test]
async fn the_endpoint_walk_onto_a_remote_worker_leaves_a_saved_fold_alone_without_headers() {
    let mut app = multi();
    app.focused = Some("host-a/t3".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    let remote = ClientEndpointId::Remote("host-a".into());
    let (_, _, space) = app.hit_map.extra.endpoint_agent_space_headers[1].clone();
    app.toggle_collapsed_agent_space(&remote, space);
    app.shell.agent_panel_space_headers = false;
    app.refresh_projection();
    app.on_action(A::NextAgent, 120);
    assert_eq!(
        opened(&app),
        Some("host-a/t5"),
        "the walk reached the worker"
    );
    assert_eq!(
        app.shell_state.remote_collapsed_agent_spaces[&remote].len(),
        1,
        "the fold stays"
    );
}
