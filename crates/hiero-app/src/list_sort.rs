//! How the list of diagrams is sorted when a column's header is clicked.

use std::cmp::Ordering;

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
            Self::Type => a.meta.diagram_type.label().cmp(b.meta.diagram_type.label()),
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
