//! Opening the worker popup from a row, replacing other popups, and the
//! rule that closing it never kills the worker.
use super::image_paste::{core_id, operations};
use super::worker_popup::{multi, opened, press, single};
use super::*;
use crate::app::float::FloatKey;
use crate::config::{CustomCommandAction, CustomCommandKeybind};
use crate::input::KeybindAction as A;
use crate::popup_size::PopupSize;
use hc_proto::terminal::Terminal;

/// An app grouped by space with a worker `t5` under `t3`, focused on `t3`,
/// whose submitted actions are counted (`pending_actions`).
fn counting() -> (App, mpsc::Receiver<crate::testcore::FakeCore>) {
    let mut app = single();
    let rx = operations(&mut app);
    core_id(&mut app, "core-fake-1");
    app.focused = Some("t3".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    (app, rx)
}

fn row(app: &App, id: &str) -> ratatui::layout::Rect {
    app.hit_map
        .agents
        .iter()
        .find(|(_, row)| row == id)
        .unwrap_or_else(|| panic!("no row for {id}"))
        .0
}

fn click_row(app: &mut App, id: &str) {
    let rect = row(app, id);
    press(app, rect);
}

fn command(persistent: bool) -> CustomCommandKeybind {
    CustomCommandKeybind {
        bindings: Default::default(),
        label: "alt+f".into(),
        command: "exec \"${SHELL:-sh}\"".into(),
        action: CustomCommandAction::Popup,
        description: None,
        width: Some(PopupSize::Percent(80)),
        height: Some(PopupSize::Percent(80)),
        persistent,
    }
}

/// The inventory with `extra` records added to the fixture's terminals.
fn publish_with(app: &mut App, extra: Vec<Terminal>) {
    let mut terms = app
        .view
        .inventory
        .terminals()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    terms.extend(extra);
    let seq = app.view.inventory.seq() + 1;
    app.view.inventory_mut().replace(seq, terms);
    app.reconcile_published_view();
}

/// Show a persistent float on the focused tab, as a `Ran` answer would.
fn show_float(app: &mut App) -> FloatKey {
    let key = FloatKey {
        owner: app.focused_owner_key().expect("a focused tab"),
        command: "alt+f".into(),
    };
    app.show_float(
        key.clone(),
        "t8".into(),
        Some(PopupSize::Percent(80)),
        Some(PopupSize::Percent(80)),
        8,
    );
    let mut float = crate::sidebar::fixture::term("t8");
    float.created_ms = 8;
    float.labels.insert("hc-tui.popup".into(), "1".into());
    publish_with(app, vec![float]);
    key
}

#[tokio::test]
async fn a_click_opens_an_active_worker_popup_over_the_current_tab() {
    let (mut app, _rx) = counting();
    click_row(&mut app, "t5");
    let popup = app.popup.as_ref().expect("the worker popup is open");
    assert_eq!(popup.terminal, "t5");
    assert_eq!(popup.worker.as_ref().unwrap().parent, "t3");
    assert!(!popup.persistent && popup.owner_tab.is_some());
    assert_eq!(
        popup.pane.spec().map(|spec| spec.role),
        Some(pane::Role::Active)
    );
    assert_eq!(app.focused.as_deref(), Some("t3"));
    assert!(app.peek.is_none());
}

#[tokio::test]
async fn next_agent_onto_a_worker_parks_a_float_and_opens_the_popup() {
    let (mut app, _rx) = counting();
    let key = show_float(&mut app);
    assert!(app.popup.as_ref().is_some_and(|p| p.persistent));
    let before = app.pending_actions;
    app.on_action(A::NextAgent, 120);
    assert_eq!(opened(&app), Some("t5"));
    assert!(
        app.floats.get(&key).is_some_and(|entry| entry.restore),
        "the float parked and returns with its tab"
    );
    assert_eq!(app.pending_actions, before, "parking is no kill");
}

#[tokio::test]
async fn a_row_press_closes_a_command_popup_and_opens_the_worker_in_one_press() {
    let (mut app, _rx) = counting();
    app.run_custom_command(&command(false));
    app.on_popup_spawned("t8".into(), 8);
    let mut popup_term = crate::sidebar::fixture::term("t8");
    popup_term.created_ms = 8;
    popup_term.labels.insert("hc-tui.popup".into(), "1".into());
    publish_with(&mut app, vec![popup_term]);
    assert_eq!(app.popup.as_ref().unwrap().terminal, "t8");
    draw_to(&mut app, 120, 40);
    let before = app.pending_actions;
    click_row(&mut app, "t5");
    assert_eq!(opened(&app), Some("t5"));
    assert_eq!(
        app.pending_actions,
        before + 1,
        "the command popup's terminal is killed"
    );
}

#[tokio::test]
async fn a_row_press_parks_a_shown_float_and_opens_the_worker() {
    let (mut app, _rx) = counting();
    let key = show_float(&mut app);
    draw_to(&mut app, 120, 40);
    let before = app.pending_actions;
    click_row(&mut app, "t5");
    assert_eq!(opened(&app), Some("t5"));
    assert!(app.floats.contains_key(&key));
    assert_eq!(app.pending_actions, before, "no kill for a parked float");
}

#[tokio::test]
async fn a_press_on_another_row_closes_the_worker_popup_and_focuses_it() {
    let (mut app, _rx) = counting();
    app.open_worker_popup("t5".into());
    draw_to(&mut app, 120, 40);
    let before = app.pending_actions;
    click_row(&mut app, "t9");
    assert!(app.popup.is_none());
    assert_eq!(app.focused.as_deref(), Some("t9"));
    assert_eq!(app.pending_actions, before, "detach only");
}

#[tokio::test]
async fn an_outside_press_on_plain_pane_area_still_swallows_for_a_float() {
    let (mut app, _rx) = counting();
    show_float(&mut app);
    draw_to(&mut app, 120, 40);
    let area = app.frame_area();
    press(&mut app, ratatui::layout::Rect::new(area.x, area.y, 1, 1));
    assert!(app.popup.as_ref().is_some_and(|p| p.persistent));
    assert_eq!(app.focused.as_deref(), Some("t3"));
}

#[tokio::test]
async fn an_outside_press_on_plain_pane_area_closes_a_worker_popup() {
    let (mut app, _rx) = counting();
    app.open_worker_popup("t5".into());
    draw_to(&mut app, 120, 40);
    let area = app.frame_area();
    let before = app.pending_actions;
    press(&mut app, ratatui::layout::Rect::new(area.x, area.y, 1, 1));
    assert!(app.popup.is_none());
    assert_eq!(app.focused.as_deref(), Some("t3"));
    assert_eq!(app.pending_actions, before);
}

#[tokio::test]
async fn a_worker_that_is_gone_shows_a_notice_and_opens_nothing() {
    let (mut app, _rx) = counting();
    app.open_worker_popup("t77".into());
    assert!(app.popup.is_none());
    assert_eq!(app.notice.as_ref().unwrap().title, "Worker unavailable");
}

// ---- never kills -----------------------------------------------------

/// Open `t5`, run `close`, and assert no action (so no `terminal.kill`) was
/// submitted and the popup is gone.
fn never_kills(close: impl FnOnce(&mut App)) {
    let (mut app, _rx) = counting();
    app.open_worker_popup("t5".into());
    draw_to(&mut app, 120, 40);
    let before = app.pending_actions;
    close(&mut app);
    assert!(opened(&app) != Some("t5"), "the worker popup is still open");
    assert_eq!(app.pending_actions, before, "an action was submitted");
    assert!(
        app.notice.is_none(),
        "{:?}",
        app.notice.as_ref().map(|n| &n.body)
    );
}

#[tokio::test]
async fn closing_by_the_x_button_sends_no_kill() {
    never_kills(|app| {
        let close = app.hit_map.extra.popup_buttons.unwrap().close;
        press(app, close);
    });
}

#[tokio::test]
async fn closing_by_an_outside_press_sends_no_kill() {
    never_kills(|app| {
        let area = app.frame_area();
        press(app, ratatui::layout::Rect::new(area.x, area.y, 1, 1));
    });
}

#[tokio::test]
async fn a_focused_tab_change_closes_it_with_no_kill() {
    never_kills(|app| {
        app.focused = Some("t9".into());
        app.refresh_projection();
        app.reconcile_published_view();
    });
}

#[tokio::test]
async fn a_command_popup_opening_over_it_sends_no_kill_for_the_worker() {
    let (mut app, _rx) = counting();
    app.open_worker_popup("t5".into());
    let before = app.pending_actions;
    app.run_custom_command(&command(false));
    assert!(app.popup.is_none(), "the worker popup closed");
    // Only the command popup's own `terminal.run` was submitted.
    assert_eq!(app.pending_actions, before + 1);
}

#[tokio::test]
async fn the_worker_finishing_closes_it_with_no_kill() {
    never_kills(|app| {
        let mut done = app.view.inventory.get("t5").unwrap().clone();
        done.state = "finished".into();
        let mut terms = app
            .view
            .inventory
            .terminals()
            .values()
            .filter(|t| t.address != "t5")
            .cloned()
            .collect::<Vec<_>>();
        terms.push(done);
        let seq = app.view.inventory.seq() + 1;
        app.view.inventory_mut().replace(seq, terms);
        app.reconcile_published_view();
    });
}

#[tokio::test]
async fn a_remote_worker_opens_from_its_row_too() {
    let mut app = multi();
    app.focused = Some("t1".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    let rect = app
        .hit_map
        .endpoint_agents
        .iter()
        .find(|(_, _, id)| id == "host-a/t5")
        .unwrap()
        .0;
    press(&mut app, rect);
    assert_eq!(opened(&app), Some("host-a/t5"));
    assert_eq!(
        app.popup.as_ref().unwrap().worker.as_ref().unwrap().parent,
        "host-a/t3"
    );
}

#[tokio::test]
async fn a_row_press_closes_a_peek_and_acts_on_the_row() {
    let (mut app, _rx) = counting();
    app.open_peek("t5".into());
    draw_to(&mut app, 120, 40);
    click_row(&mut app, "t9");
    assert!(app.peek.is_none());
    assert_eq!(app.focused.as_deref(), Some("t9"));
}
