//! Moving around worker popups from the keyboard: previous/next agent open
//! the next worker or close the popup and focus the agent, across machines,
//! a tab chord closes a worker popup whose tab lost focus, and page keys
//! page its scrollback.
use super::endpoint_nav::pending_attach;
use super::image_paste::{core_id, operations};
use super::worker_popup::{child, opened, row_app, spaced};
use super::worker_popup_keys::{ctrl_b, key, nav_live, open_app, typed};
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

// ---- paging the popup's own scrollback ------------------------------------

/// Five content rows plus two pages of history, on the normal screen.
fn scrollback_vt() -> hc_vt::Vt {
    let mut vt = hc_vt::Vt::new(40, 5, 8192);
    for i in 1..=20 {
        vt.feed(format!("line-{i:02}\r\n").as_bytes());
    }
    vt
}

fn scrolled_back(app: &App) -> u64 {
    let bar = app
        .popup
        .as_ref()
        .and_then(|popup| popup.pane.vt_ref())
        .and_then(|vt| vt.scrollbar())
        .expect("scrollbar state");
    bar.total - bar.offset - bar.len
}

#[tokio::test]
async fn plain_page_keys_page_the_worker_popups_own_scrollback() {
    let (mut app, _rx) = open_app();
    app.popup.as_mut().unwrap().pane.install_vt(scrollback_vt());
    key(&mut app, Key::PageUp, Mods::NONE, KeyAction::Press);
    assert_eq!(scrolled_back(&app), 5, "one popup height");
    assert_eq!(typed(&app), 0, "a paged key never reaches the worker");
    assert_eq!(app.pane.input_backlog().0, 0, "the pane behind never moves");
    key(&mut app, Key::Char('x'), Mods::NONE, KeyAction::Press);
    assert_eq!(typed(&app), 1, "other keys still type");
    assert_eq!(scrolled_back(&app), 0, "and jump the view to the bottom");
}

#[tokio::test]
async fn a_locked_held_prefix_and_the_key_after_it_jump_a_paged_back_view_to_the_bottom() {
    // `Pane::key` follows the tail again on any input to an active attach,
    // so the held prefix and the key after it need no extra scroll.
    let (mut app, _rx) = open_app();
    app.keyboard_locked = true;
    app.popup.as_mut().unwrap().pane.install_vt(scrollback_vt());
    key(&mut app, Key::PageUp, Mods::NONE, KeyAction::Press);
    assert_eq!(scrolled_back(&app), 5, "paged back");
    ctrl_b(&mut app, KeyAction::Press);
    assert_eq!(
        scrolled_back(&app),
        5,
        "a held prefix has not reached the worker"
    );
    key(&mut app, Key::Char('x'), Mods::NONE, KeyAction::Press);
    assert_eq!(typed(&app), 2, "prefix and x");
    assert_eq!(
        scrolled_back(&app),
        0,
        "the worker got keys, so the view is at the bottom"
    );
}

#[tokio::test]
async fn a_literal_prefix_jumps_a_paged_back_view_to_the_bottom() {
    // Same rule, through the literal-prefix arm.
    let (mut app, _rx) = open_app();
    app.popup.as_mut().unwrap().pane.install_vt(scrollback_vt());
    key(&mut app, Key::PageUp, Mods::NONE, KeyAction::Press);
    assert_eq!(scrolled_back(&app), 5);
    ctrl_b(&mut app, KeyAction::Press);
    ctrl_b(&mut app, KeyAction::Press);
    assert_eq!(typed(&app), 1);
    assert_eq!(scrolled_back(&app), 0);
}
