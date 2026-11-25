//! Keys while a worker popup is open: they type into the worker, the
//! prefix is a two-key chord that never reaches hc-tui's prefix bindings,
//! the lock and host text commits follow the popup, and previous/next
//! agent walk through the workers.
use super::endpoint_nav::pending_attach;
use super::image_paste::{core_id, operations, press_key};
use super::pane_cache::{drive_until, next_attach, serve, with_openers};
use super::worker_popup::{child, multi, opened, row_app, single, spaced};
use super::*;
use crate::app::float::{FloatEntry, FloatKey};
use crate::popup_size::PopupSize;
use hc_vt::{Action as KeyAction, Key, Mods};

fn key(app: &mut App, key: Key, mods: Mods, action: KeyAction) {
    press_key(app, term_key(key, mods, action));
}

fn ctrl_b(app: &mut App, action: KeyAction) {
    key(app, Key::Char('b'), Mods::CTRL, action);
}

fn typed(app: &App) -> usize {
    app.popup.as_ref().unwrap().pane.input_backlog().0
}

/// `Alt+Down`/`Alt+Up` as the agent navigation chords.
fn nav_live() -> crate::config::LiveKeybindConfig {
    let config: crate::config::Config = toml::from_str(
        "[keys]\nnext_agent = \"alt+down\"\nprevious_agent = \"alt+up\"\n\
         next_tab = \"alt+right\"\nprevious_tab = \"alt+left\"\n\n\
         [[keys.command]]\nkey = \"alt+f\"\ntype = \"popup\"\n\
         command = \"exec \\\"${SHELL:-sh}\\\"\"\n",
    )
    .unwrap();
    config.live_keybinds()
}

/// `single()` with the worker `t5` open, a counted action store and a
/// live `t3` whose pane would show any stray input.
fn open_app() -> (App, mpsc::Receiver<crate::testcore::FakeCore>) {
    let mut app = single();
    let rx = operations(&mut app);
    core_id(&mut app, "core-fake-1");
    app.live = nav_live();
    app.focused = Some("t3".into());
    app.refresh_projection();
    app.open_worker_popup("t5".into());
    (app, rx)
}

#[tokio::test]
async fn plain_keys_enter_and_escape_reach_the_worker() {
    let (mut app, _rx) = open_app();
    key(&mut app, Key::Char('a'), Mods::NONE, KeyAction::Press);
    assert_eq!(typed(&app), 1);
    key(&mut app, Key::Enter, Mods::NONE, KeyAction::Press);
    assert_eq!(typed(&app), 2);
    key(&mut app, Key::Escape, Mods::NONE, KeyAction::Press);
    assert_eq!(typed(&app), 3);
    assert_eq!(opened(&app), Some("t5"));
    assert_eq!(app.pane.input_backlog().0, 0, "nothing reaches the parent");
}

#[tokio::test]
async fn prefix_then_escape_closes_without_a_kill() {
    let (mut app, _rx) = open_app();
    ctrl_b(&mut app, KeyAction::Press);
    assert!(app.prefix.armed());
    assert_eq!(typed(&app), 0, "the prefix is held back from the worker");
    key(&mut app, Key::Escape, Mods::NONE, KeyAction::Press);
    assert!(app.popup.is_none());
    assert!(!app.prefix.armed());
    assert_eq!(app.pending_actions, 0);
}

#[tokio::test]
async fn prefix_then_enter_keeps_the_worker_and_closes() {
    let (mut app, _rx) = open_app();
    ctrl_b(&mut app, KeyAction::Press);
    key(&mut app, Key::Enter, Mods::NONE, KeyAction::Press);
    assert!(app.popup.is_none());
    assert!(!app.prefix.armed());
    assert_eq!(app.pending_actions, 1, "one PeekKeep, no kill");
}

#[tokio::test]
async fn prefix_then_prefix_sends_exactly_one_literal_prefix() {
    let (mut app, _rx) = open_app();
    ctrl_b(&mut app, KeyAction::Press);
    ctrl_b(&mut app, KeyAction::Press);
    assert_eq!(typed(&app), 1);
    assert!(!app.prefix.armed());
    assert_eq!(opened(&app), Some("t5"));
}

