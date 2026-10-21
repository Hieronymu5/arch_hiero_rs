//! The technologies a deployed component can be built on, shown as logos in its bottom bar.

pub use crate::tech_stack_data::TECH_STACK;

/// The kinds of technology a tech stack is grouped into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TechCategory {
    Cloud,
    OperatingSystem,
    Database,
    Middleware,
    Framework,
    Language,
    Browser,
}

impl TechCategory {
    /// Every category, in the order offered and in which a component's logos run.
    pub const ALL: [Self; 7] = [
        Self::Cloud,
        Self::OperatingSystem,
        Self::Database,
        Self::Middleware,
        Self::Framework,
        Self::Language,
        Self::Browser,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Cloud => "Cloud",
            Self::OperatingSystem => "Operating System",
            Self::Database => "Database",
            Self::Middleware => "Middleware",
            Self::Framework => "Frameworks",
            Self::Language => "Languages",
            Self::Browser => "Browsers",
        }
    }
}

/// A technology: its stable id (as stored), label, category, and 18 × 18 PNG logo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TechItem {
    pub id: &'static str,
    pub label: &'static str,
    pub category: TechCategory,
    pub png: &'static [u8],
}

pub fn find(id: &str) -> Option<&'static TechItem> {
    TECH_STACK.iter().find(|item| item.id == id)
}

/// The technologies with the given ids, in the catalogue's order. Unknown ids are skipped.
pub fn items(ids: &[String]) -> Vec<&'static TechItem> {
    TECH_STACK
        .iter()
        .filter(|item| ids.iter().any(|id| id == item.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_has_logos_in_category_order() {
        assert_eq!(TECH_STACK.len(), 92);
        assert!(TECH_STACK.iter().all(|i| i.png.starts_with(b"\x89PNG")));
        let order: Vec<usize> = TECH_STACK
            .iter()
            .map(|i| {
                TechCategory::ALL
                    .iter()
                    .position(|c| *c == i.category)
                    .unwrap()
            })
            .collect();
        assert!(order.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(find("aws").unwrap().label, "AWS");
    }

    #[test]
    fn items_keep_catalogue_order() {
        let ids = vec!["docker".to_owned(), "aws".to_owned(), "nope".to_owned()];
        let found: Vec<&str> = items(&ids).iter().map(|i| i.id).collect();
        assert_eq!(found.first(), Some(&"aws"));
        assert!(!found.contains(&"nope"));
    }
}
