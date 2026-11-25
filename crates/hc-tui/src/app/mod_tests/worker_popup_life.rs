//! The worker popup's title bar, keep, and lifetime: it ends with its
//! worker, and while open it counts as the viewer of that worker.
use super::image_paste::{core_id, get_reply, operations};
use super::pane_cache::{drive_until, next_attach, serve, with_openers};
use super::worker_popup::{multi, opened, press, single};
use super::*;
use crate::app::float::FloatKey;
use crate::chrome::status::AgentStatus;
use hc_proto::frame::Kind;

fn open_in(mut app: App, terminal: &str) -> App {
    app.focused = app.focused.take().or(Some("t3".into()));
    app.refresh_projection();
    app.open_worker_popup(terminal.into());
    draw_to(&mut app, 120, 40);
    app
}

fn status_of(app: &App, id: &str) -> AgentStatus {
    app.projection
        .snapshot
        .agents
        .iter()
        .find(|agent| agent.pane_id == id)
        .unwrap()
        .agent_status
}

// ---- title bar ---------------------------------------------------------

#[tokio::test]
async fn the_title_bar_shows_the_workers_status_name_parent_and_buttons() {
    let mut app = single();
    app.focused = Some("t3".into());
    app.refresh_projection();
    app.open_worker_popup("t5".into());
    let screen = screen_lines(&draw_to(&mut app, 120, 40));
    let bar = screen
        .iter()
        .find(|line| line.contains("from t3"))
        .expect("a title bar names the parent");
    let dot = crate::chrome::status::status_icon(AgentStatus::Blocked, app.shell.status_indicators);
    for part in [dot, "bwau-1", "blocked", "from t3", "↗ keep", "×"] {
        assert!(bar.contains(part), "{part:?} missing in {bar:?}");
    }
    let buttons = app.hit_map.extra.popup_buttons.expect("buttons are drawn");
    let row = screen.iter().position(|line| line == bar).unwrap() as u16;
    let at = |rect: ratatui::layout::Rect| {
        (rect.y == row).then(|| {
            bar.chars()
                .skip(rect.x as usize)
                .take(rect.width as usize)
                .collect::<String>()
        })
    };
    assert!(at(buttons.keep).unwrap().contains("↗ keep"));
    assert!(at(buttons.close).unwrap().contains('×'));
}

#[tokio::test]
async fn a_worker_with_no_status_shows_no_status_pieces() {
    let mut app = single();
    let mut plain = crate::sidebar::fixture::term("t6");
    plain.created_ms = 1;
    plain.parent = Some("t3".into());
    plain.name = Some("plain".into());
    let mut terms: Vec<_> = app.view.inventory.terminals().values().cloned().collect();
    terms.push(plain);
    app.view.inventory_mut().replace(2, terms);
    app.focused = Some("t3".into());
    app.refresh_projection();
    app.open_worker_popup("t6".into());
    let screen = screen_lines(&draw_to(&mut app, 120, 40));
    let bar = screen.iter().find(|l| l.contains("from t3")).unwrap();
    assert!(bar.contains("plain") && !bar.contains("unknown"), "{bar:?}");
}

// ---- keep ----------------------------------------------------------------

#[tokio::test]
async fn keep_submits_a_peek_keep_for_the_worker_into_a_new_tab_and_closes() {
    let mut app = open_in(single(), "t5");
    let mut rx = operations(&mut app);
    core_id(&mut app, "core-fake-1");
    let server = tokio::spawn(async move {
        let mut core = rx.recv().await.unwrap();
        core.read_hello().await;
        core.answer_hello(
            true,
            &[
                "terminal.get",
                "terminal.label",
                "doc.get",
                "doc.set",
                "terminal.keep",
            ],
            &[],
        )
        .await;
        let get = core.read_request().await;
        assert_eq!(get["method"], "terminal.get");
        assert_eq!(get["params"]["terminal"], "t5");
        core.reply(&get["id"], get_reply("t5", 1)).await;
        let label = core.read_request().await;
        assert_eq!(label["method"], "terminal.label");
        label["params"].clone()
    });
    let keep = app.hit_map.extra.popup_buttons.unwrap().keep;
    press(&mut app, keep);
    assert!(app.popup.is_none(), "the popup closes after keeping");
    assert!(app.notice.is_none());
    let params = tokio::time::timeout(crate::testcore::DEADLINE, server)
        .await
        .expect("the keep reached the core")
        .unwrap();
    assert_eq!(params["terminal"], "t5");
    assert_eq!(params["labels"]["herdr.space"], "S");
    assert!(
        params["labels"]["herdr.tab"]
            .as_str()
            .is_some_and(|tab| !tab.is_empty() && tab != "tab"),
        "a fresh tab, not the parent's: {params}"
    );
    assert!(params["labels"].get("herdr.seen").is_none());
}