#[tokio::test]
async fn a_held_prefixs_key_repeat_sends_nothing_and_keeps_it_armed() {
    let (mut app, _rx) = open_app();
    ctrl_b(&mut app, KeyAction::Press);
    // The lease logic may re-route a repeat into the popup arm: it must
    // be neither a second prefix press nor program input.
    let repeat = term_key(Key::Char('b'), Mods::CTRL, KeyAction::Repeat);
    for _ in 0..2 {
        assert_eq!(app.route_key_press(&repeat, 120), None);
    }
    assert_eq!(typed(&app), 0);
    assert!(app.prefix.armed(), "a repeat is not a second press");
    assert_eq!(opened(&app), Some("t5"));
}

#[tokio::test]
async fn a_second_distinct_prefix_press_hands_its_release_to_the_worker() {
    let (mut app, _rx) = open_app();
    app.host_reports_key_releases = true;
    // No release between the two presses (a quick double tap).
    ctrl_b(&mut app, KeyAction::Press);
    ctrl_b(&mut app, KeyAction::Press);
    ctrl_b(&mut app, KeyAction::Release);
    assert_eq!(typed(&app), 2, "the literal prefix press and its release");
}

#[tokio::test]
async fn a_refused_keep_still_swallows_the_enter_release() {
    let mut app = multi();
    operations(&mut app);
    core_id(&mut app, "core-fake-1");
    pending_attach(&mut app);
    app.host_reports_key_releases = true;
    app.focused = Some("t1".into());
    app.refresh_projection();
    app.open_worker_popup("host-a/t5".into());
    ctrl_b(&mut app, KeyAction::Press);
    ctrl_b(&mut app, KeyAction::Release);
    key(&mut app, Key::Enter, Mods::NONE, KeyAction::Press);
    assert_eq!(opened(&app), Some("host-a/t5"), "the keep was refused");
    key(&mut app, Key::Enter, Mods::NONE, KeyAction::Release);
    assert_eq!(
        typed(&app),
        0,
        "neither the chord's press nor release typed"
    );
}

#[tokio::test]
async fn prefix_then_x_sends_nothing_and_runs_no_action() {
    let (mut app, _rx) = open_app();
    ctrl_b(&mut app, KeyAction::Press);
    key(&mut app, Key::Char('x'), Mods::NONE, KeyAction::Press);
    assert_eq!(typed(&app), 0);
    assert!(!app.prefix.armed());
    assert_eq!(app.pending_actions, 0, "no Close of the parent");
    assert_eq!(app.focused.as_deref(), Some("t3"));
    assert!(
        app.view
            .inventory
            .get("t3")
            .is_some_and(|t| t.state == "live")
    );
}

#[tokio::test]
async fn the_release_of_every_held_back_press_never_reaches_the_worker() {
    let (mut app, _rx) = open_app();
    app.host_reports_key_releases = true;
    ctrl_b(&mut app, KeyAction::Press);
    ctrl_b(&mut app, KeyAction::Release);
    key(&mut app, Key::Char('x'), Mods::NONE, KeyAction::Press);
    key(&mut app, Key::Char('x'), Mods::NONE, KeyAction::Release);
    assert_eq!(typed(&app), 0);
    // A typed key's release is the worker's own.
    key(&mut app, Key::Char('a'), Mods::NONE, KeyAction::Press);
    key(&mut app, Key::Char('a'), Mods::NONE, KeyAction::Release);
    assert_eq!(typed(&app), 2);
}

