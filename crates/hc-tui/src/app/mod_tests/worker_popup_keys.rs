//! Keys while a worker popup is open: they type into the worker, the
//! prefix is a two-key chord that never reaches hc-tui's prefix bindings,
//! and the lock and host text commits follow the popup. Walking workers
//! with the agent keys is in `worker_popup_nav.rs`.
use super::endpoint_nav::pending_attach;
use super::image_paste::{core_id, operations, press_key};
use super::pane_cache::{drive_until, next_attach, serve, with_openers};
use super::worker_popup::{multi, opened, press, single};
use super::*;
use crate::app::float::FloatKey;
use hc_vt::{Action as KeyAction, Key, Mods};

pub(super) fn key(app: &mut App, key: Key, mods: Mods, action: KeyAction) {
    press_key(app, term_key(key, mods, action));
}

pub(super) fn ctrl_b(app: &mut App, action: KeyAction) {
    key(app, Key::Char('b'), Mods::CTRL, action);
}

pub(super) fn typed(app: &App) -> usize {
    app.popup.as_ref().unwrap().pane.input_backlog().0
}

/// `Alt+Down`/`Alt+Up` as the agent navigation chords.
pub(super) fn nav_live() -> crate::config::LiveKeybindConfig {
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
pub(super) fn open_app() -> (App, mpsc::Receiver<crate::testcore::FakeCore>) {
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
async fn a_prefix_armed_in_the_pane_is_not_the_workers_chord() {
    let mut app = single();
    app.live = nav_live();
    app.focused = Some("t3".into());
    app.refresh_projection();
    draw_to(&mut app, 120, 40);
    ctrl_b(&mut app, KeyAction::Press);
    assert!(app.prefix.armed(), "the pane's prefix is armed");
    let rect = app
        .hit_map
        .agents
        .iter()
        .find(|(_, id)| id == "t5")
        .unwrap()
        .0;
    press(&mut app, rect);
    assert_eq!(opened(&app), Some("t5"));
    assert!(!app.prefix.armed(), "opening the worker disarms it");
    key(&mut app, Key::Char('x'), Mods::NONE, KeyAction::Press);
    assert_eq!(typed(&app), 1, "the first key reaches the worker");
    assert_eq!(opened(&app), Some("t5"), "and the popup stays open");
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
async fn a_locked_navigation_chord_types_into_the_worker() {
    let (mut app, _rx) = open_app();
    app.keyboard_locked = true;
    key(&mut app, Key::ArrowDown, Mods::ALT, KeyAction::Press);
    assert_eq!(typed(&app), 1, "the key reaches the worker");
    assert_eq!(opened(&app), Some("t5"), "the walk did not move on");
    assert_eq!(app.focused.as_deref(), Some("t3"), "focus is unchanged");
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
