//! A diagram's tags and scenes.
//!
//! A tag is a named set of a diagram's elements, shown or hidden together. An element in no tag
//! always shows; one in tags shows while any of them does, and only while what holds it shows
//! too. An arrow shows while both its ends do.
//!
//! A scene is a named view of a diagram that shows the elements of one or more of its tags and
//! hides the rest of its tags' elements. Scenes are stepped through, by hand or played, while
//! viewing. Elements in no tag show in every scene.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::stored::{StoredDiagram, StoredElement};

/// The longest a tag's or a scene's name can be, in characters.
pub const TAG_NAME_MAX: usize = 40;

/// A named set of a diagram's elements, shown or hidden together, in the order the tags play.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagramTag {
    pub id: String,
    /// Unique within the diagram, ignoring case.
    pub name: String,
}

/// A named view of a diagram showing the elements of some of its tags, in the order the scenes
/// are stepped through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagramScene {
    pub id: String,
    /// Unique within the diagram, ignoring case.
    pub name: String,
    /// The ids of the diagram's tags the scene shows, each once, at least one, in the order of
    /// the diagram's tags.
    pub tags: Vec<String>,
}

/// A new random id for a tag or a scene.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// What's wrong with `name` as the name of a `kind` of thing, such as a tag or a scene, among
/// `named` (each an id and a name), other than the one `except`, if anything: it must be there,
/// at most [`TAG_NAME_MAX`] characters long, and not already taken, ignoring case.
pub fn name_problem<'a>(
    name: &str,
    named: impl IntoIterator<Item = (&'a str, &'a str)>,
    kind: &str,
    except: Option<&str>,
) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Some("Enter a name.".into());
    }
    if trimmed.chars().count() > TAG_NAME_MAX {
        return Some(format!(
            "A {kind}'s name must be {TAG_NAME_MAX} characters or fewer."
        ));
    }
    let lower = trimmed.to_lowercase();
    named
        .into_iter()
        .any(|(id, other)| Some(id) != except && other.trim().to_lowercase() == lower)
        .then(|| format!("There's already a {kind} called {trimmed}."))
}

/// What's wrong with `name` as the name of a tag among `tags`, other than `except`, if anything.
pub fn tag_name_problem(name: &str, tags: &[DiagramTag], except: Option<&str>) -> Option<String> {
    name_problem(
        name,
        tags.iter().map(|t| (t.id.as_str(), t.name.as_str())),
        "tag",
        except,
    )
}

/// What's wrong with a scene named `name` showing `tags`, among `scenes`, other than `except`:
/// its name's problem, as a tag's would be, and then that it shows no tag. `None` when it's fine.
pub fn scene_problem(
    name: &str,
    tags: &[String],
    scenes: &[DiagramScene],
    except: Option<&str>,
) -> Option<(Option<String>, Option<String>)> {
    let name = name_problem(
        name,
        scenes.iter().map(|s| (s.id.as_str(), s.name.as_str())),
        "scene",
        except,
    );
    let tags = tags
        .is_empty()
        .then(|| "Choose at least one tag.".to_owned());
    (name.is_some() || tags.is_some()).then_some((name, tags))
}

/// `ids` in the order of the diagram's `tags`, each once, leaving out any that isn't one.
pub fn in_tag_order(ids: &[String], tags: &[DiagramTag]) -> Vec<String> {
    tags.iter()
        .filter(|t| ids.contains(&t.id))
        .map(|t| t.id.clone())
        .collect()
}

/// The scene `id`, named `name` (trimmed), showing `shown` in the order of the diagram's `tags`.
pub fn scene_of(id: String, name: &str, shown: &[String], tags: &[DiagramTag]) -> DiagramScene {
    DiagramScene {
        id,
        name: name.trim().to_owned(),
        tags: in_tag_order(shown, tags),
    }
}

/// `scenes` with `scene` in place of the one with its id, or added at the end when there's none.
pub fn with_scene(scenes: &[DiagramScene], scene: DiagramScene) -> Vec<DiagramScene> {
    let mut scenes = scenes.to_vec();
    match scenes.iter_mut().find(|s| s.id == scene.id) {
        Some(found) => *found = scene,
        None => scenes.push(scene),
    }
    scenes
}

/// `items` with the one at `from` moved to `to`, the others keeping their order. Unchanged when
/// either is out of range.
pub fn moved<T: Clone>(items: &[T], from: usize, to: usize) -> Vec<T> {
    let mut items = items.to_vec();
    if from < items.len() && to < items.len() {
        let item = items.remove(from);
        items.insert(to, item);
    }
    items
}