#[tokio::test]
async fn the_popup_closing_with_the_prefix_armed_leaves_nothing_armed() {
    let (mut app, _rx) = open_app();
    ctrl_b(&mut app, KeyAction::Press);
    assert!(app.prefix.armed());
    let mut done = app.view.inventory.get("t5").unwrap().clone();
    done.state = "finished".into();
    let mut terms: Vec<_> = app
        .view
        .inventory
        .terminals()
        .values()
        .filter(|t| t.address != "t5")
        .cloned()
        .collect();
    terms.push(done);
    let seq = app.view.inventory.seq() + 1;
    app.view.inventory_mut().replace(seq, terms);
    app.reconcile_published_view();
    assert!(app.popup.is_none());
    assert!(
        !app.prefix.armed(),
        "the parent never inherits an armed prefix"
    );
    key(&mut app, Key::Char('x'), Mods::NONE, KeyAction::Press);
    assert_eq!(app.pending_actions, 0, "x runs nothing");
}

#[tokio::test]
async fn popup_command_chords_go_to_the_worker() {
    let (mut app, _rx) = open_app();
    key(&mut app, Key::Char('f'), Mods::ALT, KeyAction::Press);
    assert_eq!(typed(&app), 1, "Alt+F types into the worker");
    assert_eq!(opened(&app), Some("t5"), "no command popup replaced it");
    assert!(app.popup_pending.is_none());
}

#[tokio::test]
async fn the_lock_sends_keys_to_the_worker_not_the_pane_underneath() {
    let (mut app, _rx) = open_app();
    app.keyboard_locked = true;
    key(&mut app, Key::Char('a'), Mods::NONE, KeyAction::Press);
    assert_eq!(typed(&app), 1);
    assert_eq!(app.pane.input_backlog().0, 0);
}

#[tokio::test]
async fn a_report_all_text_commit_goes_to_the_worker_not_the_pane_underneath() {
    let (mut app, _rx) = open_app();
    app.host_reports_all_keys = true;
    app.host_reports_key_releases = true;
    for ch in ['h', 'i'] {
        let mut k = term_key(Key::Char(ch), Mods::NONE, KeyAction::Press);
        k.text_commit = Some(ch.to_string());
        k.vt_bytes = ch.to_string().into_bytes();
        press_key(&mut app, k);
    }
    assert_eq!(typed(&app), 2);
    assert_eq!(app.pane.input_backlog().0, 0);
}

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

// ---- the shared popup rules ----------------------------------------------

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
async fn ctrl_g_types_into_the_worker_instead_of_locking() {
    let (mut app, _rx) = open_app();
    key(&mut app, Key::Char('g'), Mods::CTRL, KeyAction::Press);
    assert!(!app.keyboard_locked, "a shown popup takes the lock chord");
    assert_eq!(typed(&app), 1);
}

/// Under the lock the prefix is held back only to see whether `Esc` follows.
#[tokio::test]
async fn a_locked_keyboard_types_every_chord_but_the_exit_into_the_worker() {
    let (mut app, _rx) = open_app();
    app.keyboard_locked = true;
    ctrl_b(&mut app, KeyAction::Press);
    assert!(app.prefix.armed(), "held back, waiting for the next key");
    assert_eq!(typed(&app), 0);
    // A navigation chord is not a chord under the lock: the held prefix
    // and the key both go to the worker.
    key(&mut app, Key::ArrowDown, Mods::ALT, KeyAction::Press);
    assert!(!app.prefix.armed());
    assert_eq!(typed(&app), 2);
    assert_eq!(opened(&app), Some("t5"));
}

#[tokio::test]
async fn a_locked_prefix_then_escape_closes_the_popup_with_no_kill() {
    let (mut app, _rx) = open_app();
    app.keyboard_locked = true;
    ctrl_b(&mut app, KeyAction::Press);
    key(&mut app, Key::Escape, Mods::NONE, KeyAction::Press);
    assert!(app.popup.is_none());
    assert!(!app.prefix.armed());
    assert_eq!(app.pending_actions, 0);
    assert!(app.keyboard_locked, "closing does not unlock");
}

