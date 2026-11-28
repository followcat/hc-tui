//! The machine menu's New workspace (#82): a workspace can be created on
//! any connected machine, including an empty one, instead of only on the
//! active machine. Connect is withheld where it would do nothing.

use super::endpoint_nav::{app_multi, spaced_term};
use super::*;
use crate::chrome::menu::ContextMenuTarget;
use crate::chrome::render::MachineHit;
use crate::sidebar::fixture::machine;
use crate::testcore;
use hc_vt::{Action as VtAction, Key, Mods};
use ratatui::layout::Rect;
use serde_json::json;

fn press(key: Key) -> input::TerminalKey {
    term_key(key, Mods::NONE, VtAction::Press)
}

fn pending(app: &mut App) {
    app.open_attach = Arc::new(|_| Box::pin(std::future::pending()));
    app.pane = pane::Pane::new(app.pane_tx.clone(), app.open_attach.clone());
}

fn operations(app: &mut App) -> mpsc::Receiver<testcore::FakeCore> {
    let (accepted, rx) = mpsc::channel(4);
    let (rpc, _sub) = testcore::openers(accepted);
    app.set_operations(rpc);
    app.view.core.as_mut().unwrap().id = "core-fake-1".into();
    rx
}

async fn settle(app: &mut App) {
    let result = tokio::time::timeout(
        testcore::DEADLINE,
        App::operation_result(&mut app.operations),
    )
    .await
    .unwrap();
    app.on_operation(result);
}

/// Serves the create sequence and returns the `terminal.run` params.
async fn serve_create(mut rx: mpsc::Receiver<testcore::FakeCore>) -> serde_json::Value {
    let mut core = rx.recv().await.unwrap();
    core.read_hello().await;
    core.answer_hello(true, &["doc.get", "doc.set", "terminal.run"], &[])
        .await;
    for _ in 0..2 {
        let read = core.read_request().await;
        assert_eq!(read["method"], "doc.get");
        core.reply(&read["id"], json!({"value": null, "version": 0}))
            .await;
    }
    let run = core.read_request().await;
    assert_eq!(run["method"], "terminal.run");
    let mut terminal = testcore::minimal_terminal();
    terminal["address"] = json!("host-b/t1");
    terminal["cwd"] = json!("/home/user");
    terminal["labels"] = run["params"]["labels"].clone();
    core.reply(&run["id"], terminal).await;
    let read = core.read_request().await;
    assert_eq!(read["method"], "doc.get");
    core.reply(&read["id"], json!({"value": null, "version": 0}))
        .await;
    let write = core.read_request().await;
    assert_eq!(write["method"], "doc.set");
    core.reply(&write["id"], json!({"version": 1})).await;
    run["params"].clone()
}

fn open_machine_menu(app: &mut App, endpoint: ClientEndpointId) -> Vec<&'static str> {
    app.hit_map.machines.push(MachineHit {
        rect: Rect::new(0, 0, 12, 1),
        status_badge: Rect::default(),
        collapse_toggle: Rect::default(),
        endpoint_id: endpoint,
    });
    app.context_menu_press((1, 0));
    assert!(app.menu.is_some(), "the machine row opens a context menu");
    app.menu
        .as_ref()
        .unwrap()
        .items()
        .iter()
        .map(|i| i.label)
        .collect()
}

#[tokio::test]
async fn new_workspace_on_an_empty_connected_machine_targets_that_machine() {
    // Both machines are online; only Local has a terminal, so the active
    // machine is Local. Creating from host-b's menu must land on host-b.
    let mut app = app_multi(
        vec![spaced_term("t1", "work")],
        vec![machine("host-b", "connected")],
    );
    pending(&mut app);
    app.focused = Some("t1".into());
    app.refresh_projection();
    assert_eq!(
        app.projection.active_endpoint_id,
        ClientEndpointId::Local,
        "the active machine stays Local"
    );

    let labels = open_machine_menu(&mut app, ClientEndpointId::Remote("host-b".into()));
    assert!(labels.contains(&"New workspace"), "menu: {labels:?}");
    assert!(!labels.contains(&"Connect"), "menu: {labels:?}");
    let ContextMenuTarget::Machine { name, connected } = &app.menu.as_ref().unwrap().target else {
        panic!("the menu targets the machine");
    };
    assert_eq!(name.as_deref(), Some("host-b"));
    assert!(*connected);

    let rx = operations(&mut app);
    let server = tokio::spawn(serve_create(rx));
    app.menu_key(&press(Key::Enter));
    assert!(app.menu.is_none());
    settle(&mut app).await;
    assert_eq!(app.pending_actions, 0);
    assert!(app.notice.is_none());

    let params = server.await.unwrap();
    assert_eq!(params["on"], "host-b", "created on the menu's machine");
    assert!(
        params.get("cwd").is_none(),
        "an empty machine has no local cwd: {}",
        params
    );
}

