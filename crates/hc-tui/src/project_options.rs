//! The projection under the structure options the app's configuration
//! selects, and the test every re-read that must agree with the projection
//! shares.
use super::*;

/// `project` with the structure options the app's configuration selects.
pub(crate) fn project_with_options(
    view: &View,
    selected: Option<&str>,
    opts: ReadOptions,
) -> Projection {
    let own = layout_document(view).unwrap_or_default();
    project_with(view, selected, &own, |machine, terms, selected_here| {
        let spaces = crate::structure::read_with(terms, app_document(view, machine), machine, opts);
        structured_snapshot_from_spaces(
            &spaces,
            terms,
            selected_here,
            view.generation,
            view.version,
            &own,
            machine,
        )
    })
}

/// Whether the projection lists `term` at all: not hidden by a matching
/// `closed` entry, and not a popup-owned terminal — client chrome, not a
/// layout terminal, like the reference's client-local popup runtime. Every
/// re-read that must agree with the projection about which terminals exist
/// (a worker's parent chain) uses the same test.
pub(crate) fn projected(term: &Terminal, closed: &BTreeMap<String, u64>) -> bool {
    closed.get(&term.address) != Some(&term.created_ms)
        && !term.labels.contains_key(crate::app::popup::POPUP_LABEL)
}
