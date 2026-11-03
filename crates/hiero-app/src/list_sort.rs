//! How the list of diagrams is sorted when a column's header is clicked, and how sub diagrams
//! are shown under the diagrams they belong to.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use hiero_core::model::DiagramId;
use hiero_store::Summary;

/// A column of the list of diagrams that it can be sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Name,
    Type,
    Updated,
    Status,
}

impl Column {
    /// The columns in the order they're shown, with their headers.
    pub const ALL: [(Self, &'static str); 4] = [
        (Self::Name, "Name"),
        (Self::Type, "Type"),
        (Self::Updated, "Last updated"),
        (Self::Status, "Status"),
    ];

    fn compare(self, a: &Summary, b: &Summary) -> Ordering {
        match self {
            Self::Name => a.meta.name.to_lowercase().cmp(&b.meta.name.to_lowercase()),
            Self::Type => a.type_label().cmp(&b.type_label()),
            // RFC 3339 UTC timestamps sort as text.
            Self::Updated => a.meta.updated_at.cmp(&b.meta.updated_at),
            // Drafts before releases, then by the latest version.
            Self::Status => {
                (a.status.saved, a.status.latest).cmp(&(b.status.saved, b.status.latest))
            }
        }
    }
}

/// The column the list is sorted by, and which way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListSort {
    pub column: Column,
    pub descending: bool,
}

impl ListSort {
    /// The sort after clicking `column`'s header: descending at first, then each click on the
    /// same column turns it around.
    pub fn clicked(current: Option<Self>, column: Column) -> Self {
        match current {
            Some(sort) if sort.column == column => Self {
                column,
                descending: !sort.descending,
            },
            _ => Self {
                column,
                descending: true,
            },
        }
    }

    /// The arrow shown after the sorted column's header.
    pub const fn arrow(self) -> &'static str {
        if self.descending { "↓" } else { "↑" }
    }

    /// Sorts `rows`, keeping rows that tie in the order they came in (by name, from the
    /// store).
    pub fn sort(self, rows: &mut [Summary]) {
        rows.sort_by(|a, b| {
            let order = self.column.compare(a, b);
            if self.descending {
                order.reverse()
            } else {
                order
            }
        });
    }
}

/// A row of the list of diagrams, as a tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeRow {
    /// Where the diagram is in the rows given.
    pub index: usize,
    /// How many diagrams up the diagram it belongs to is.
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
}

