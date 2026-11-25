//! Worker terminals that get no tab: creation never labels one, and the
//! creation re-read lists the same terminals the projection does.
use super::*;
use crate::chrome::status::AgentStatus;
use hc_proto::terminal::Terminal;

fn worker(address: &str, parent: &str, created_ms: u64) -> Terminal {
    let mut term = crate::sidebar::fixture::term(address);
    term.created_ms = created_ms;
    term.parent = Some(parent.into());
    term.name = Some("bwau-1".into());
    term
}

fn nesting(terms: Vec<Terminal>) -> App {
    let mut app = app_with_terms(terms);
    app.shell.nest_worker_terminals = true;
    app.focused = Some("t3".into());
    app.refresh_projection();
    app
}

fn promoted(app: &App) -> Vec<String> {
    let creation = app.creation(None, false).unwrap();
    creation
        .promotion
        .expect("the unlabelled space is promoted")
        .members
        .into_iter()
        .map(|member| member.terminal)
        .collect()
}

#[tokio::test]
async fn a_new_tab_labels_no_hidden_worker() {
    let app = nesting(vec![
        crate::sidebar::fixture::term("t3"),
        worker("t5", "t3", 1),
    ]);
    assert_eq!(
        app.projection.snapshot.workspaces.len(),
        1,
        "the worker is hidden in its parent's space"
    );
    assert_eq!(promoted(&app), ["t3"]);
}

#[tokio::test]
async fn flag_off_a_new_tab_still_sees_the_worker_as_its_own_space() {
    let mut app = nesting(vec![
        crate::sidebar::fixture::term("t3"),
        worker("t5", "t3", 1),
    ]);
    app.shell.nest_worker_terminals = false;
    app.refresh_projection();
    assert_eq!(promoted(&app), ["t3", "t5"]);
}

#[tokio::test]
async fn the_creation_re_read_drops_what_the_projection_drops() {
    // A popup-owned terminal is not projected, so its worker is an orphan
    // there and sits in the space as a tab. The re-read must agree.
    let mut popup = crate::sidebar::fixture::term("t7");
    popup.created_ms = 1;
    popup.labels.insert(popup::POPUP_LABEL.into(), "1".into());
    let app = nesting(vec![
        crate::sidebar::fixture::term("t3"),
        popup,
        worker("t8", "t7", 2),
    ]);
    assert_eq!(promoted(&app), ["t3", "t8"]);
}

#[tokio::test]
async fn a_blocked_worker_lights_its_parents_tab_and_space() {
    let mut parent = crate::sidebar::fixture::agent("t3", "working");
    parent.created_ms = 0;
    let mut child = crate::sidebar::fixture::agent("t5", "blocked");
    child.created_ms = 1;
    child.parent = Some("t3".into());
    let app = nesting(vec![parent, child]);
    let snapshot = &app.projection.snapshot;
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.tabs[0].agent_status, AgentStatus::Blocked);
    assert_eq!(snapshot.workspaces[0].agent_status, AgentStatus::Blocked);
}
