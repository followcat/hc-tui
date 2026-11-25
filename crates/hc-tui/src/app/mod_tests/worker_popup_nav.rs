//! Walking worker popups from the keyboard: previous/next agent open the
//! next worker or close the popup and focus the agent, across machines, and
//! a tab chord closes a worker popup whose tab lost focus.
use super::endpoint_nav::pending_attach;
use super::image_paste::{core_id, operations};
use super::worker_popup::{child, opened, row_app, spaced};
use super::worker_popup_keys::{key, nav_live};
use super::*;
use crate::app::float::{FloatEntry, FloatKey};
use crate::popup_size::PopupSize;
use hc_vt::{Action as KeyAction, Key, Mods};

// ---- the keyboard walk ------------------------------------------------

/// Agents `[t3, t5, t7, t6, t9]`: t3, t5, t7, t6 in one tab, t9 in another.
fn walk_app() -> App {
    let mut app = row_app(
        vec![
            spaced("t3", "S", 0),
            child("t5", "t3", 1),
            child("t6", "t3", 2),
            child("t7", "t5", 3),
            spaced("t9", "U", 4),
        ],
        vec![],
    );
    operations(&mut app);
    core_id(&mut app, "core-fake-1");
    app.live = nav_live();
    app.focused = Some("t3".into());
    app.refresh_projection();
    app
}

fn next(app: &mut App) {
    key(app, Key::ArrowDown, Mods::ALT, KeyAction::Press);
}

fn previous(app: &mut App) {
    key(app, Key::ArrowUp, Mods::ALT, KeyAction::Press);
}

#[tokio::test]
async fn next_agent_walks_every_worker_then_closes_and_focuses_the_next_agent() {
    let mut app = walk_app();
    assert_eq!(
        crate::chrome::agents::ordered_agent_pane_ids(
            &app.projection.snapshot,
            app.shell.agent_panel_sort
        ),
        ["t3", "t5", "t7", "t6", "t9"]
    );
    app.on_action(crate::input::KeybindAction::NextAgent, 120);
    assert_eq!(opened(&app), Some("t5"));
    next(&mut app);
    assert_eq!(opened(&app), Some("t7"));
    next(&mut app);
    assert_eq!(opened(&app), Some("t6"));
    let before = app.pending_actions;
    next(&mut app);
    assert!(app.popup.is_none(), "the popup never stays over t9");
    assert_eq!(app.focused.as_deref(), Some("t9"));
    // The one action is the saved focus move to t9; a worker kill would
    // make it two.
    assert_eq!(app.pending_actions, before + 1, "detach only");
}

#[tokio::test]
async fn previous_agent_closes_the_popup_and_focuses_the_parent_then_moves_on() {
    let mut app = walk_app();
    app.open_worker_popup("t5".into());
    previous(&mut app);
    assert!(app.popup.is_none());
    assert_eq!(app.focused.as_deref(), Some("t3"));
    assert_eq!(app.pending_actions, 0);
    previous(&mut app);
    assert_eq!(
        app.focused.as_deref(),
        Some("t9"),
        "a second press passes t3"
    );
}

#[tokio::test]
async fn the_same_walk_holds_across_machines() {
    let mut app = row_app(
        vec![
            spaced("t1", "one", 0),
            spaced("host-a/t3", "S", 0),
            child("host-a/t5", "host-a/t3", 1),
            child("host-a/t6", "host-a/t3", 2),
            child("host-a/t7", "host-a/t5", 3),
            spaced("host-a/t9", "U", 4),
        ],
        vec![crate::sidebar::fixture::machine("host-a", "connected")],
    );
    operations(&mut app);
    core_id(&mut app, "core-fake-1");
    pending_attach(&mut app);
    app.live = nav_live();
    app.focused = Some("host-a/t3".into());
    app.refresh_projection();
    app.on_action(crate::input::KeybindAction::NextAgent, 120);
    assert_eq!(opened(&app), Some("host-a/t5"));
    next(&mut app);
    assert_eq!(opened(&app), Some("host-a/t7"));
    next(&mut app);
    assert_eq!(opened(&app), Some("host-a/t6"));
    next(&mut app);
    assert!(app.popup.is_none());
    assert_eq!(app.focused.as_deref(), Some("host-a/t9"));
    app.open_worker_popup("host-a/t5".into());
    previous(&mut app);
    assert!(app.popup.is_none());
    assert_eq!(app.focused.as_deref(), Some("host-a/t3"));
}

// ---- the owner-tab rule on a tab chord -----------------------------------

/// Space `S` with tabs one (`t3`, worker `t5`) and two (`t4`), and a hidden
/// float of tab two waiting to come back. Returns the app on tab one with
/// the worker popup open when `with_popup`, and the actions already queued.
fn two_tab_app(with_popup: bool) -> (App, FloatKey) {
    let mut tab_two = spaced("t4", "S", 2);
    tab_two.labels.insert("herdr.tab".into(), "two".into());
    let mut app = row_app(
        vec![spaced("t3", "S", 0), child("t5", "t3", 1), tab_two],
        vec![],
    );
    operations(&mut app);
    core_id(&mut app, "core-fake-1");
    app.live = nav_live();
    app.focused = Some("t4".into());
    app.refresh_projection();
    let key = FloatKey {
        owner: app.focused_owner_key().expect("a focused tab"),
        command: "alt+f".into(),
    };
    app.hide_float(
        key.clone(),
        FloatEntry {
            terminal: "t8".into(),
            width: Some(PopupSize::Percent(80)),
            height: Some(PopupSize::Percent(80)),
            restore: true,
            created_ms: 8,
            seen: false,
            kill_sent: false,
        },
    );
    app.focused = Some("t3".into());
    app.refresh_projection();
    if with_popup {
        app.open_worker_popup("t5".into());
    }
    (app, key)
}

#[tokio::test]
async fn a_tab_chord_closes_the_worker_popup_at_once_and_the_new_tabs_float_returns() {
    let (mut control, _) = two_tab_app(false);
    key(&mut control, Key::ArrowRight, Mods::ALT, KeyAction::Press);
    let control_actions = control.pending_actions;

    let (mut app, key_two) = two_tab_app(true);
    assert_eq!(opened(&app), Some("t5"));
    key(&mut app, Key::ArrowRight, Mods::ALT, KeyAction::Press);
    // No publish ran: only the tab chord's own refresh did the work.
    assert_eq!(app.focused.as_deref(), Some("t4"));
    assert!(opened(&app).is_none(), "the worker popup left with its tab");
    let popup = app.popup.as_ref().expect("the new tab's float is shown");
    assert!(popup.persistent && popup.terminal == "t8");
    assert_eq!(popup.key.as_ref(), Some(&key_two));
    assert_eq!(
        app.pending_actions, control_actions,
        "closing the popup added no action, so no Kill"
    );
}

#[tokio::test]
async fn the_worker_popup_stays_while_its_tab_stays_focused() {
    let (mut app, _) = two_tab_app(true);
    for _ in 0..3 {
        app.refresh_projection();
        app.share_seen();
        app.mark_active_seen();
    }
    assert_eq!(opened(&app), Some("t5"));
}