/// The rows to show: each diagram that doesn't belong to another, and under each one that's
/// expanded, its sub diagrams, indented. Siblings keep the order of `rows`. A diagram whose
/// parent is missing, or that's in a loop of diagrams opening each other, is shown at the top.
pub fn tree_rows(rows: &[Summary], expanded: &BTreeSet<DiagramId>) -> Vec<TreeRow> {
    let index_of: BTreeMap<&DiagramId, usize> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (&r.meta.id, i))
        .collect();
    let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut roots = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        match row.parent.as_ref().and_then(|p| index_of.get(p)) {
            Some(&parent) => children.entry(parent).or_default().push(i),
            None => roots.push(i),
        }
    }
    // Diagrams only reachable through a loop have no root above them: the first of each loop
    // is shown at the top.
    let mut reachable = vec![false; rows.len()];
    let mut stack = roots.clone();
    while let Some(i) = stack.pop() {
        if !std::mem::replace(&mut reachable[i], true) {
            stack.extend(children.get(&i).into_iter().flatten().copied());
        }
    }
    for i in 0..rows.len() {
        if !reachable[i] {
            roots.push(i);
            let mut stack = vec![i];
            while let Some(j) = stack.pop() {
                if !std::mem::replace(&mut reachable[j], true) {
                    stack.extend(children.get(&j).into_iter().flatten().copied());
                }
            }
        }
    }
    roots.sort_unstable();
    let mut out = Vec::new();
    let mut shown = vec![false; rows.len()];
    let mut stack: Vec<(usize, usize)> = roots.iter().rev().map(|&i| (i, 0)).collect();
    while let Some((i, depth)) = stack.pop() {
        if std::mem::replace(&mut shown[i], true) {
            continue;
        }
        let kids = children.get(&i).map_or(&[][..], Vec::as_slice);
        let open = expanded.contains(&rows[i].meta.id);
        out.push(TreeRow {
            index: i,
            depth,
            has_children: !kids.is_empty(),
            expanded: open,
        });
        if open {
            stack.extend(kids.iter().rev().map(|&k| (k, depth + 1)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use hiero_core::model::{DiagramMeta, DiagramTypeId};
    use hiero_store::Status;

    use super::*;

    fn summary(name: &str, diagram_type: DiagramTypeId, updated: &str, saved: bool) -> Summary {
        Summary {
            meta: DiagramMeta {
                id: name.into(),
                diagram_type,
                name: name.into(),
                description: None,
                created_at: updated.into(),
                updated_at: updated.into(),
            },
            status: Status {
                saved,
                latest: Some(1),
            },
            revision: 0,
            parent: None,
            subtype: None,
        }
    }

    fn rows() -> Vec<Summary> {
        vec![
            summary(
                "alpha",
                DiagramTypeId::Physical,
                "2026-10-02T00:00:00.000Z",
                true,
            ),
            summary(
                "Beta",
                DiagramTypeId::Conceptual,
                "2026-10-03T00:00:00.000Z",
                false,
            ),
            summary(
                "gamma",
                DiagramTypeId::Conceptual,
                "2026-10-01T00:00:00.000Z",
                true,
            ),
        ]
    }

    fn names(sort: ListSort) -> Vec<String> {
        let mut rows = rows();
        sort.sort(&mut rows);
        rows.into_iter().map(|r| r.meta.name).collect()
    }

    fn with_parent(name: &str, parent: Option<&str>) -> Summary {
        let mut row = summary(name, DiagramTypeId::Conceptual, "", true);
        row.parent = parent.map(Into::into);
        row
    }

    fn shown(rows: &[Summary], expanded: &[&str]) -> Vec<(String, usize, bool)> {
        let expanded = expanded.iter().map(|&e| e.into()).collect();
        tree_rows(rows, &expanded)
            .into_iter()
            .map(|r| (rows[r.index].meta.name.clone(), r.depth, r.has_children))
            .collect()
    }

    #[test]
    fn sub_diagrams_show_indented_under_expanded_diagrams() {
        let rows = vec![
            with_parent("a", None),
            with_parent("a1", Some("a")),
            with_parent("a1x", Some("a1")),
            with_parent("a2", Some("a")),
            with_parent("b", None),
            with_parent("orphan", Some("deleted")),
        ];
        let s = |n: &str, d, c| (n.to_owned(), d, c);
        assert_eq!(
            shown(&rows, &[]),
            [s("a", 0, true), s("b", 0, false), s("orphan", 0, false)]
        );
        assert_eq!(
            shown(&rows, &["a", "a1"]),
            [
                s("a", 0, true),
                s("a1", 1, true),
                s("a1x", 2, false),
                s("a2", 1, false),
                s("b", 0, false),
                s("orphan", 0, false)
            ]
        );
    }

    #[test]
    fn diagrams_that_open_each_other_still_show() {
        let rows = vec![with_parent("x", Some("y")), with_parent("y", Some("x"))];
        assert_eq!(
            shown(&rows, &["x", "y"]),
            [("x".to_owned(), 0, true), ("y".to_owned(), 1, true)]
        );
    }

    #[test]
    fn a_click_sorts_descending_and_the_next_one_ascending() {
        let first = ListSort::clicked(None, Column::Name);
        assert!(first.descending);
        assert_eq!(first.arrow(), "↓");
        let second = ListSort::clicked(Some(first), Column::Name);
        assert!(!second.descending);
        assert_eq!(second.arrow(), "↑");
        assert!(ListSort::clicked(Some(second), Column::Name).descending);
        // Another column starts descending again.
        let other = ListSort::clicked(Some(second), Column::Type);
        assert_eq!(other.column, Column::Type);
        assert!(other.descending);
    }

    #[test]
    fn sorts_by_each_column() {
        let by = |column, descending| names(ListSort { column, descending });
        assert_eq!(by(Column::Name, true), ["gamma", "Beta", "alpha"]);
        assert_eq!(by(Column::Name, false), ["alpha", "Beta", "gamma"]);
        assert_eq!(by(Column::Updated, true), ["Beta", "alpha", "gamma"]);
        // Ties keep the store's order, by name.
        assert_eq!(by(Column::Type, true), ["alpha", "Beta", "gamma"]);
        assert_eq!(by(Column::Type, false), ["Beta", "gamma", "alpha"]);
        assert_eq!(by(Column::Status, false), ["Beta", "alpha", "gamma"]);
    }
}
