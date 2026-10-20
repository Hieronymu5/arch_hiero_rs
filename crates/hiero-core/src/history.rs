//! Version history support: the content a version captures, its hash for the Saved/Unsaved
//! status, and the undo/redo stack.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::stored::{
    StoredConnection, StoredContent, StoredDiagram, StoredElement, canonical_json,
};

/// The longest description a saved version can have.
pub const VERSION_DESCRIPTION_MAX: usize = 200;

/// Checks a version's description, returning the message to show when it isn't valid.
pub fn validate_version_description(text: &str) -> Result<(), &'static str> {
    if text.chars().count() > VERSION_DESCRIPTION_MAX {
        Err("Description must be 200 characters or fewer.")
    } else {
        Ok(())
    }
}

/// Everything a version captures: what a user can change about a diagram. The id and type
/// never change, and the dates aren't content.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagramSnapshot {
    pub name: String,
    pub description: Option<String>,
    pub colors: BTreeMap<String, String>,
    pub content: StoredContent,
}

impl DiagramSnapshot {
    pub fn of(diagram: &StoredDiagram) -> Self {
        Self {
            name: diagram.meta.name.clone(),
            description: diagram.meta.description.clone(),
            colors: diagram.colors.clone(),
            content: diagram.content.clone(),
        }
    }

    /// Applies the snapshot to `diagram`, keeping its id, type, and dates.
    pub fn apply_to(&self, diagram: &mut StoredDiagram) {
        diagram.meta.name.clone_from(&self.name);
        diagram.meta.description.clone_from(&self.description);
        diagram.colors.clone_from(&self.colors);
        diagram.content.clone_from(&self.content);
    }

    fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "colors": self.colors,
            "elements": self.content.elements.iter().map(StoredElement::to_json).collect::<Vec<_>>(),
            "connections": self.content.connections.iter().map(StoredConnection::to_json).collect::<Vec<_>>(),
        })
    }

    /// The SHA-256 of the snapshot's canonical JSON, as lowercase hex.
    pub fn hash(&self) -> String {
        let digest = Sha256::digest(canonical_json(&self.to_json()).as_bytes());
        digest.iter().fold(String::with_capacity(64), |mut out, b| {
            use std::fmt::Write as _;
            let _ = write!(out, "{b:02x}");
            out
        })
    }
}

/// The hash of a diagram's current content, compared with its latest version's.
pub fn content_hash(diagram: &StoredDiagram) -> String {
    DiagramSnapshot::of(diagram).hash()
}

/// Undo and redo, as lists of earlier and later states, capped in length.
#[derive(Debug, Clone)]
pub struct UndoStack<T> {
    past: Vec<T>,
    future: Vec<T>,
    limit: usize,
    /// Groups consecutive changes with the same key into one step, such as typing in a field.
    last_key: Option<String>,
}

impl<T> Default for UndoStack<T> {
    fn default() -> Self {
        Self::new(500)
    }
}

impl<T> UndoStack<T> {
    pub fn new(limit: usize) -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            limit: limit.max(1),
            last_key: None,
        }
    }

    /// Records `before`, the state before a change. Clears redo.
    pub fn push(&mut self, before: T) {
        self.push_keyed(before, None);
    }

    /// Records `before` unless the previous change had the same `key`, so a run of changes,
    /// such as typing in one field, is undone in one step.
    pub fn push_keyed(&mut self, before: T, key: Option<String>) {
        self.future.clear();
        if key.is_some() && key == self.last_key && !self.past.is_empty() {
            return;
        }
        self.last_key = key;
        self.past.push(before);
        if self.past.len() > self.limit {
            self.past.remove(0);
        }
    }

    /// Ends any group of keyed changes, so the next change is a step of its own.
    pub fn break_group(&mut self) {
        self.last_key = None;
    }

    /// The state to go back to, given the current one, which becomes redoable.
    pub fn undo(&mut self, current: T) -> Option<T> {
        let previous = self.past.pop()?;
        self.future.push(current);
        self.last_key = None;
        Some(previous)
    }

    /// The state to go forward to, given the current one, which becomes undoable.
    pub fn redo(&mut self, current: T) -> Option<T> {
        let next = self.future.pop()?;
        self.past.push(current);
        self.last_key = None;
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Puts `state` onto the redo list, as a change that was overtaken by someone else's.
    pub fn push_redo(&mut self, state: T) {
        self.future.push(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DiagramMeta, DiagramTypeId};
    use serde_json::json;

    fn diagram(name: &str) -> StoredDiagram {
        StoredDiagram {
            meta: DiagramMeta {
                id: "d".into(),
                diagram_type: DiagramTypeId::Conceptual,
                name: name.into(),
                description: None,
                created_at: "2026-01-01T00:00:00.000Z".into(),
                updated_at: "2026-01-01T00:00:00.000Z".into(),
            },
            colors: BTreeMap::new(),
            content: StoredContent {
                elements: vec![
                    StoredElement::from_json(&json!({
                        "id": "e", "kind": "actor", "label": "A", "agent": true, "x": 40, "y": 40
                    }))
                    .unwrap(),
                ],
                connections: vec![],
            },
        }
    }

    #[test]
    fn hash_ignores_dates_but_not_content() {
        let a = diagram("One");
        let mut b = a.clone();
        b.meta.updated_at = "2027-01-01T00:00:00.000Z".into();
        assert_eq!(content_hash(&a), content_hash(&b));
        b.meta.name = "Two".into();
        assert_ne!(content_hash(&a), content_hash(&b));
        assert_eq!(content_hash(&a).len(), 64);
    }

    #[test]
    fn undo_and_redo() {
        let mut stack = UndoStack::new(10);
        stack.push(1);
        stack.push(2);
        assert_eq!(stack.undo(3), Some(2));
        assert_eq!(stack.undo(2), Some(1));
        assert_eq!(stack.undo(1), None);
        assert_eq!(stack.redo(1), Some(2));
        stack.push(5);
        assert!(!stack.can_redo());
    }

    #[test]
    fn keyed_changes_group() {
        let mut stack = UndoStack::new(10);
        stack.push_keyed(1, Some("name".into()));
        stack.push_keyed(2, Some("name".into()));
        stack.push_keyed(3, Some("other".into()));
        assert_eq!(stack.undo(4), Some(3));
        assert_eq!(stack.undo(3), Some(1));
    }

    #[test]
    fn limit_drops_oldest() {
        let mut stack = UndoStack::new(2);
        stack.push(1);
        stack.push(2);
        stack.push(3);
        assert_eq!(stack.undo(4), Some(3));
        assert_eq!(stack.undo(3), Some(2));
        assert_eq!(stack.undo(2), None);
    }
}
