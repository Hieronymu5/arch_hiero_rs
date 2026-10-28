//! Cut, copy, and paste: the selected elements, with everything they hold and the arrows between
//! them, as JSON text on the system clipboard, so they can be pasted into any diagram of the same
//! type, in this window or another. Pasted text is checked as strictly as an imported file.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::engine::{Conn, DiagramType, El, Engine, Placement, descendant_ids, list_labels};
use crate::file_check::check_content;
use crate::geometry::{GRID, Point, Rect};
use crate::layout::{
    CANVAS_AREA, child_placement_area, clamp_into, content_area, find_free_position, snap_to_canvas,
};
use crate::model::{ConnectionId, DiagramTypeId, ElementId};
use crate::stored::{StoredConnection, StoredElement};

/// What the clipboard text's `format` says, so other text isn't taken for elements.
pub const CLIPBOARD_FORMAT: &str = "arch-hiero-elements";
/// The version of the clipboard text this app writes and reads.
pub const CLIPBOARD_VERSION: u64 = 1;

/// Elements on the clipboard: the copied ones, which sit in nothing, and everything they hold,
/// with the arrows between them.
pub struct Clip<T: DiagramType> {
    pub elements: Vec<El<T>>,
    pub connections: Vec<Conn<T>>,
}

impl<T: DiagramType> Clip<T> {
    /// The clipboard text: `{format, version, type, elements, connections}`.
    pub fn to_text(&self) -> String {
        let mut object = Map::new();
        object.insert("format".into(), Value::from(CLIPBOARD_FORMAT));
        object.insert("version".into(), Value::from(CLIPBOARD_VERSION));
        object.insert("type".into(), Value::from(T::ID.as_str()));
        let elements = self
            .elements
            .iter()
            .map(|e| StoredElement::from_typed(e).to_json())
            .collect();
        object.insert("elements".into(), Value::Array(elements));
        // A type without arrows has no list of them, as in its files.
        if T::ID.has_arrows() {
            let connections = self
                .connections
                .iter()
                .map(|c| StoredConnection::from_typed(c).to_json())
                .collect();
            object.insert("connections".into(), Value::Array(connections));
        }
        serde_json::to_string_pretty(&Value::Object(object)).expect("clips serialise")
    }

    /// Reads clipboard text, or says why it can't be pasted here.
    pub fn from_text(text: &str) -> Result<Self, String> {
        const NOT_OURS: &str = "The clipboard doesn’t hold diagram elements.";
        let value: Value = serde_json::from_str(text).map_err(|_| NOT_OURS)?;
        let object = value
            .as_object()
            .filter(|o| o.get("format").and_then(Value::as_str) == Some(CLIPBOARD_FORMAT))
            .ok_or(NOT_OURS)?;
        let version = object
            .get("version")
            .and_then(Value::as_u64)
            .ok_or(NOT_OURS)?;
        if version > CLIPBOARD_VERSION {
            return Err(
                "The elements were copied from a newer version of ArchHiero. Update it and try \
                 again."
                    .into(),
            );
        }
        let diagram_type = object
            .get("type")
            .and_then(Value::as_str)
            .and_then(DiagramTypeId::parse)
            .ok_or(NOT_OURS)?;
        if diagram_type != T::ID {
            return Err(format!(
                "The elements were copied from a {} diagram, so they can only be pasted into one.",
                diagram_type.label().to_lowercase()
            ));
        }
        let allowed = |k: &str| {
            matches!(k, "format" | "version" | "type" | "elements")
                || (T::ID.has_arrows() && k == "connections")
        };
        if let Some(key) = object.keys().find(|k| !allowed(k)) {
            return Err(format!(
                "The clipboard has a field “{key}” that isn’t known."
            ));
        }
        let list = |key: &str| match object.get(key) {
            None if key == "connections" && !T::ID.has_arrows() => Ok(&[][..]),
            Some(Value::Array(items)) => Ok(items.as_slice()),
            _ => Err(NOT_OURS.to_owned()),
        };
        let (elements, connections) = (list("elements")?, list("connections")?);
        check_content::<T>(elements, connections)?;
        let bad = |e: crate::stored::ContentError| e.0;
        Ok(Self {
            elements: elements
                .iter()
                .map(|v| StoredElement::from_json(v).and_then(|e| e.to_typed()))
                .collect::<Result<_, _>>()
                .map_err(bad)?,
            connections: connections
                .iter()
                .map(|v| StoredConnection::from_json(v).and_then(|c| c.to_typed()))
                .collect::<Result<_, _>>()
                .map_err(bad)?,
        })
    }
}

