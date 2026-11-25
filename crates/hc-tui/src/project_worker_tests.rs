//! Hidden workers through the projection: each gets an agent row carrying
//! its parent's ids, and no pane or layout of its own.
use super::*;
use crate::sidebar::fixture::{agent, term, view};
use crate::structure::ReadOptions;
use serde_json::json;
use std::sync::Arc;

const NEST: ReadOptions = ReadOptions { nest_workers: true };

fn worker(base: Terminal, parent: &str, name: &str, created_ms: u64) -> Terminal {
    let mut worker = base;
    worker.parent = Some(parent.into());
    worker.name = Some(name.into());
    worker.created_ms = created_ms;
    worker
}

/// App space `S` with one tab per `(tab id, terminal)`.
fn with_app_doc(mut v: View, tabs: &[(&str, &str)]) -> View {
    let tabs = tabs
        .iter()
        .map(|(id, t)| json!({"id": id, "root": {"type": "block", "content": {"terminal": t}}}))
        .collect::<Vec<_>>();
    v.documents_mut().insert(
        hc_proto::watch::DocKey {
            machine: None,
            name: "herdr-app".into(),
        },
        Arc::new(hc_proto::watch::Document {
            value: json!({"spaces": [{"id": "S", "name": "S", "folder": "/home/user/proj", "tabs": tabs}]}),
            version: 1,
            ..Default::default()
        }),
    );
    v
}

fn agent_ids(projection: &Projection) -> Vec<&str> {
    projection
        .snapshot
        .agents
        .iter()
        .map(|a| a.pane_id.as_str())
        .collect()
}

#[test]
fn flag_off_no_agent_is_a_worker_popup_row() {
    let v = with_app_doc(
        view(
            vec![
                agent("t3", "working"),
                worker(agent("t5", "idle"), "t3", "five", 1),
            ],
            vec![],
        ),
        &[("tab", "t3")],
    );
    let flat = project(&v, Some("t3"));
    assert_eq!(
        flat.snapshot.workspaces.len(),
        2,
        "t5 falls back on its own"
    );
    assert!(flat.snapshot.agents.iter().all(|a| !a.worker_popup));
}

#[test]
fn worker_rows_follow_their_parent_in_creation_order() {
    let mut dead = worker(term("t8"), "t3", "dead", 4);
    dead.state = "finished".into();
    let v = with_app_doc(
        view(
            vec![
                agent("t3", "working"),
                worker(agent("t5", "blocked"), "t3", "five", 1),
                worker(agent("t6", "idle"), "t3", "six", 2),
                worker(agent("t7", "idle"), "t5", "seven", 3),
                dead,
            ],
            vec![],
        ),
        &[("tab", "t3")],
    );
    let projection = project_with_options(&v, Some("t3"), NEST);
    assert_eq!(agent_ids(&projection), ["t3", "t5", "t7", "t6"]);
    assert_eq!(projection.snapshot.workspaces.len(), 1);
}

#[test]
fn a_worker_row_carries_its_parents_ids_and_has_no_pane() {
    let v = with_app_doc(
        view(
            vec![
                agent("t3", "working"),
                worker(agent("t5", "blocked"), "t3", "five", 1),
            ],
            vec![],
        ),
        &[("tab", "t3")],
    );
    let projection = project_with_options(&v, Some("t3"), NEST);
    let snapshot = &projection.snapshot;
    let parent = snapshot.agents.iter().find(|a| a.pane_id == "t3").unwrap();
    let row = snapshot.agents.iter().find(|a| a.pane_id == "t5").unwrap();
    assert!(row.worker_popup && !parent.worker_popup);
    assert_eq!(
        (&row.workspace_id, &row.tab_id),
        (&parent.workspace_id, &parent.tab_id)
    );
    assert_eq!(row.name.as_deref(), Some("five"));
    assert_eq!(row.agent_status, AgentStatus::Blocked);
    assert!(!row.focused);
    assert!(snapshot.panes.iter().all(|p| p.pane_id != "t5"));
    assert!(projection.layouts.values().all(|l| {
        !l.root
            .as_ref()
            .unwrap()
            .terminals()
            .contains(&"t5".to_string())
    }));
}

#[test]
fn a_worker_that_is_no_reporter_still_gets_an_unknown_row() {
    let v = with_app_doc(
        view(
            vec![agent("t3", "working"), worker(term("t6"), "t3", "plain", 1)],
            vec![],
        ),
        &[("tab", "t3")],
    );
    let projection = project_with_options(&v, Some("t3"), NEST);
    let row = projection
        .snapshot
        .agents
        .iter()
        .find(|a| a.pane_id == "t6")
        .expect("a non-agent worker is reachable through its row");
    assert!(row.worker_popup);
    assert_eq!(row.agent_status, AgentStatus::Unknown);
}

#[test]
fn a_worker_of_a_non_agent_member_still_nests_after_it() {
    let v = with_app_doc(
        view(
            vec![
                term("t3"),
                agent("t4", "idle"),
                worker(agent("t5", "working"), "t3", "five", 1),
            ],
            vec![],
        ),
        &[("tab", "t3"), ("x", "t4")],
    );
    let projection = project_with_options(&v, Some("t4"), NEST);
    assert_eq!(agent_ids(&projection), ["t5", "t4"]);
    let row = &projection.snapshot.agents[0];
    assert_eq!(
        row.tab_id,
        projection
            .snapshot
            .panes
            .iter()
            .find(|p| p.pane_id == "t3")
            .unwrap()
            .tab_id
    );
}

#[test]
fn a_live_orphan_stays_a_plain_member_of_its_fallback_space() {
    let v = with_app_doc(
        view(
            vec![
                agent("t3", "working"),
                worker(agent("t10", "idle"), "t99", "orphan", 1),
            ],
            vec![],
        ),
        &[("tab", "t3")],
    );
    let projection = project_with_options(&v, Some("t3"), NEST);
    let row = projection
        .snapshot
        .agents
        .iter()
        .find(|a| a.pane_id == "t10")
        .unwrap();
    assert!(!row.worker_popup);
    assert!(projection.snapshot.panes.iter().any(|p| p.pane_id == "t10"));
}
