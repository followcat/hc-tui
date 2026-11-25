//! `[[keys.command]]` commands learn which terminal was focused:
//! `HC_FOCUSED_TERMINAL`, `HC_FOCUSED_MACHINE`, `HC_FOCUSED_SPACE` and
//! `HC_FOCUSED_TAB`.

use super::floats::{core_id, run_reply, settle};
use super::m4s2::{operations, pending};
use super::*;
use crate::app::creation::{export_prefix, focused_env};
use crate::config::{CustomCommandAction, CustomCommandKeybind};
use serde_json::json;
use std::collections::BTreeMap;

fn command(action: CustomCommandAction, line: &str) -> CustomCommandKeybind {
    CustomCommandKeybind {
        bindings: Default::default(),
        label: "alt+h".into(),
        command: line.into(),
        action,
        description: None,
        width: None,
        height: None,
        persistent: false,
        local: false,
    }
}

fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect()
}

#[test]
fn a_local_terminal_gives_its_id_and_an_empty_machine() {
    let env = focused_env(
        Some("t7"),
        Some(&labels(&[("herdr.space", "work"), ("herdr.tab", "main")])),
    );
    assert_eq!(env["HC_FOCUSED_TERMINAL"], "t7");
    assert_eq!(env["HC_FOCUSED_MACHINE"], "");
    assert_eq!(env["HC_FOCUSED_SPACE"], "work");
    assert_eq!(env["HC_FOCUSED_TAB"], "main");
    // Never the variables of the command's own terminal.
    assert!(!env.contains_key("HC_TERMINAL_ID"));
    assert!(!env.contains_key("HC_TERMINAL_SOCKET"));
}

#[test]
fn a_remote_terminal_is_stripped_to_the_id_valid_on_its_machine() {
    let env = focused_env(Some("host-a/t3"), None);
    assert_eq!(env["HC_FOCUSED_TERMINAL"], "t3");
    assert_eq!(env["HC_FOCUSED_MACHINE"], "host-a");
    assert!(!env.contains_key("HC_FOCUSED_SPACE"));
    assert!(!env.contains_key("HC_FOCUSED_TAB"));
}

#[test]
fn no_focused_terminal_sets_nothing_and_exports_nothing() {
    assert!(focused_env(None, None).is_empty());
    assert_eq!(export_prefix(&BTreeMap::new()), "");
}

#[test]
fn the_export_prefix_quotes_values() {
    let env = labels(&[("HC_FOCUSED_SPACE", "it's mine")]);
    assert_eq!(
        export_prefix(&env),
        "export HC_FOCUSED_SPACE='it'\\''s mine'; "
    );
}

fn remote_app() -> App {
    let mut app = super::endpoint_nav::two_machine_app();
    app.focused = Some("host-a/t3".into());
    app.refresh_projection();
    app
}

async fn popup_params(app: &mut App) -> serde_json::Value {
    pending(app);
    app.reconcile_published_view();
    let mut rx = operations(app);
    core_id(app, "core-fake-1");
    let server = tokio::spawn(async move {
        let mut core = rx.recv().await.unwrap();
        core.read_hello().await;
        core.answer_hello(true, &["terminal.run"], &[]).await;
        let run = core.read_request().await;
        assert_eq!(run["method"], "terminal.run");
        let params = run["params"].clone();
        core.reply(&run["id"], run_reply("t9", 9)).await;
        params
    });
    app.run_custom_command(&command(CustomCommandAction::Popup, "claude-hint"));
    settle(app).await;
    server.await.unwrap()
}

#[tokio::test]
async fn a_popup_on_a_local_terminal_gets_the_local_variables() {
    let mut app = app_with_spaces(&[("t1", "work")]);
    let params = popup_params(&mut app).await;
    let env = &params["env"];
    assert_eq!(env["HERDR_ENV"], "1");
    assert_eq!(env["HC_FOCUSED_TERMINAL"], "t1");
    assert_eq!(env["HC_FOCUSED_MACHINE"], "");
    assert_eq!(env["HC_FOCUSED_SPACE"], "work");
    assert!(params.get("on").is_none());
    assert!(env.get("HC_TERMINAL_ID").is_none());
}

#[tokio::test]
async fn a_popup_on_a_remote_terminal_runs_there_with_its_local_id() {
    let mut app = remote_app();
    let params = popup_params(&mut app).await;
    assert_eq!(params["on"], "host-a");
    assert_eq!(params["env"]["HC_FOCUSED_TERMINAL"], "t3");
    assert_eq!(params["env"]["HC_FOCUSED_MACHINE"], "host-a");
}

#[tokio::test]
async fn a_shell_command_exports_them_before_its_line() {
    let mut app = remote_app();
    pending(&mut app);
    app.reconcile_published_view();
    let mut rx = operations(&mut app);
    core_id(&mut app, "core-fake-1");
    let server = tokio::spawn(async move {
        let mut core = rx.recv().await.unwrap();
        core.read_hello().await;
        core.answer_hello(true, &["machine.exec"], &[]).await;
        let exec = core.read_request().await;
        assert_eq!(exec["params"]["on"], "host-a");
        let line = exec["params"]["line"].as_str().unwrap().to_owned();
        core.reply(&exec["id"], json!({"output": ""})).await;
        line
    });
    app.run_custom_command(&command(CustomCommandAction::Shell, "echo hi"));
    settle(&mut app).await;
    let line = server.await.unwrap();
    assert!(line.starts_with("export "), "{line}");
    assert!(line.contains("HC_FOCUSED_TERMINAL='t3'"), "{line}");
    assert!(line.contains("HC_FOCUSED_MACHINE='host-a'"), "{line}");
    assert!(line.ends_with("; echo hi"), "{line}");
}

/// The `machine.exec` params a shell command sends while a remote pane
/// (`host-a/t3`) is focused.
async fn shell_exec_params(command: CustomCommandKeybind) -> serde_json::Value {
    let mut app = remote_app();
    pending(&mut app);
    app.reconcile_published_view();
    let mut rx = operations(&mut app);
    core_id(&mut app, "core-fake-1");
    let server = tokio::spawn(async move {
        let mut core = rx.recv().await.unwrap();
        core.read_hello().await;
        core.answer_hello(true, &["machine.exec"], &[]).await;
        let exec = core.read_request().await;
        assert_eq!(exec["method"], "machine.exec");
        let params = exec["params"].clone();
        core.reply(&exec["id"], json!({"output": ""})).await;
        params
    });
    app.run_custom_command(&command);
    settle(&mut app).await;
    server.await.unwrap()
}

#[tokio::test]
async fn machine_local_runs_here_but_keeps_the_focused_variables() {
    let params = shell_exec_params(CustomCommandKeybind {
        local: true,
        ..command(CustomCommandAction::Shell, "echo hi")
    })
    .await;
    assert!(params["on"].is_null(), "{params}");
    let line = params["line"].as_str().unwrap();
    assert!(line.contains("HC_FOCUSED_TERMINAL='t3'"), "{line}");
    assert!(line.contains("HC_FOCUSED_MACHINE='host-a'"), "{line}");
    assert!(line.ends_with("; echo hi"), "{line}");
}

#[tokio::test]
async fn the_default_still_runs_on_the_focused_machine() {
    let params = shell_exec_params(command(CustomCommandAction::Shell, "echo hi")).await;
    assert_eq!(params["on"], "host-a");
}
