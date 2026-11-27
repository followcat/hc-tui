//! A press on a space header while a float, a command popup or a peek is
//! open acts as it does on any other part of the sidebar: the popups
//! swallow it and the peek closes, and no fold changes.
use super::agent_fixtures::{header, local_folds, press, single, workspace_of};
use super::*;

/// A header press on `app` with nothing folded: the header's rect.
fn header_of_s(app: &App) -> ratatui::layout::Rect {
    let space = workspace_of(app, "S");
    header(app, &space)
}

#[tokio::test]
async fn a_header_press_with_a_shown_float_is_swallowed() {
    let mut app = single();
    let key = crate::app::float::FloatKey {
        owner: app.focused_owner_key().unwrap(),
        command: "alt+f".into(),
    };
    app.show_float(key.clone(), "t8".into(), None, None, 8);
    draw_to(&mut app, 120, 40);
    let rect = header_of_s(&app);
    press(&mut app, rect);
    assert!(app.popup.is_some(), "the float stays open");
    assert!(local_folds(&app).is_empty(), "no fold changed");
}

#[tokio::test]
async fn a_header_press_with_a_command_popup_is_swallowed() {
    let mut app = single();
    let mut pane = pane::Pane::new(app.pane_tx.clone(), app.open_attach.clone());
    pane.want(Some(pane::Spec {
        terminal: "t8".into(),
        role: pane::Role::Active,
    }));
    app.popup = Some(popup::PopupState {
        pane,
        terminal: "t8".into(),
        width: None,
        height: None,
        owner_tab: app.projection.snapshot.focused_tab_id.clone(),
        persistent: false,
        key: None,
        created_ms: 8,
        seen: false,
        worker: None,
    });
    draw_to(&mut app, 120, 40);
    let rect = header_of_s(&app);
    press(&mut app, rect);
    assert!(app.popup.is_some(), "the popup stays open");
    assert!(local_folds(&app).is_empty(), "no fold changed");
}

#[tokio::test]
async fn a_header_press_with_a_peek_open_closes_it_and_folds_nothing() {
    let mut app = single();
    app.open_peek("t9".into());
    draw_to(&mut app, 120, 40);
    let rect = header_of_s(&app);
    press(&mut app, rect);
    assert!(app.peek.is_none(), "an outside press closes the peek");
    assert!(local_folds(&app).is_empty(), "no fold changed");
}
