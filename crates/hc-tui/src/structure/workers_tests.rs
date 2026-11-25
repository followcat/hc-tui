use super::super::{HiddenWorker, ReadOptions, Space, read, read_with};
use hc_proto::terminal::Terminal;
use serde_json::{Value, json};

const NEST: ReadOptions = ReadOptions { nest_workers: true };

fn terminal(address: &str) -> Terminal {
    crate::sidebar::fixture::term(address)
}

/// A worker of `parent`: no labels, live, created at `created_ms`.
fn worker(address: &str, parent: &str, name: &str, created_ms: u64) -> Terminal {
    let mut term = terminal(address);
    term.parent = Some(parent.into());
    term.name = Some(name.into());
    term.created_ms = created_ms;
    term
}

/// An app document with space `S`, one tab per `(tab id, terminal)`.
fn app_doc(tabs: &[(&str, &str)]) -> Value {
    let tabs = tabs
        .iter()
        .map(|(tab, term)| json!({"id": tab, "root": {"type": "block", "content": {"terminal": term}}}))
        .collect::<Vec<_>>();
    json!({"spaces": [{"id": "S", "name": "S", "folder": "/home/user/proj", "tabs": tabs}]})
}

fn leaders(space: &Space) -> Vec<String> {
    space
        .tabs
        .iter()
        .map(|tab| tab.root.terminals()[0].clone())
        .collect()
}

fn hidden(space: &Space) -> Vec<(&str, &str)> {
    space
        .workers
        .iter()
        .map(|w| (w.address.as_str(), w.parent.as_str()))
        .collect()
}

#[test]
fn flag_off_the_unplaced_worker_forms_its_own_fallback_space() {
    let parent = terminal("t3");
    let child = worker("t5", "t3", "worker-1", 1);
    let doc = app_doc(&[("tab", "t3")]);
    let flat = read(&[&parent, &child], &doc, None);
    assert_eq!(
        read_with(&[&parent, &child], &doc, None, ReadOptions::default()),
        flat
    );
    assert_eq!(flat.len(), 2);
    assert_eq!(flat[0].name, "S");
    assert_eq!(flat[0].tabs.len(), 1);
    assert!(flat[0].workers.is_empty());
    assert_eq!(flat[1].name, "proj");
    assert!(!flat[1].from_app);
    assert_eq!(flat[1].tabs[0].root.terminals(), ["t5"]);
}

#[test]
fn a_remote_worker_is_hidden_on_its_parents_space_with_no_tab() {
    let parent = terminal("host-a/t3");
    let child = worker("host-a/t5", "host-a/t3", "worker-1", 1);
    let doc = app_doc(&[("tab", "t3")]);
    let spaces = read_with(&[&parent, &child], &doc, Some("host-a"), NEST);
    assert_eq!(spaces.len(), 1, "no fallback space: {spaces:?}");
    assert_eq!(leaders(&spaces[0]), ["host-a/t3"]);
    assert_eq!(
        spaces[0].workers,
        [HiddenWorker {
            address: "host-a/t5".into(),
            parent: "host-a/t3".into()
        }]
    );
}

#[test]
fn workers_and_their_workers_are_recorded_in_creation_order() {
    let parent = terminal("t3");
    let a = worker("t5", "t3", "a", 1);
    let b = worker("t6", "t3", "b", 2);
    let grandchild = worker("t7", "t5", "c", 3);
    let other = terminal("t4");
    let doc = app_doc(&[("tab", "t3"), ("x", "t4")]);
    let spaces = read_with(&[&parent, &other, &a, &b, &grandchild], &doc, None, NEST);
    assert_eq!(spaces.len(), 1);
    assert_eq!(leaders(&spaces[0]), ["t3", "t4"]);
    assert_eq!(
        hidden(&spaces[0]),
        [("t5", "t3"), ("t6", "t3"), ("t7", "t5")]
    );
}