impl<T: DiagramType> Engine<T> {
    /// The selected elements with everything they hold and the arrows between them, for the
    /// clipboard. `None` when no element is selected.
    pub fn copy_selection(&mut self) -> Option<Clip<T>> {
        let roots: Vec<ElementId> = self.group_roots().iter().map(|e| e.id().clone()).collect();
        if roots.is_empty() {
            return None;
        }
        let mut ids: HashSet<ElementId> = HashSet::new();
        for root in &roots {
            ids.extend(descendant_ids(self.elements(), root));
            ids.insert(root.clone());
        }
        let elements = self
            .elements()
            .iter()
            .filter(|e| ids.contains(&e.id))
            .map(|e| {
                if roots.contains(&e.id) {
                    e.clone().with_parent(None)
                } else {
                    e.clone()
                }
            })
            .collect();
        let connections = self
            .connections()
            .iter()
            .filter(|c| ids.contains(&c.source_id) && ids.contains(&c.target_id))
            .cloned()
            .collect();
        self.announcement = format!("{} copied.", self.labels_of(&roots));
        Some(Clip {
            elements,
            connections,
        })
    }

    /// Copies the selected elements, then removes them with everything they hold.
    pub fn cut_selection(&mut self) -> Option<Clip<T>> {
        let clip = self.copy_selection()?;
        let roots: Vec<ElementId> = clip
            .elements
            .iter()
            .filter(|e| e.parent_id.is_none())
            .map(|e| e.id.clone())
            .collect();
        let labels = self.labels_of(&roots);
        let ids: HashSet<&ElementId> = clip.elements.iter().map(|e| &e.id).collect();
        // What's inside goes first, so removing a container doesn't leave it behind.
        let order: Vec<ElementId> = self
            .layout()
            .iter()
            .rev()
            .filter(|e| ids.contains(e.id()))
            .map(|e| e.id().clone())
            .collect();
        for id in &order {
            if self.element(id).is_some() {
                T::remove(self, id);
            }
        }
        self.clear_selection();
        self.announcement = format!("{labels} cut.");
        Some(clip)
    }

    /// Adds the clipboard's elements with new ids and selects them. They go into the selected
    /// element when it can hold them all, or else where they were copied from, a grid step
    /// down and to the right for each copy already there. Each lands in the container its
    /// middle is over, as a dropped element does.
    pub fn paste(&mut self, clip: &Clip<T>) {
        if clip.elements.is_empty() {
            return;
        }
        let new_ids: HashMap<ElementId, ElementId> = clip
            .elements
            .iter()
            .map(|e| (e.id.clone(), ElementId(self.new_id())))
            .collect();
        let mut pasted: Vec<El<T>> = clip
            .elements
            .iter()
            .map(|e| {
                let mut e = e.clone();
                e.id = new_ids[&e.id].clone();
                e.parent_id = e.parent_id.and_then(|p| new_ids.get(&p).cloned());
                e
            })
            .collect();
        let connections: Vec<Conn<T>> = clip
            .connections
            .iter()
            .filter_map(|c| {
                Some(Conn::<T> {
                    id: ConnectionId(self.new_id()),
                    source_id: new_ids.get(&c.source_id)?.clone(),
                    target_id: new_ids.get(&c.target_id)?.clone(),
                    props: c.props.clone(),
                })
            })
            .collect();
        let roots: Vec<ElementId> = pasted
            .iter()
            .filter(|e| e.parent_id.is_none())
            .map(|e| e.id.clone())
            .collect();
        let shift = self.paste_shift(&pasted);
        for e in &mut pasted {
            e.x += shift.x;
            e.y += shift.y;
        }
        let elements = self.elements().iter().cloned().chain(pasted).collect();
        self.set_elements(elements);
        let pasted_ids: HashSet<&ElementId> = new_ids.values().collect();
        let placements: Vec<(ElementId, Placement)> = roots
            .iter()
            .filter_map(|id| self.laid(id))
            .map(|root| {
                let size = root.rect.size();
                let parent = self
                    .containers_for(root.el.kind, self.layout())
                    .into_iter()
                    .filter(|c| !pasted_ids.contains(c.id()))
                    .rev()
                    .find(|c| c.rect.contains_point(root.rect.center()));
                let placement = match parent {
                    Some(parent) => Placement {
                        position: snap_to_canvas(
                            clamp_into(
                                root.rect.position(),
                                size,
                                content_area(&parent.container_rect()),
                            ),
                            size,
                        ),
                        parent_id: Some(parent.id().clone()),
                    },
                    None => Placement {
                        position: snap_to_canvas(root.rect.position(), size),
                        parent_id: None,
                    },
                };
                (root.id().clone(), T::constrain_move(self, root, placement))
            })
            .collect();
        let elements = self.apply_placements(self.elements().to_vec(), &placements);
        self.set_elements(elements);
        if !connections.is_empty() {
            let all = self
                .connections()
                .iter()
                .cloned()
                .chain(connections)
                .collect();
            self.set_connections(all);
        }
        self.selection.clone_from(&roots);
        self.selected_connection = None;
        self.announcement = match roots.as_slice() {
            [one] => match self.element(one).cloned() {
                Some(element) => format!("{} pasted.", T::describe(self, &element)),
                None => String::new(),
            },
            _ => format!("{} pasted.", self.labels_of(&roots)),
        };
    }

