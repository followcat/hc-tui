//! `ProjectCache` under changing structure options: a change of options is
//! never an unchanged publish and re-reads every endpoint.

use super::*;
use crate::sidebar::fixture::{term, view};

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
