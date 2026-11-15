//! The path to the open diagram shown in the top bar: the diagrams it was reached through.

use hiero_core::model::DiagramId;

use crate::app::Page;

/// The longest a diagram's name is shown in the path before it's cut short.
pub const NAME_MAX: usize = 24;

/// The diagrams the open one, `current`, was reached through, oldest first, each with its
/// place in the `back` list. Pages other than diagrams, such as its history, are skipped, as
/// are repeats. Coming back round to `current` starts the path again from there.
pub fn trail(back: &[Page], current: &DiagramId) -> Vec<(usize, DiagramId)> {
    let mut path: Vec<(usize, DiagramId)> = Vec::new();
    for (index, page) in back.iter().enumerate() {
        let Page::Editor(id) = page else {
            continue;
        };
        if let Some(at) = path.iter().position(|(_, seen)| seen == id) {
            // Back to a diagram already on the path: what came after it is left behind.
            path.truncate(at + 1);
            path[at].0 = index;
        } else {
            path.push((index, id.clone()));
        }
    }
    if let Some(at) = path.iter().position(|(_, id)| id == current) {
        path.truncate(at);
    }
    path
}

/// `name` cut to at most `max` characters, ending in an ellipsis when it was cut.
pub fn abbreviate(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        return name.to_owned();
    }
    let kept: String = name.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(id: &str) -> Page {
        Page::Editor(DiagramId::from(id))
    }

    fn ids(path: &[(usize, DiagramId)]) -> Vec<(usize, &str)> {
        path.iter().map(|(i, id)| (*i, id.0.as_str())).collect()
    }

    #[test]
    fn the_path_holds_the_diagrams_passed_through() {
        let back = [Page::List, editor("a"), editor("b")];
        assert_eq!(ids(&trail(&back, &"c".into())), [(1, "a"), (2, "b")]);
        assert!(trail(&[Page::List], &"a".into()).is_empty());
    }

    #[test]
    fn other_pages_and_repeats_are_left_out() {
        let back = [
            Page::List,
            editor("a"),
            Page::History("a".into()),
            editor("a"),
            Page::Details("a".into()),
            editor("b"),
        ];
        assert_eq!(ids(&trail(&back, &"c".into())), [(3, "a"), (5, "b")]);
    }

    #[test]
    fn coming_back_round_starts_the_path_again() {
        // a, b, then a again: b is left behind.
        let back = [Page::List, editor("a"), editor("b"), editor("a")];
        assert_eq!(ids(&trail(&back, &"c".into())), [(3, "a")]);
        // Back at a diagram on the path, the path ends before it.
        let back = [Page::List, editor("a"), editor("b"), editor("c")];
        assert_eq!(ids(&trail(&back, &"b".into())), [(1, "a")]);
    }

    #[test]
    fn long_names_are_cut_short() {
        assert_eq!(abbreviate("Payments", 10), "Payments");
        assert_eq!(abbreviate("Order handling", 14), "Order handling");
        assert_eq!(abbreviate("Order handling service", 10), "Order han…");
        // No space before the ellipsis.
        assert_eq!(abbreviate("Order handling service", 7), "Order…");
        assert_eq!(abbreviate("Ünïcödé names", 4), "Ünï…");
    }
}