    /// How far pasted elements move from where they were copied: into the selected element
    /// when it can hold every one, or else a grid step down and to the right for each copy
    /// already in their place.
    fn paste_shift(&self, pasted: &[El<T>]) -> Point {
        let roots: Vec<&El<T>> = pasted.iter().filter(|e| e.parent_id.is_none()).collect();
        let rect_of = |e: &El<T>| {
            let size = T::natural_size(e, T::info(e.kind));
            Rect::new(
                e.x,
                e.y,
                e.width.unwrap_or(size.width),
                e.height.unwrap_or(size.height),
            )
        };
        let left = roots.iter().map(|e| e.x).fold(f64::INFINITY, f64::min);
        let top = roots.iter().map(|e| e.y).fold(f64::INFINITY, f64::min);
        let right = roots
            .iter()
            .map(|e| rect_of(e).right())
            .fold(f64::NEG_INFINITY, f64::max);
        let bottom = roots
            .iter()
            .map(|e| rect_of(e).bottom())
            .fold(f64::NEG_INFINITY, f64::max);
        let bounds = Rect::new(left, top, right - left, bottom - top);
        let target = self.selected().filter(|s| {
            s.is_container && roots.iter().all(|r| s.info.can_contain.contains(&r.kind))
        });
        if let Some(target) = target {
            let siblings: Vec<Rect> = self
                .layout()
                .iter()
                .filter(|e| e.el.parent_id.as_ref() == Some(target.id()))
                .map(|e| e.rect)
                .collect();
            let position = find_free_position(
                &siblings,
                bounds.size(),
                child_placement_area(&target.container_rect()),
            );
            return Point::new(position.x - left, position.y - top);
        }
        let taken = |step: f64| {
            roots.iter().any(|r| {
                self.elements()
                    .iter()
                    .any(|e| e.kind == r.kind && e.x == r.x + step && e.y == r.y + step)
            })
        };
        let steps = (CANVAS_AREA.right() / GRID) as u32;
        let step = (0..steps)
            .map(|n| f64::from(n) * GRID)
            .find(|step| !taken(*step))
            .unwrap_or(0.0);
        Point::new(step, step)
    }

    /// The labels of `ids`, such as "CRM and Billing".
    fn labels_of(&self, ids: &[ElementId]) -> String {
        let labels: Vec<String> = ids
            .iter()
            .filter_map(|id| self.label_of(id).map(str::to_owned))
            .collect();
        list_labels(&labels)
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::geometry::Size;
    use crate::model::{KindInfo, NoColors, NoProps};
    use crate::text::EstimateText;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Kind {
        Group,
        Item,
    }

    struct Test;

    static PALETTE: [KindInfo<Kind>; 2] = [
        KindInfo {
            can_contain: &[Kind::Item],
            size: Some(Size::new(400.0, 200.0)),
            ..KindInfo::new(Kind::Group, "Group")
        },
        KindInfo {
            connects_to: &[Kind::Item],
            layer: 1,
            ..KindInfo::new(Kind::Item, "Item")
        },
    ];

    impl DiagramType for Test {
        const ID: DiagramTypeId = DiagramTypeId::Conceptual;
        type Kind = Kind;
        type Props = NoProps;
        type Arrow = NoProps;
        type Colors = NoColors;
        fn palette() -> &'static [KindInfo<Kind>] {
            &PALETTE
        }
    }

    static ESTIMATE: EstimateText = EstimateText;

    fn engine() -> Engine<Test> {
        let mut n = 0;
        Engine::new(Vec::new(), Vec::new(), NoColors {})
            .with_measure(&ESTIMATE)
            .with_ids(move || {
                n += 1;
                format!("id{n}")
            })
    }

    fn id(s: &str) -> ElementId {
        ElementId::from(s)
    }

    /// A group at (40, 40) holding two items with an arrow between them: id1 is the group,
    /// id2 and id3 the items, and id4 the arrow.
    fn group_of_two() -> Engine<Test> {
        let mut e = engine();
        e.palette_drop(Kind::Group, Point::new(240.0, 140.0));
        e.palette_drop(Kind::Item, Point::new(140.0, 120.0));
        e.palette_drop(Kind::Item, Point::new(340.0, 120.0));
        e.add_connection(&id("id2"), &id("id3"));
        e
    }