/// `scenes` without the tag `id`, leaving out any that showed only it.
pub fn scenes_without_tag(scenes: &[DiagramScene], id: &str) -> Vec<DiagramScene> {
    scenes
        .iter()
        .map(|s| DiagramScene {
            tags: s.tags.iter().filter(|t| *t != id).cloned().collect(),
            ..s.clone()
        })
        .filter(|s| !s.tags.is_empty())
        .collect()
}

/// The scenes that show only the tag `id`, so go when it does.
pub fn scenes_only_of<'a>(scenes: &'a [DiagramScene], id: &str) -> Vec<&'a DiagramScene> {
    scenes
        .iter()
        .filter(|s| s.tags.len() == 1 && s.tags[0] == id)
        .collect()
}

/// The ids of the tags `scene` hides: every one of `tags` it doesn't show.
pub fn hidden_in_scene(tags: &[DiagramTag], scene: &DiagramScene) -> BTreeSet<String> {
    tags.iter()
        .filter(|t| !scene.tags.contains(&t.id))
        .map(|t| t.id.clone())
        .collect()
}

/// The tags an element as stored is in: its `tags` property.
pub fn element_tags(element: &StoredElement) -> Vec<String> {
    match element.props.get("tags") {
        Some(Value::Array(ids)) => ids
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

/// Sets the tags an element as stored is in, leaving the property out when there are none.
pub fn set_element_tags(element: &mut StoredElement, tags: Vec<String>) {
    if tags.is_empty() {
        element.props.remove("tags");
    } else {
        element.props.insert(
            "tags".into(),
            Value::Array(tags.into_iter().map(Value::from).collect()),
        );
    }
}

/// `elements` taken out of the tag `id`.
pub fn untag_elements(elements: &mut [StoredElement], id: &str) {
    for element in elements {
        let tags = element_tags(element);
        if tags.iter().any(|t| t == id) {
            set_element_tags(element, tags.into_iter().filter(|t| t != id).collect());
        }
    }
}

/// How many elements are in each tag, by its id.
pub fn tag_counts(elements: &[StoredElement]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for element in elements {
        for tag in element_tags(element) {
            *counts.entry(tag).or_insert(0) += 1;
        }
    }
    counts
}

/// The ids of the elements hidden while the tags `hidden` are: those whose every tag is hidden,
/// and those held, however deep, by one that is. Each element is given as its id, its parent's
/// id, and its tags.
pub fn hidden_element_ids<'a>(
    elements: impl IntoIterator<Item = (&'a str, Option<&'a str>, &'a [String])>,
    hidden: &BTreeSet<String>,
) -> BTreeSet<String> {
    if hidden.is_empty() {
        return BTreeSet::new();
    }
    let elements: Vec<_> = elements.into_iter().collect();
    let by_id: BTreeMap<&str, (Option<&str>, &[String])> = elements
        .iter()
        .map(|(id, parent, tags)| (*id, (*parent, *tags)))
        .collect();
    let own = |tags: &[String]| !tags.is_empty() && tags.iter().all(|t| hidden.contains(t));
    elements
        .iter()
        .filter(|(id, _, _)| {
            // Up through what holds it, stopping at a loop, which a checked diagram never has.
            let mut seen = BTreeSet::new();
            let mut at = Some(*id);
            while let Some(current) = at {
                if !seen.insert(current) {
                    return false;
                }
                let Some((parent, tags)) = by_id.get(current) else {
                    return false;
                };
                if own(tags) {
                    return true;
                }
                at = *parent;
            }
            false
        })
        .map(|(id, _, _)| (*id).to_owned())
        .collect()
}

/// Replaces `diagram`'s tags with `given`, each an id to keep a tag (or none to add one) and a
/// name, in the order they play. A tag left out is deleted: its elements are taken out of it,
/// and the scenes that showed only it go. Nothing changes when a name or an id is wrong.
pub fn replace_tags(
    diagram: &mut StoredDiagram,
    given: &[(Option<String>, String)],
) -> Result<(), String> {
    let mut tags: Vec<DiagramTag> = Vec::new();
    for (id, name) in given {
        if let Some(problem) = tag_name_problem(name, &tags, None) {
            return Err(format!("Tag “{}”: {problem}", name.trim()));
        }
        let id = match id {
            Some(id) if tags.iter().any(|t| t.id == *id) => {
                return Err(format!("The tag id {id} is given twice."));
            }
            Some(id) if !diagram.tags.iter().any(|t| t.id == *id) => {
                return Err(format!(
                    "There's no tag with the id {id}; leave the id out to add one."
                ));
            }
            Some(id) => id.clone(),
            None => new_id(),
        };
        tags.push(DiagramTag {
            id,
            name: name.trim().to_owned(),
        });
    }
    let gone: Vec<String> = diagram
        .tags
        .iter()
        .filter(|t| !tags.iter().any(|k| k.id == t.id))
        .map(|t| t.id.clone())
        .collect();
    for id in &gone {
        untag_elements(&mut diagram.content.elements, id);
        diagram.scenes = scenes_without_tag(&diagram.scenes, id);
    }
    // Elements' and scenes' tags follow the tags' new order.
    for element in &mut diagram.content.elements {
        let ordered = in_tag_order(&element_tags(element), &tags);
        set_element_tags(element, ordered);
    }
    for scene in &mut diagram.scenes {
        scene.tags = in_tag_order(&scene.tags, &tags);
    }
    diagram.tags = tags;
    Ok(())
}

/// Replaces `diagram`'s scenes with `given`, each an id to keep a scene (or none to add one), a
/// name, and the tags it shows, by id or name, in the order they're stepped through. Nothing
/// changes when one is wrong.
pub fn replace_scenes(
    diagram: &mut StoredDiagram,
    given: &[(Option<String>, String, Vec<String>)],
) -> Result<(), String> {
    let mut scenes: Vec<DiagramScene> = Vec::new();
    for (id, name, shown) in given {
        let mut ids = Vec::new();
        for text in shown {
            let wanted = text.trim().to_lowercase();
            let tag = diagram
                .tags
                .iter()
                .find(|t| t.id == *text || t.name.trim().to_lowercase() == wanted)
                .ok_or_else(|| format!("Scene “{}”: there's no tag “{text}”.", name.trim()))?;
            ids.push(tag.id.clone());
        }
        if let Some((name_problem, tags_problem)) = scene_problem(name, &ids, &scenes, None) {
            let problem = name_problem.or(tags_problem).unwrap_or_default();
            return Err(format!("Scene “{}”: {problem}", name.trim()));
        }
        let id = id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if scenes.iter().any(|s| s.id == id) {
            return Err(format!("The scene id {id} is given twice."));
        }
        scenes.push(scene_of(id, name, &ids, &diagram.tags));
    }
    diagram.scenes = scenes;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(id: &str, name: &str) -> DiagramTag {
        DiagramTag {
            id: id.into(),
            name: name.into(),
        }
    }

    fn tags() -> Vec<DiagramTag> {
        vec![
            tag("core", "Core"),
            tag("pay", "Payments"),
            tag("mail", "Notifications"),
        ]
    }

    fn scene(id: &str, shown: &[&str]) -> DiagramScene {
        DiagramScene {
            id: id.into(),
            name: id.into(),
            tags: shown.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    #[test]
    fn a_tag_is_named_with_something_not_too_long_and_not_taken() {
        let tags = tags();
        assert_eq!(
            tag_name_problem("  ", &tags, None).as_deref(),
            Some("Enter a name.")
        );
        assert_eq!(
            tag_name_problem(&"x".repeat(41), &tags, None).as_deref(),
            Some("A tag's name must be 40 characters or fewer.")
        );
        assert_eq!(tag_name_problem(&"x".repeat(40), &tags, None), None);
        assert_eq!(
            tag_name_problem(" payments ", &tags, None).as_deref(),
            Some("There's already a tag called payments.")
        );
        // Renaming a tag can keep its own name, or change its case.
        assert_eq!(tag_name_problem("PAYMENTS", &tags, Some("pay")), None);
    }

    #[test]
    fn moves_an_item_keeping_the_others_in_order() {
        let ids = |t: Vec<DiagramTag>| t.into_iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(ids(moved(&tags(), 2, 0)), ["mail", "core", "pay"]);
        assert_eq!(ids(moved(&tags(), 0, 1)), ["pay", "core", "mail"]);
        assert_eq!(ids(moved(&tags(), 0, 3)), ["core", "pay", "mail"]);
    }

    #[test]
    fn hides_an_element_whose_every_tag_is_hidden_and_what_it_holds() {
        let s = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<String>>();
        let (core, pay, both, none) = (s(&["core"]), s(&["pay"]), s(&["core", "pay"]), s(&[]));
        let elements = [
            ("free", None, none.as_slice()),
            ("box", None, core.as_slice()),
            ("inside", Some("box"), none.as_slice()),
            ("deeper", Some("inside"), pay.as_slice()),
            ("both", None, both.as_slice()),
        ];
        let hidden = |h: &[&str]| {
            let h = h.iter().map(|s| (*s).to_owned()).collect();
            hidden_element_ids(elements.iter().copied(), &h)
                .into_iter()
                .collect::<Vec<_>>()
        };
        assert!(hidden(&[]).is_empty());
        assert_eq!(hidden(&["core"]), ["box", "deeper", "inside"]);
        assert_eq!(hidden(&["pay"]), ["deeper"]);
        assert_eq!(
            hidden(&["core", "pay"]),
            ["both", "box", "deeper", "inside"]
        );
    }

    #[test]
    fn a_scene_needs_a_free_name_and_a_tag() {
        let scenes = [scene("Phase 1", &["core"])];
        assert_eq!(
            scene_problem("phase 1", &[], &scenes, None),
            Some((
                Some("There's already a scene called phase 1.".into()),
                Some("Choose at least one tag.".into())
            ))
        );
        assert_eq!(
            scene_problem("Phase 1", &["core".into()], &scenes, Some("Phase 1")),
            None
        );
    }

    #[test]
    fn a_scene_shows_its_tags_in_the_tags_order_and_hides_the_rest() {
        let tags = tags();
        let made = scene_of(
            "s".into(),
            " Both ",
            &["mail".into(), "core".into(), "gone".into()],
            &tags,
        );
        assert_eq!(made.name, "Both");
        assert_eq!(made.tags, ["core", "mail"]);
        assert_eq!(
            hidden_in_scene(&tags, &made)
                .into_iter()
                .collect::<Vec<_>>(),
            ["pay"]
        );
        let replaced = with_scene(std::slice::from_ref(&made), scene("s", &["pay"]));
        assert_eq!(replaced, [scene("s", &["pay"])]);
        assert_eq!(with_scene(&[], made.clone()), [made]);
    }

    #[test]
    fn deleting_a_tag_drops_the_scenes_that_showed_only_it() {
        let scenes = [scene("a", &["core"]), scene("b", &["core", "pay"])];
        assert_eq!(scenes_without_tag(&scenes, "core"), [scene("b", &["pay"])]);
        assert_eq!(scenes_only_of(&scenes, "core"), [&scenes[0]]);
    }

    #[test]
    fn replacing_tags_untags_elements_and_drops_scenes_of_tags_gone() {
        use crate::model::{DiagramMeta, DiagramTypeId};
        use crate::stored::StoredContent;
        let element = StoredElement::from_json(&serde_json::json!({
            "id": "e", "kind": "actor", "label": "E", "x": 0, "y": 0, "tags": ["pay", "core"]
        }))
        .unwrap();
        let mut diagram = StoredDiagram {
            meta: DiagramMeta {
                id: "d".into(),
                diagram_type: DiagramTypeId::Conceptual,
                name: "D".into(),
                description: None,
                created_at: String::new(),
                updated_at: String::new(),
            },
            colors: BTreeMap::new(),
            line_weight: None,
            canvas: None,
            subtype: None,
            tags: tags(),
            scenes: vec![scene("a", &["core"]), scene("b", &["core", "pay"])],
            content: StoredContent {
                elements: vec![element],
                connections: Vec::new(),
            },
        };
        let given = [
            (Some("pay".to_owned()), "Billing".to_owned()),
            (None, "New".to_owned()),
        ];
        replace_tags(&mut diagram, &given).unwrap();
        assert_eq!(diagram.tags[0], tag("pay", "Billing"));
        assert_eq!(diagram.tags[1].name, "New");
        assert_eq!(element_tags(&diagram.content.elements[0]), ["pay"]);
        assert_eq!(diagram.scenes, [scene("b", &["pay"])]);
        let twice = [(None, "x".to_owned()), (None, "X".to_owned())];
        assert_eq!(
            replace_tags(&mut diagram, &twice),
            Err("Tag “X”: There's already a tag called X.".into())
        );
        replace_scenes(
            &mut diagram,
            &[(None, "Only".into(), vec!["billing".into()])],
        )
        .unwrap();
        assert_eq!(diagram.scenes[0].tags, ["pay"]);
        assert!(replace_scenes(&mut diagram, &[(None, "S".into(), vec!["nope".into()])]).is_err());
    }
}