#[tokio::test]
async fn keep_on_another_machines_worker_is_refused_and_leaves_the_popup_open() {
    let mut app = multi();
    operations(&mut app);
    core_id(&mut app, "core-fake-1");
    app.focused = Some("t1".into());
    app.refresh_projection();
    app.open_worker_popup("host-a/t5".into());
    draw_to(&mut app, 120, 40);
    let keep = app.hit_map.extra.popup_buttons.unwrap().keep;
    press(&mut app, keep);
    let notice = app.notice.as_ref().expect("a refusal notice");
    assert_eq!(notice.title, "Keep unavailable");
    assert!(notice.body.contains("worker's machine"), "{}", notice.body);
    assert_eq!(app.pending_actions, 0, "nothing was submitted");
    assert_eq!(opened(&app), Some("host-a/t5"));
}

#[tokio::test]
async fn peek_keep_on_another_machines_terminal_shows_the_same_notice_and_closes() {
    let mut app = multi();
    operations(&mut app);
    core_id(&mut app, "core-fake-1");
    app.focused = Some("t1".into());
    app.refresh_projection();
    app.open_peek("host-a/t5".into());
    let enter = term_key(hc_vt::Key::Enter, hc_vt::Mods::NONE, hc_vt::Action::Press);
    assert!(app.peek_key(&enter));
    assert!(app.peek.is_none(), "the peek closes first, as before");
    let notice = app.notice.as_ref().expect("a refusal notice");
    assert_eq!(notice.title, "Keep unavailable");
    assert!(notice.body.contains("worker's machine"));
    assert_eq!(app.pending_actions, 0);
}

// ---- lifetime ----------------------------------------------------------

#[tokio::test]
async fn the_worker_leaving_the_inventory_closes_the_popup() {
    let mut app = open_in(single(), "t5");
    let terms: Vec<_> = app
        .view
        .inventory
        .terminals()
        .values()
        .filter(|t| t.address != "t5")
        .cloned()
        .collect();
    let seq = app.view.inventory.seq() + 1;
    app.view.inventory_mut().replace(seq, terms);
    app.reconcile_published_view();
    assert!(app.popup.is_none());
}

/// `id` is an idle reporter whose `done` completion nobody has seen.
fn unseen_done_app(id: &str) -> App {
    let mut app = single();
    let mut terms: Vec<_> = app.view.inventory.terminals().values().cloned().collect();
    for term in &mut terms {
        if term.address == id {
            term.status.as_mut().unwrap().state = "done".into();
            term.turn = 5;
            term.labels.insert("herdr.seen".into(), "0".into());
        }
    }
    app.view.inventory_mut().replace(2, terms);
    app.focused = Some("t3".into());
    app.refresh_projection();
    app
}

fn seen_writes(app: &mut App) -> Vec<(String, u64)> {
    let rx = app.seen_req_rx.as_mut().expect("no seen worker started");
    std::iter::from_fn(|| rx.try_recv().ok())
        .map(|write| (write.terminal, write.turn))
        .collect()
}

/// Serve the popup's attach and drive it to a fresh paint. The returned
/// wire must stay alive for the paint to stay fresh.
async fn paint_popup(
    app: &mut App,
    requests: &mut mpsc::Receiver<(crate::testcore::FakeCore, hc_proto::terminal::AttachParams)>,
) -> crate::testcore::FakeCore {
    let (core, params) = next_attach(app, requests).await;
    let mut core = serve(core, &params, 1).await;
    core.send_stream(1, Kind::Data, b"\x1b[?9001lhello").await;
    drive_until(app, |app| {
        app.popup.as_ref().is_some_and(|p| p.pane.has_fresh_paint())
    })
    .await;
    core
}