#[tokio::test]
async fn new_workspace_from_local_menu_targets_local_when_active_is_remote() {
    // A focused terminal on host-b makes it the active machine; Local's
    // menu still creates on Local (no `on`), in its home.
    let mut app = app_multi(
        vec![spaced_term("host-b/t1", "work")],
        vec![machine("host-b", "connected")],
    );
    pending(&mut app);
    app.focused = Some("host-b/t1".into());
    app.refresh_projection();
    assert_eq!(
        app.projection.active_endpoint_id,
        ClientEndpointId::Remote("host-b".into())
    );

    let labels = open_machine_menu(&mut app, ClientEndpointId::Local);
    assert!(labels.contains(&"New workspace"), "menu: {labels:?}");

    let mut rx = operations(&mut app);
    let server = tokio::spawn(async move {
        let mut core = rx.recv().await.unwrap();
        core.read_hello().await;
        core.answer_hello(true, &["doc.get", "doc.set", "terminal.run"], &[])
            .await;
        for _ in 0..2 {
            let read = core.read_request().await;
            assert_eq!(read["method"], "doc.get");
            core.reply(&read["id"], json!({"value": null, "version": 0}))
                .await;
        }
        let run = core.read_request().await;
        assert_eq!(run["method"], "terminal.run");
        let mut terminal = testcore::minimal_terminal();
        terminal["address"] = json!("t9");
        terminal["cwd"] = json!("/home/user");
        terminal["labels"] = run["params"]["labels"].clone();
        core.reply(&run["id"], terminal).await;
        let read = core.read_request().await;
        core.reply(&read["id"], json!({"value": null, "version": 0}))
            .await;
        let write = core.read_request().await;
        assert_eq!(write["method"], "doc.set");
        core.reply(&write["id"], json!({"version": 1})).await;
        run["params"].clone()
    });
    app.menu_key(&press(Key::Enter));
    settle(&mut app).await;
    assert_eq!(app.pending_actions, 0);
    assert!(app.notice.is_none());

    let params = server.await.unwrap();
    assert!(
        params.get("on").is_none(),
        "created on Local, not the active remote: {}",
        params
    );
    assert!(params.get("cwd").is_none());
}

#[tokio::test]
async fn the_new_workspace_action_follows_the_sidebar_highlight() {
    // The sidebar's only "selected row" is the Navigate/mobile picker
    // highlight (a workspace). A highlight on host-b's workspace makes the
    // `new workspace` action create on host-b even though the focused pane
    // and active machine are Local.
    let mut app = app_multi(
        vec![
            spaced_term("t1", "work"),
            spaced_term("host-b/t2", "remote"),
        ],
        vec![machine("host-b", "connected")],
    );
    pending(&mut app);
    app.focused = Some("t1".into());
    app.refresh_projection();
    assert_eq!(app.projection.active_endpoint_id, ClientEndpointId::Local);

    let remote = app
        .projection
        .endpoints
        .iter()
        .find(|e| e.endpoint_id == ClientEndpointId::Remote("host-b".into()))
        .and_then(|e| e.snapshot.as_deref())
        .and_then(|s| s.workspaces.first())
        .expect("host-b lists a workspace");
    let target = crate::chrome::navigate::navigation_target(
        &app.projection.endpoints,
        &ClientEndpointId::Remote("host-b".into()),
        &remote.workspace_id,
    )
    .expect("host-b workspace is a navigation target");
    app.navigate_target = Some(target);

    let (creation, _) = app.workspace_creation().unwrap();
    assert_eq!(creation.params.on.as_deref(), Some("host-b"));
}

#[tokio::test]
async fn connect_is_not_offered_on_a_connected_machine() {
    let mut app = app_multi(
        vec![spaced_term("t1", "work")],
        vec![machine("host-b", "connected")],
    );
    pending(&mut app);
    app.focused = Some("t1".into());
    app.refresh_projection();

    let labels = open_machine_menu(&mut app, ClientEndpointId::Remote("host-b".into()));
    assert!(
        !labels.contains(&"Connect"),
        "a connected machine does nothing with Connect: {labels:?}"
    );
}

#[tokio::test]
async fn a_disconnected_machine_keeps_connect() {
    let mut app = app_multi(
        vec![spaced_term("t1", "work")],
        vec![machine("host-b", "unreachable")],
    );
    pending(&mut app);
    app.focused = Some("t1".into());
    app.refresh_projection();

    let labels = open_machine_menu(&mut app, ClientEndpointId::Remote("host-b".into()));
    assert!(labels.contains(&"Connect"), "menu: {labels:?}");
    assert!(!labels.contains(&"New workspace"), "menu: {labels:?}");
}
