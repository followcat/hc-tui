//! Nesting depth of the agents panel's rows: a worker's row sits under the
//! row of the terminal it was started from.

/// The nesting depth of each row of one group, in display order, given every
/// row's id and the id of the terminal it was started from.
///
/// Walks the rows with a stack of ancestors. A row whose parent is on the
/// stack goes one level below it, and the stack is cut back to that parent
/// first. Any other row is at depth 0 and resets the stack: a row nests only
/// when it follows its parent's subtree, so a worker whose parent has no row
/// (a plain shell, say) renders flat, never under an unrelated row. A row
/// that repeats an id already on the stack is treated as unrelated too, so a
/// corrupt list can never make the walk loop or nest a row under itself.
pub(crate) fn depths(rows: &[(&str, Option<&str>)]) -> Vec<usize> {
    let mut stack: Vec<(&str, usize)> = Vec::new();
    rows.iter()
        .map(|&(id, parent)| {
            let repeated = stack.iter().any(|(seen, _)| *seen == id);
            let under = parent
                .filter(|_| !repeated)
                .and_then(|parent| stack.iter().rposition(|(seen, _)| *seen == parent));
            let depth = match under {
                Some(at) => {
                    stack.truncate(at + 1);
                    stack[at].1 + 1
                }
                None => {
                    stack.clear();
                    0
                }
            };
            stack.push((id, depth));
            depth
        })
        .collect()
}

/// `depths` per group: `rows` carries each row's group key, and a change of
/// key (a space header boundary) starts a fresh walk, so a row never nests
/// under a row of another group.
pub(crate) fn depths_by_group<K: PartialEq>(rows: &[(K, &str, Option<&str>)]) -> Vec<usize> {
    let mut depths_out = Vec::with_capacity(rows.len());
    let mut start = 0;
    while start < rows.len() {
        let end = rows[start..]
            .iter()
            .position(|(key, ..)| *key != rows[start].0)
            .map_or(rows.len(), |len| start + len);
        let group = rows[start..end]
            .iter()
            .map(|&(_, id, parent)| (id, parent))
            .collect::<Vec<_>>();
        depths_out.extend(depths(&group));
        start = end;
    }
    depths_out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_worker_nests_under_its_parent_through_chains_and_siblings() {
        let rows = [
            ("t3", None),
            ("t5", Some("t3")),
            ("t7", Some("t5")),
            ("t6", Some("t3")),
            ("t10", None),
        ];
        assert_eq!(depths(&rows), [0, 1, 2, 1, 0]);
    }

    #[test]
    fn a_worker_before_its_parent_or_without_one_stays_flat() {
        assert_eq!(depths(&[("t5", Some("t3")), ("t3", None)]), [0, 0]);
        // The parent has no row (a plain shell): flat, even after other rows.
        assert_eq!(
            depths(&[("t1", None), ("t5", Some("t9")), ("t6", Some("t5"))]),
            [0, 0, 1]
        );
        // A parent that is not the nearest subtree any more no longer nests.
        assert_eq!(
            depths(&[("t1", None), ("t2", None), ("t5", Some("t1"))]),
            [0, 0, 0]
        );
    }

    #[test]
    fn a_parent_cycle_or_repeated_id_never_nests_a_row_under_itself() {
        assert_eq!(depths(&[("a", Some("b")), ("b", Some("a"))]), [0, 1]);
        assert_eq!(depths(&[("a", Some("a")), ("a", Some("a"))]), [0, 0]);
    }

    #[test]
    fn a_group_boundary_resets_the_walk() {
        let rows = [
            ("S", "t3", None),
            ("S", "t5", Some("t3")),
            ("T", "t6", Some("t3")),
            ("T", "t7", None),
        ];
        assert_eq!(depths_by_group(&rows), [0, 1, 0, 0]);
    }
}