#[tokio::test]
async fn a_locked_prefix_then_prefix_sends_one_prefix_and_enter_does_not_keep() {
    let (mut app, _rx) = open_app();
    app.keyboard_locked = true;
    ctrl_b(&mut app, KeyAction::Press);
    ctrl_b(&mut app, KeyAction::Press);
    assert_eq!(typed(&app), 1);
    ctrl_b(&mut app, KeyAction::Press);
    key(&mut app, Key::Enter, Mods::NONE, KeyAction::Press);
    assert_eq!(typed(&app), 3, "prefix + Enter, never a keep");
    assert_eq!(opened(&app), Some("t5"));
    assert_eq!(app.pending_actions, 0);
}

#[tokio::test]
async fn a_locked_held_prefixs_release_follows_it_to_the_worker() {
    let (mut app, _rx) = open_app();
    app.keyboard_locked = true;
    app.host_reports_key_releases = true;
    // Released before the next key: the release is replayed behind the press.
    ctrl_b(&mut app, KeyAction::Press);
    ctrl_b(&mut app, KeyAction::Release);
    assert_eq!(typed(&app), 0, "nothing is sent while it is held back");
    key(&mut app, Key::Char('x'), Mods::NONE, KeyAction::Press);
    assert_eq!(typed(&app), 3, "prefix press, its release, then x");
    // Still down at the next key: the real release is forwarded later.
    ctrl_b(&mut app, KeyAction::Press);
    key(&mut app, Key::Char('y'), Mods::NONE, KeyAction::Press);
    assert_eq!(typed(&app), 5);
    ctrl_b(&mut app, KeyAction::Release);
    assert_eq!(typed(&app), 6);
}

#[tokio::test]
async fn a_locked_prefix_then_x_reaches_the_worker_in_order() {
    let mut app = single();
    app.focused = Some("t3".into());
    app.refresh_projection();
    let mut requests = with_openers(&mut app);
    app.open_worker_popup("t5".into());
    let (core, params) = next_attach(&mut app, &mut requests).await;
    let mut core = serve(core, &params, 1).await;
    core.send_stream(1, hc_proto::frame::Kind::Data, b"\x1b[?9001l$ ")
        .await;
    drive_until(&mut app, |app| {
        app.popup.as_ref().is_some_and(|p| p.pane.has_fresh_paint())
    })
    .await;
    app.keyboard_locked = true;
    ctrl_b(&mut app, KeyAction::Press);
    key(&mut app, Key::Char('x'), Mods::NONE, KeyAction::Press);
    let mut sent = Vec::new();
    while sent.len() < 2 {
        let read = tokio::select! {
            frame = core.read_stream(1) => frame,
            event = app.pane_rx.recv() => {
                app.pane_event(event.expect("pane event"), Instant::now());
                continue;
            }
        };
        if read.kind == hc_proto::frame::Kind::Data {
            sent.extend_from_slice(&read.payload);
        }
    }
    assert_eq!(sent, b"\x02x", "the held prefix first, then x");
}

#[tokio::test]
async fn a_shown_float_under_the_lock_is_unchanged_by_the_worker_chord() {
    let mut app = single();
    app.focused = Some("t3".into());
    app.refresh_projection();
    let key_f = FloatKey {
        owner: app.focused_owner_key().unwrap(),
        command: "alt+f".into(),
    };
    app.show_float(key_f, "t8".into(), None, None, 8);
    let mut float = crate::sidebar::fixture::term("t8");
    float.created_ms = 8;
    float.labels.insert("hc-tui.popup".into(), "1".into());
    let mut terms: Vec<_> = app.view.inventory.terminals().values().cloned().collect();
    terms.push(float);
    let seq = app.view.inventory.seq() + 1;
    app.view.inventory_mut().replace(seq, terms);
    app.reconcile_published_view();
    app.keyboard_locked = true;
    ctrl_b(&mut app, KeyAction::Press);
    assert!(!app.prefix.armed(), "a float has no prefix chord");
    assert_eq!(typed(&app), 1, "the prefix types into the float");
    key(&mut app, Key::Escape, Mods::NONE, KeyAction::Press);
    assert!(app.popup.is_some(), "Esc never closes a float");
    assert_eq!(typed(&app), 2);
}