#[tokio::test]
async fn reading_a_done_worker_in_the_popup_acknowledges_and_marks_it_seen() {
    let mut app = unseen_done_app("t5");
    assert_eq!(status_of(&app, "t5"), AgentStatus::Done);
    let mut requests = with_openers(&mut app);
    app.open_worker_popup("t5".into());
    let _core = paint_popup(&mut app, &mut requests).await;
    draw_to(&mut app, 120, 40);
    assert_ne!(status_of(&app, "t5"), AgentStatus::Done, "acknowledged");
    app.share_seen();
    assert_eq!(seen_writes(&mut app), [("t5".to_owned(), 5)]);
}

#[tokio::test]
async fn an_unpainted_popup_is_not_acknowledged_until_it_paints() {
    let mut app = unseen_done_app("t5");
    app.open_worker_popup("t5".into());
    draw_to(&mut app, 120, 40);
    assert_eq!(status_of(&app, "t5"), AgentStatus::Done);
    // Open but unpainted: not acknowledged yet, though the worker already
    // counts as viewed for `herdr.seen`, like any visible pane.
    app.share_seen();
    assert_eq!(seen_writes(&mut app), [("t5".to_owned(), 5)]);
}

/// A painted, non-worker popup over `t9`, whose `done` completion is
/// unseen: a command popup, or a persistent float.
async fn painted_other_popup(persistent: bool) -> (App, crate::testcore::FakeCore) {
    let mut app = unseen_done_app("t9");
    let mut requests = with_openers(&mut app);
    let mut pane = app.new_pane();
    pane.want(Some(pane::Spec {
        terminal: "t9".into(),
        role: pane::Role::Active,
    }));
    pane.on_view(&app.view);
    let key = persistent.then(|| FloatKey {
        owner: app.focused_owner_key().expect("a focused tab"),
        command: "alt+f".into(),
    });
    app.popup = Some(popup::PopupState {
        pane,
        terminal: "t9".into(),
        width: None,
        height: None,
        owner_tab: None,
        persistent,
        key,
        created_ms: app.view.inventory.get("t9").unwrap().created_ms,
        seen: true,
        worker: None,
    });
    let core = paint_popup(&mut app, &mut requests).await;
    (app, core)
}

#[tokio::test]
async fn a_done_terminal_in_a_command_popup_is_neither_acknowledged_nor_marked_seen() {
    let (mut app, _core) = painted_other_popup(false).await;
    assert!(app.popup.as_ref().unwrap().pane.has_fresh_paint());
    draw_to(&mut app, 120, 40);
    app.share_seen();
    assert_eq!(status_of(&app, "t9"), AgentStatus::Done);
    assert_eq!(seen_writes(&mut app), []);
}

#[tokio::test]
async fn a_done_terminal_in_a_float_is_neither_acknowledged_nor_marked_seen() {
    let (mut app, _core) = painted_other_popup(true).await;
    assert!(app.popup.as_ref().unwrap().pane.has_fresh_paint());
    draw_to(&mut app, 120, 40);
    app.share_seen();
    assert_eq!(status_of(&app, "t9"), AgentStatus::Done);
    assert_eq!(seen_writes(&mut app), []);
}

#[tokio::test]
async fn after_the_popup_closes_a_new_completion_is_neither_acknowledged_nor_marked_seen() {
    let mut app = unseen_done_app("t5");
    let mut requests = with_openers(&mut app);
    app.open_worker_popup("t5".into());
    let _core = paint_popup(&mut app, &mut requests).await;
    draw_to(&mut app, 120, 40);
    assert_ne!(
        status_of(&app, "t5"),
        AgentStatus::Done,
        "viewed while open"
    );
    app.share_seen();
    assert_eq!(seen_writes(&mut app), [("t5".to_owned(), 5)]);

    app.close_popup();
    let mut terms: Vec<_> = app.view.inventory.terminals().values().cloned().collect();
    for term in &mut terms {
        if term.address == "t5" {
            term.turn = 6;
        }
    }
    let seq = app.view.inventory.seq() + 1;
    app.view.inventory_mut().replace(seq, terms);
    app.reconcile_published_view();
    draw_to(&mut app, 120, 40);
    app.share_seen();
    assert_eq!(seen_writes(&mut app), [], "no longer viewed");
    assert_eq!(status_of(&app, "t5"), AgentStatus::Done);
}