#[test]
fn a_worker_of_a_fallback_placed_terminal_joins_that_space() {
    let parent = terminal("t1");
    let child = worker("t2", "t1", "w", 1);
    let spaces = read_with(&[&parent, &child], &Value::Null, None, NEST);
    assert_eq!(spaces.len(), 1);
    assert_eq!(spaces[0].name, "proj");
    assert_eq!(leaders(&spaces[0]), ["t1"]);
    assert_eq!(hidden(&spaces[0]), [("t2", "t1")]);
}

#[test]
fn a_worker_of_a_split_parent_is_hidden_with_no_tab() {
    let (a, b) = (terminal("t1"), terminal("t2"));
    let child = worker("t5", "t2", "w", 1);
    let doc = json!({"spaces": [{"id": "S", "name": "S", "tabs": [{"id": "tab", "root": {"type": "row", "children": [
        {"type": "block", "content": {"terminal": "t1"}},
        {"type": "block", "content": {"terminal": "t2"}}]}}]}]});
    let spaces = read_with(&[&a, &b, &child], &doc, None, NEST);
    assert_eq!(spaces[0].tabs.len(), 1);
    assert_eq!(spaces[0].tabs[0].root.terminals(), ["t1", "t2"]);
    assert_eq!(hidden(&spaces[0]), [("t5", "t2")]);
}

#[test]
fn finished_workers_are_hidden_and_a_live_orphan_falls_back() {
    let parent = terminal("t16");
    let mut done_a = worker("t8", "t16", "a", 1);
    let mut done_b = worker("t9", "t16", "b", 2);
    done_a.state = "finished".into();
    done_b.state = "finished".into();
    // Parent never seen: still finished, still hidden.
    let mut done_orphan = worker("t11", "t99", "c", 4);
    done_orphan.state = "finished".into();
    let orphan = worker("t10", "t99", "o", 3);
    let doc = app_doc(&[("tab", "t16")]);
    let terms = [&parent, &done_a, &done_b, &orphan, &done_orphan];
    let spaces = read_with(&terms, &doc, None, NEST);
    assert_eq!(spaces.len(), 2, "{spaces:?}");
    assert_eq!(leaders(&spaces[0]), ["t16"]);
    assert!(spaces[0].workers.is_empty(), "a finished worker has no row");
    assert_eq!(spaces[1].name, "proj");
    assert_eq!(leaders(&spaces[1]), ["t10"]);
    assert!(spaces[1].workers.is_empty());
    // Flag off keeps every one of them.
    let flat = read(&terms, &doc, None);
    assert_eq!(flat[1].tabs[0].root.terminals().len(), 4);
}

#[test]
fn a_worker_with_its_own_space_label_keeps_its_placement() {
    let parent = terminal("t3");
    let mut child = worker("t5", "t3", "w", 1);
    child.labels.insert("herdr.space".into(), "other".into());
    let spaces = read_with(&[&parent, &child], &app_doc(&[("tab", "t3")]), None, NEST);
    assert_eq!(spaces.len(), 2);
    assert!(spaces.iter().all(|s| s.workers.is_empty()));
    assert_eq!(spaces[1].tabs[0].root.terminals(), ["t5"]);
}

/// The fallback loop runs in creation order, so a worker finds its parent
/// placed only when the parent was created first. A worker created before a
/// parent that is itself a hidden worker (an id reused after a restart, say)
/// finds no placement for it and falls back to a tab of its own: shown, never
/// lost.
#[test]
fn a_worker_created_before_its_hidden_parent_falls_back_to_its_own_tab() {
    let lead = terminal("t3");
    let parent = worker("t5", "t3", "parent", 2);
    let early = worker("t4", "t5", "early", 1);
    let doc = app_doc(&[("tab", "t3")]);
    let spaces = read_with(&[&lead, &parent, &early], &doc, None, NEST);
    assert_eq!(spaces.len(), 2, "{spaces:?}");
    assert_eq!(leaders(&spaces[0]), ["t3"]);
    assert_eq!(hidden(&spaces[0]), [("t5", "t3")]);
    assert_eq!(spaces[1].name, "proj");
    assert_eq!(leaders(&spaces[1]), ["t4"]);
    assert!(spaces[1].workers.is_empty());
}
