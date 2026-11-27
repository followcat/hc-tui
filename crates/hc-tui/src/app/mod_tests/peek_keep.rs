//! Peek's `↗ keep`: refused for a terminal on another machine than the
//! focused one, since keeping labels it with the focused machine's space.
use super::endpoint_nav::{app_multi, pending_attach, spaced_agent};
use super::image_paste::operations;
use super::*;
use crate::sidebar::fixture::machine;

/// A local terminal `t1`, terminals of `host-a` and one of `host-b`, each
/// machine with its own space.
fn local_and_two_remotes() -> App {
    let mut app = app_multi(
        vec![
            spaced_agent("t1", "one"),
            spaced_agent("host-a/t1", "S"),
            spaced_agent("host-a/t3", "S"),
            spaced_agent("host-a/t5", "S"),
            spaced_agent("host-b/t1", "T"),
        ],
        vec![
            machine("host-a", "connected"),
            machine("host-b", "connected"),
        ],
    );
    app.area = (120, 40);
    pending_attach(&mut app);
    app
}

/// Focus `focused`, peek `peeked` and press Enter, the peek bar's `↗ keep`.
fn keep_peek_of(app: &mut App, focused: &str, peeked: &str) {
    app.focused = Some(focused.into());
    app.refresh_projection();
    app.open_peek(peeked.into());
    assert!(app.peek.is_some(), "the peek opened");
    let enter = term_key(hc_vt::Key::Enter, hc_vt::Mods::NONE, hc_vt::Action::Press);
    assert!(app.peek_key(&enter));
}

/// Keeping `peeked` while `focused` has focus is refused: the peek closes,
/// nothing is submitted and the notice is on screen.
fn assert_refused(focused: &str, peeked: &str) {
    let mut app = local_and_two_remotes();
    let _core = operations(&mut app);
    keep_peek_of(&mut app, focused, peeked);
    assert!(app.peek.is_none(), "the peek closes, as for any keep");
    assert_eq!(app.pending_actions, 0, "no PeekKeep was submitted");
    let screen = screen_lines(&draw_to(&mut app, 120, 40));
    assert!(
        screen.iter().any(|line| line.contains("Keep unavailable")),
        "the notice title is on screen: {screen:#?}"
    );
    assert!(
        screen
            .iter()
            .any(|line| line.contains("Keep it from a tab on that terminal's machine")),
        "the notice body is on screen: {screen:#?}"
    );
}

#[tokio::test]
async fn peek_keep_of_a_terminal_on_another_machine_is_refused() {
    assert_refused("t1", "host-a/t1");
}

#[tokio::test]
async fn peek_keep_of_a_local_terminal_from_a_remote_focus_is_refused() {
    assert_refused("host-a/t3", "t1");
}

#[tokio::test]
async fn peek_keep_of_a_terminal_on_a_different_remote_machine_is_refused() {
    assert_refused("host-a/t3", "host-b/t1");
}

#[tokio::test]
async fn peek_keep_of_a_terminal_on_the_focused_remote_machine_keeps() {
    let mut app = local_and_two_remotes();
    let _core = operations(&mut app);
    keep_peek_of(&mut app, "host-a/t3", "host-a/t5");
    assert!(app.peek.is_none(), "keep dismisses the peek");
    assert_eq!(app.pending_actions, 1, "exactly one PeekKeep was submitted");
    let screen = screen_lines(&draw_to(&mut app, 120, 40));
    assert!(
        !screen.iter().any(|line| line.contains("Keep unavailable")),
        "no refusal notice: {screen:#?}"
    );
}