    fn rect(e: &Engine<Test>, s: &str) -> Rect {
        e.laid(&id(s)).expect("the element exists").rect
    }

    #[test]
    fn copies_paste_beside_the_original_with_what_they_hold() {
        let mut e = group_of_two();
        e.select(&id("id1"));
        let clip = e.copy_selection().expect("something is selected");
        assert_eq!(e.announcement, "Group copied.");
        assert_eq!(clip.elements.len(), 3);
        assert_eq!(clip.connections.len(), 1);

        e.paste(&clip);
        // The copy is a grid step down and right, with its items inside and their arrow.
        assert_eq!(e.selection, vec![id("id5")]);
        let original = rect(&e, "id1");
        let copy = rect(&e, "id5");
        assert_eq!((copy.x, copy.y), (original.x + GRID, original.y + GRID));
        let inside: Vec<&ElementId> = e
            .elements()
            .iter()
            .filter(|el| el.parent_id == Some(id("id5")))
            .map(|el| &el.id)
            .collect();
        assert_eq!(inside, vec![&id("id6"), &id("id7")]);
        assert_eq!(e.connections().len(), 2);
        let arrow = &e.connections()[1];
        assert_eq!(
            (&arrow.source_id, &arrow.target_id),
            (&id("id6"), &id("id7"))
        );
        assert!(e.announcement.starts_with("Group, ") && e.announcement.ends_with(" pasted."));

        // Pasting again goes one more step along.
        e.paste(&clip);
        let third = rect(&e, &e.selection[0].0.clone());
        assert_eq!(
            (third.x, third.y),
            (original.x + GRID * 2.0, original.y + GRID * 2.0)
        );
    }

    #[test]
    fn cut_elements_paste_back_where_they_were() {
        let mut e = group_of_two();
        let before = rect(&e, "id2");
        e.select(&id("id2"));
        let clip = e.cut_selection().expect("something is selected");
        assert_eq!(e.announcement, "Item cut.");
        assert!(e.element(&id("id2")).is_none());
        // Its arrow goes with it, but isn't on the clipboard: the other end wasn't copied.
        assert!(e.connections().is_empty());
        assert!(clip.connections.is_empty());

        e.paste(&clip);
        let pasted = e.selection[0].clone();
        assert_eq!(rect(&e, pasted.as_str()), before);
        assert_eq!(e.element(&pasted).unwrap().parent_id, Some(id("id1")));
    }

    #[test]
    fn cutting_a_container_takes_what_it_holds() {
        let mut e = group_of_two();
        e.select(&id("id1"));
        e.cut_selection();
        assert!(e.elements().is_empty());
        assert!(e.connections().is_empty());
        assert!(e.selection.is_empty());
    }

    #[test]
    fn pasting_with_a_container_selected_puts_them_in_it() {
        let mut e = group_of_two();
        e.palette_drop(Kind::Group, Point::new(840.0, 540.0));
        let target = e.selection[0].clone();
        e.select(&id("id2"));
        let clip = e.copy_selection().unwrap();
        e.select(&target);
        e.paste(&clip);
        let pasted = e.selection[0].clone();
        assert_eq!(e.element(&pasted).unwrap().parent_id, Some(target.clone()));
        assert!(rect(&e, target.as_str()).contains_rect(&rect(&e, pasted.as_str())));
    }

    #[test]
    fn nothing_selected_copies_nothing() {
        let mut e = group_of_two();
        e.clear_selection();
        assert!(e.copy_selection().is_none());
        assert!(e.cut_selection().is_none());
        assert_eq!(e.elements().len(), 3);
    }

    #[test]
    fn clipboard_text_reads_back_and_is_checked() {
        let mut e = group_of_two();
        e.select(&id("id1"));
        let text = e.copy_selection().unwrap().to_text();
        let read = Clip::<Test>::from_text(&text).expect("the text reads back");
        assert_eq!(read.elements.len(), 3);
        assert_eq!(read.connections.len(), 1);

        let refused = |text: &str| Clip::<Test>::from_text(text).err().unwrap();
        assert_eq!(
            refused("Hello"),
            "The clipboard doesn’t hold diagram elements."
        );
        assert_eq!(
            refused(&text.replace("\"conceptual\"", "\"physical\"")),
            "The elements were copied from a physical diagram, so they can only be pasted into one."
        );
        assert_eq!(
            refused(&text.replace("\"version\": 1", "\"version\": 2")),
            "The elements were copied from a newer version of ArchHiero. Update it and try again."
        );
        // The elements are checked as a file's are.
        assert_eq!(
            refused(&text.replace("\"kind\": \"item\"", "\"kind\": \"thing\"")),
            "“Item”: “kind” must be one of: group, item."
        );
    }
}
