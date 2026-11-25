//! `ProjectCache` under changing structure options: a change of options is
//! never an unchanged publish and re-reads every endpoint, and a changed
//! parent re-reads the structure.

use super::tests::{assert_same_projection, set_doc};
use super::*;
use crate::sidebar::fixture::{agent, term, view};
use hc_proto::terminal::Terminal;

const NEST: ReadOptions = ReadOptions { nest_workers: true };

#[test]
fn a_change_of_read_options_is_never_an_unchanged_publish() {
    let view = view(vec![term("t3")], vec![]);
    let mut cache = ProjectCache::default();
    cache.project_with(&view, Some("t3"), ReadOptions::default());
    assert!(cache.is_unchanged_with(&view, Some("t3"), ReadOptions::default()));
    assert!(!cache.is_unchanged_with(&view, Some("t3"), NEST));
}

#[test]
fn a_change_of_read_options_re_reads_every_endpoint() {
    let view = view(vec![term("t3")], vec![]);
    let mut cache = ProjectCache::default();
    cache.project_with(&view, Some("t3"), ReadOptions::default());
    let flat = Arc::clone(&cache.endpoints[&None].spaces);
    cache.project_with(&view, Some("t3"), ReadOptions::default());
    assert!(
        Arc::ptr_eq(&flat, &cache.endpoints[&None].spaces),
        "unchanged options reuse the read"
    );
    cache.project_with(&view, Some("t3"), NEST);
    assert_eq!(cache.endpoints[&None].opts, NEST);
    assert!(
        !Arc::ptr_eq(&flat, &cache.endpoints[&None].spaces),
        "new options re-read the structure"
    );
}

/// App space `S` holding `t3`, plus a live worker `t5` of `t3`.
fn worker_view(parent: Option<&str>) -> View {
    let mut worker = term("t5");
    worker.parent = parent.map(str::to_owned);
    worker.created_ms = 1;
    let mut view = view(vec![term("t3"), worker], vec![]);
    set_doc(
        &mut view,
        None,
        "herdr-app",
        serde_json::json!({"spaces": [{"id": "S", "name": "S", "folder": "/home/user/proj",
            "tabs": [{"id": "tab", "root": {"type": "block", "content": {"terminal": "t3"}}}]}]}),
    );
    view
}

#[test]
fn a_changed_parent_re_reads_the_structure() {
    let mut cache = ProjectCache::default();
    let nested = worker_view(Some("t3"));
    assert_eq!(
        cache
            .project_with(&nested, Some("t3"), NEST)
            .snapshot
            .workspaces
            .len(),
        1,
        "the worker is hidden in its parent's space"
    );
    let orphaned = worker_view(None);
    let cached = cache.project_with(&orphaned, Some("t3"), NEST);
    assert_eq!(
        cached.snapshot.workspaces.len(),
        2,
        "the orphan is placed on its own"
    );
    assert_same_projection(
        &cached,
        &crate::project::project_with_options(&orphaned, Some("t3"), NEST),
    );
}

/// App space `S` holding `t3`, with parent-linked terminals of every kind
/// the worker rule handles: a worker of `t3` and that worker's own worker, a
/// finished worker, a worker with its own space label, a worker of the
/// fallback-placed `t6`, and a worker whose parent is unknown. With `linked`
/// false every `parent` is cleared: with the setting off the projection must
/// not read `parent`, so both views project the same.
fn parent_linked_view(linked: bool) -> View {
    let of = |mut t: Terminal, parent: &str, created_ms: u64| {
        t.parent = linked.then(|| parent.to_owned());
        t.created_ms = created_ms;
        t
    };
    let mut finished = of(agent("t7", "done"), "t3", 3);
    finished.state = "finished".into();
    let mut labelled = of(agent("t8", "working"), "t3", 4);
    labelled.labels.insert("herdr.space".into(), "other".into());
    let mut loose = agent("t6", "idle");
    loose.cwd = "/home/user/elsewhere".into();
    loose.created_ms = 5;
    let mut view = view(
        vec![
            agent("t3", "working"),
            of(agent("t5", "blocked"), "t3", 1),
            of(agent("t4", "working"), "t5", 2),
            finished,
            labelled,
            loose,
            of(agent("t10", "working"), "t6", 6),
            of(agent("t11", "blocked"), "t99", 7),
        ],
        vec![],
    );
    set_doc(
        &mut view,
        None,
        "herdr-app",
        serde_json::json!({"spaces": [{"id": "S", "name": "S", "folder": "/home/user/proj",
            "tabs": [{"id": "tab", "root": {"type": "block", "content": {"terminal": "t3"}}}]}]}),
    );
    view
}

#[test]
fn with_the_setting_off_parent_links_leave_the_projection_unchanged() {
    let off = ReadOptions::default();
    let unlinked = parent_linked_view(false);
    let linked = parent_linked_view(true);
    let reference = crate::project::project_with_options(&unlinked, Some("t3"), off);
    assert_same_projection(
        &crate::project::project_with_options(&linked, Some("t3"), off),
        &reference,
    );
    // Through the cache too, when the parent links arrive on a later view.
    let mut cache = ProjectCache::default();
    cache.project_with(&unlinked, Some("t3"), off);
    assert_same_projection(&cache.project_with(&linked, Some("t3"), off), &reference);
    // The fixture is one the setting changes, so the equality is not vacuous.
    assert_ne!(
        crate::project::project_with_options(&linked, Some("t3"), NEST).snapshot,
        reference.snapshot
    );
}
