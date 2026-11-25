//! The terminal comparisons `ProjectCache` decides reuse with: whole-record
//! equality, and equality of exactly the fields `structure::read` consumes.
use crate::project::repository::repository_equal;
use hc_proto::terminal::Terminal;

pub(super) fn terms_equal(stored: &[Terminal], current: &[&Terminal]) -> bool {
    stored.len() == current.len() && stored.iter().zip(current).all(|(a, b)| a == *b)
}

/// Exactly the fields `structure::read` consumes: identity, ordering,
/// finished state and the labels/cwd/repository facts that group terminals
/// into spaces and tabs. Title, status, turn and every other display-only
/// field are excluded — changing them reuses the `Space`s and rebuilds
/// only the per-terminal snapshot that carries them.
pub(super) fn terms_structurally_equal(stored: &[Terminal], current: &[&Terminal]) -> bool {
    stored.len() == current.len()
        && stored
            .iter()
            .zip(current)
            .all(|(a, b)| structurally_equal(a, b))
}

fn structurally_equal(a: &Terminal, b: &Terminal) -> bool {
    a.address == b.address
        && a.created_ms == b.created_ms
        && (a.state == "finished") == (b.state == "finished")
        && a.cwd == b.cwd
        && a.parent == b.parent
        && a.name == b.name
        && a.labels.get("herdr.space") == b.labels.get("herdr.space")
        && a.labels.get("herdr.tab") == b.labels.get("herdr.tab")
        && repository_equal(a.repository.as_ref(), b.repository.as_ref())
}
