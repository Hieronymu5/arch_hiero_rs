//! A diagram's tags and scenes.
//!
//! A tag is a named set of a diagram's elements, shown or hidden together. An element in no tag
//! always shows; one in tags shows while any of them does, and only while what holds it shows
//! too. An arrow shows while both its ends do.
//!
//! A sequence diagram's tags hold its messages instead (see
//! [`DiagramTypeId::tags_hold_arrows`](crate::model::DiagramTypeId::tags_hold_arrows)): its
//! participants are in none, and always show, and a message shows while any of its tags does.
//!
//! A scene is a named view of a diagram that shows the elements of one or more of its tags and
//! hides the rest of its tags' elements. Scenes are stepped through, by hand or played, while
//! viewing: viewing opens on the last, and Play steps through them from the first. Elements in
//! no tag show in every scene. A scene added without tags shows a tag of its own, named after
//! it ([`tag_for_scene`]).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::geometry::Rect;
use crate::stored::{StoredConnection, StoredContent, StoredDiagram, StoredElement};

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

/// The tag a new scene named `name` shows when it's given none: the tag of that name, ignoring
/// case, or a new one named as the scene, added at the end of `tags`. Returns the tags with it,
/// and its id.
pub fn tag_for_scene(name: &str, tags: &[DiagramTag]) -> (Vec<DiagramTag>, String) {
    let trimmed = name.trim();
    let lower = trimmed.to_lowercase();
    let mut tags = tags.to_vec();
    if let Some(existing) = tags.iter().find(|t| t.name.trim().to_lowercase() == lower) {
        let id = existing.id.clone();
        return (tags, id);
    }
    let id = new_id();
    tags.push(DiagramTag {
        id: id.clone(),
        name: trimmed.to_owned(),
    });
    (tags, id)
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

/// The `tags` property in `props`, an element's or an arrow's.
fn tags_in(props: &serde_json::Map<String, Value>) -> Vec<String> {
    match props.get("tags") {
        Some(Value::Array(ids)) => ids
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

/// Sets the `tags` property in `props`, leaving it out when there are none.
fn set_tags_in(props: &mut serde_json::Map<String, Value>, tags: Vec<String>) {
    if tags.is_empty() {
        props.remove("tags");
    } else {
        props.insert(
            "tags".into(),
            Value::Array(tags.into_iter().map(Value::from).collect()),
        );
    }
}

/// The tags an element as stored is in: its `tags` property.
pub fn element_tags(element: &StoredElement) -> Vec<String> {
    tags_in(&element.props)
}

/// The tags an arrow as stored is in, such as a sequence diagram's message: its `tags`
/// property.
pub fn connection_tags(connection: &StoredConnection) -> Vec<String> {
    tags_in(&connection.props)
}

/// Sets the tags an arrow as stored is in, leaving the property out when there are none.
pub fn set_connection_tags(connection: &mut StoredConnection, tags: Vec<String>) {
    set_tags_in(&mut connection.props, tags);
}

/// `connections` taken out of the tag `id`.
pub fn untag_connections(connections: &mut [StoredConnection], id: &str) {
    for connection in connections {
        let tags = connection_tags(connection);
        if tags.iter().any(|t| t == id) {
            set_connection_tags(connection, tags.into_iter().filter(|t| t != id).collect());
        }
    }
}

/// An element a scene shows, for finding the section it's about: where it is, whether it's a
/// section, and whether it's in a tag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShownElement {
    pub rect: Rect,
    pub section: bool,
    pub tagged: bool,
}

/// Where in `shown`, the elements a scene shows, the leftmost section the scene is active in
/// is: one in a tag itself, or over a tagged element the scene shows that reaches under no
/// other section. Elements in no tag show in every scene, and one stretched across several
/// sections, such as a bar along the diagram, is about none of them, so neither makes a section
/// active.
pub fn leftmost_active_section(shown: &[ShownElement]) -> Option<usize> {
    let sections: Vec<usize> = (0..shown.len()).filter(|&i| shown[i].section).collect();
    let mut active: BTreeSet<usize> = sections
        .iter()
        .copied()
        .filter(|&i| shown[i].tagged)
        .collect();
    for element in shown.iter().filter(|e| !e.section && e.tagged) {
        let under: Vec<usize> = sections
            .iter()
            .copied()
            .filter(|&i| {
                let s = shown[i].rect;
                element.rect.x < s.right() && element.rect.right() > s.x
            })
            .collect();
        if let [only] = under[..] {
            active.insert(only);
        }
    }
    active
        .into_iter()
        .min_by(|&a, &b| shown[a].rect.x.total_cmp(&shown[b].rect.x))
}

/// Sets the tags an element as stored is in, leaving the property out when there are none.
pub fn set_element_tags(element: &mut StoredElement, tags: Vec<String>) {
    set_tags_in(&mut element.props, tags);
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

/// How many elements are in each tag, by its id; or with `arrows`, where tags hold arrows, as a
/// sequence diagram's do, how many arrows.
pub fn tag_counts(content: &StoredContent, arrows: bool) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    let each: Vec<Vec<String>> = if arrows {
        content.connections.iter().map(connection_tags).collect()
    } else {
        content.elements.iter().map(element_tags).collect()
    };
    for tag in each.into_iter().flatten() {
        *counts.entry(tag).or_insert(0) += 1;
    }
    counts
}

/// The ids of the arrows their own tags hide while the tags `hidden` are, where tags hold
/// arrows: those in tags, every one of them hidden. One in no tag always shows. Each arrow is
/// given as its id and its tags.
pub fn hidden_arrow_ids<'a>(
    arrows: impl IntoIterator<Item = (&'a str, &'a [String])>,
    hidden: &BTreeSet<String>,
) -> BTreeSet<String> {
    arrows
        .into_iter()
        .filter(|(_, tags)| !tags.is_empty() && tags.iter().all(|t| hidden.contains(t)))
        .map(|(id, _)| id.to_owned())
        .collect()
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
/// name, in the order they play. A tag left out is deleted: its elements, or its messages, are
/// taken out of it, and the scenes that showed only it go. Nothing changes when a name or an id
/// is wrong.
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
        untag_connections(&mut diagram.content.connections, id);
        diagram.scenes = scenes_without_tag(&diagram.scenes, id);
    }
    // Elements', arrows', and scenes' tags follow the tags' new order.
    for element in &mut diagram.content.elements {
        let ordered = in_tag_order(&element_tags(element), &tags);
        set_element_tags(element, ordered);
    }
    for connection in &mut diagram.content.connections {
        let ordered = in_tag_order(&connection_tags(connection), &tags);
        set_connection_tags(connection, ordered);
    }
    for scene in &mut diagram.scenes {
        scene.tags = in_tag_order(&scene.tags, &tags);
    }
    diagram.tags = tags;
    Ok(())
}

/// Replaces `diagram`'s scenes with `given`, each an id to keep a scene (or none to add one), a
/// name, and the tags it shows, by id or name, in the order they're stepped through. A scene
/// given no tags shows a tag of its own: the tag of its name, or a new one named after it,
/// added to the diagram's tags. Nothing changes when one is wrong.
pub fn replace_scenes(
    diagram: &mut StoredDiagram,
    given: &[(Option<String>, String, Vec<String>)],
) -> Result<(), String> {
    let mut scenes: Vec<DiagramScene> = Vec::new();
    let mut tags = diagram.tags.clone();
    for (id, name, shown) in given {
        let mut ids = Vec::new();
        for text in shown {
            let wanted = text.trim().to_lowercase();
            let tag = tags
                .iter()
                .find(|t| t.id == *text || t.name.trim().to_lowercase() == wanted)
                .ok_or_else(|| format!("Scene “{}”: there's no tag “{text}”.", name.trim()))?;
            ids.push(tag.id.clone());
        }
        // Its own tag's name is checked as the scene's, below; nothing is kept if it's wrong.
        if ids.is_empty() && !name.trim().is_empty() {
            let (with_own, own) = tag_for_scene(name, &tags);
            tags = with_own;
            ids.push(own);
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
        scenes.push(scene_of(id, name, &ids, &tags));
    }
    diagram.tags = tags;
    diagram.scenes = scenes;
    Ok(())
}

/// The smallest rect around all of `rects`, or nothing when there are none.
pub fn bounds_of(rects: impl IntoIterator<Item = Rect>) -> Option<Rect> {
    rects.into_iter().reduce(|a, b| {
        let (left, top) = (a.x.min(b.x), a.y.min(b.y));
        let (right, bottom) = (a.right().max(b.right()), a.bottom().max(b.bottom()));
        Rect::new(left, top, right - left, bottom - top)
    })
}

/// What the camera frames for a scene, given each element it shows: its area, and whether
/// another scene hides it. What it shows that another scene hides, as what every scene shows,
/// such as the application they're all in, would keep the camera from moving; all it shows,
/// when it shows nothing of its own; nothing when it shows nothing.
pub fn scene_frame(shown: &[(Rect, bool)]) -> Option<Rect> {
    bounds_of(shown.iter().filter(|(_, own)| *own).map(|(r, _)| *r))
        .or_else(|| bounds_of(shown.iter().map(|(r, _)| *r)))
}

/// How a scene is built from the top down, as a line moving down the canvas reaches what it
/// adds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SweepPace {
    /// When the line starts moving down, in seconds.
    pub start: f64,
    /// How fast it moves, in canvas pixels a second, unless that would take longer than
    /// `longest`.
    pub speed: f64,
    /// The longest it takes to pass from the topmost item's top to the lowest item's top, in
    /// seconds.
    pub longest: f64,
}

/// A line moving down the canvas, revealing what a scene adds as it passes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sweep {
    /// Where it starts: the topmost item's top.
    pub top: f64,
    /// The lowest item's bottom, which it has passed once it's done.
    pub bottom: f64,
    /// When it starts moving, in seconds.
    pub start: f64,
    /// How fast it moves, in canvas pixels a second.
    pub speed: f64,
}

impl Sweep {
    /// The line over `spans`, each an item's top and bottom: it starts at the topmost top, and
    /// goes faster on a tall scene, so it never takes longer than `pace.longest` to reach the
    /// lowest top. Nothing without spans.
    pub fn over(spans: &[(f64, f64)], pace: SweepPace) -> Option<Self> {
        let top = spans.iter().map(|s| s.0).reduce(f64::min)?;
        let last = spans.iter().map(|s| s.0).fold(top, f64::max);
        let bottom = spans.iter().map(|s| s.1).fold(last, f64::max);
        Some(Self {
            top,
            bottom,
            start: pace.start,
            speed: pace.speed.max((last - top) / pace.longest),
        })
    }

    /// Where the line is `elapsed` seconds after the scene was shown.
    pub fn line_at(&self, elapsed: f64) -> f64 {
        self.top + (elapsed - self.start).max(0.0) * self.speed
    }

    /// When the line has passed everything, `trail` seconds behind it included, in seconds
    /// after the scene was shown.
    pub fn end(&self, trail: f64) -> f64 {
        self.start + (self.bottom - self.top) / self.speed + trail
    }
}

/// CSS's `ease`, `cubic-bezier(0.25, 0.1, 0.25, 1)`: quick to start, slowing to a stop, as
/// fades in a browser go.
pub fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    // The curve's x and y for a parameter `u`, from control points (0.25, 0.1) and (0.25, 1).
    let bezier = |u: f64, p1: f64, p2: f64| {
        3.0 * (1.0 - u) * (1.0 - u) * u * p1 + 3.0 * (1.0 - u) * u * u * p2 + u * u * u
    };
    // Find the parameter whose x is `t` by halving: x rises steadily with it.
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..40 {
        let mid = f64::midpoint(low, high);
        if bezier(mid, 0.25, 0.25) < t {
            low = mid;
        } else {
            high = mid;
        }
    }
    bezier(f64::midpoint(low, high), 0.1, 1.0)
}

/// Eases in and out: slow to start, fast in the middle, slow to stop.
pub fn ease_in_out(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_moving_down_reaches_each_item_and_goes_faster_on_a_tall_scene() {
        let pace = SweepPace {
            start: 0.5,
            speed: 500.0,
            longest: 2.0,
        };
        let sweep = Sweep::over(&[(100.0, 200.0), (200.0, 250.0), (400.0, 460.0)], pace).unwrap();
        // At 500 px a second, from the topmost top at 100.
        assert_eq!(
            (sweep.top, sweep.bottom, sweep.speed),
            (100.0, 460.0, 500.0)
        );
        assert_eq!(sweep.line_at(0.2), 100.0);
        assert!((sweep.line_at(0.7) - 200.0).abs() < 1e-9);
        assert!((sweep.end(0.0) - (0.5 + 0.72)).abs() < 1e-9);
        // 4000 px in 2 s: 2000 px a second.
        let tall = Sweep::over(&[(0.0, 100.0), (4000.0, 4100.0)], pace).unwrap();
        assert_eq!(tall.speed, 2000.0);
        assert!((tall.line_at(2.5) - 4000.0).abs() < 1e-9);
        assert_eq!(Sweep::over(&[], pace), None);
    }

    #[test]
    fn a_scene_frames_what_it_shows_of_its_own() {
        let a = Rect::new(100.0, 50.0, 40.0, 40.0);
        let b = Rect::new(20.0, 200.0, 60.0, 30.0);
        assert_eq!(bounds_of([a, b]), Some(Rect::new(20.0, 50.0, 120.0, 180.0)));
        assert_eq!(bounds_of([]), None);
        // Only what another scene hides, when there is any; otherwise all it shows.
        assert_eq!(scene_frame(&[(a, true), (b, false)]), Some(a));
        assert_eq!(scene_frame(&[(a, false), (b, false)]), bounds_of([a, b]));
        assert_eq!(scene_frame(&[]), None);
    }

    #[test]
    fn css_ease_starts_quickly_and_slows_to_a_stop() {
        assert!(ease(0.0).abs() < 1e-9 && (ease(1.0) - 1.0).abs() < 1e-9);
        // As browsers draw it: about 0.8 of the way at half the time.
        assert!((ease(0.5) - 0.8024).abs() < 1e-3, "{}", ease(0.5));
        assert!(ease(0.25) > 0.25 && ease(0.75) > 0.75);
    }

    #[test]
    fn easing_starts_and_stops_slowly() {
        assert_eq!(ease_in_out(0.0), 0.0);
        assert_eq!(ease_in_out(0.5), 0.5);
        assert_eq!(ease_in_out(1.0), 1.0);
        assert!(ease_in_out(0.1) < 0.1 && ease_in_out(0.9) > 0.9);
    }

    #[test]
    fn the_leftmost_section_over_what_a_scene_shows_of_its_tags_is_found() {
        let at = |x: f64, width: f64, section: bool, tagged: bool| ShownElement {
            rect: Rect::new(x, 0.0, width, 40.0),
            section,
            tagged,
        };
        // Book, Explore, and Travel, out of order.
        let sections = [
            at(1000.0, 400.0, true, false),
            at(0.0, 400.0, true, false),
            at(2000.0, 400.0, true, false),
        ];
        let with = |more: &[ShownElement]| {
            let mut all = sections.to_vec();
            all.extend_from_slice(more);
            leftmost_active_section(&all)
        };
        // Elements in no tag show in every scene, so they don't count.
        assert_eq!(
            with(&[
                at(100.0, 100.0, false, false),
                at(1950.0, 200.0, false, true)
            ]),
            Some(2)
        );
        // An element under one section makes it active; one across several, none of them.
        assert_eq!(with(&[at(1300.0, 100.0, false, true)]), Some(0));
        assert_eq!(
            with(&[at(0.0, 2400.0, false, true), at(2100.0, 100.0, false, true)]),
            Some(2)
        );
        assert_eq!(with(&[at(300.0, 900.0, false, true)]), None);
        // A section in a tag is active itself.
        assert_eq!(
            leftmost_active_section(&[at(0.0, 400.0, true, true), at(1000.0, 400.0, true, false)]),
            Some(0)
        );
        assert_eq!(leftmost_active_section(&sections), None);
    }

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

    #[test]
    fn a_scene_without_tags_shows_a_tag_of_its_own() {
        // The tag of its name, ignoring case, when there is one.
        let (same, id) = tag_for_scene(" payments ", &tags());
        assert_eq!((same, id.as_str()), (tags(), "pay"));
        // Otherwise a new one, named as the scene, at the end.
        let (added, id) = tag_for_scene(" Step 1 ", &tags());
        assert_eq!(added.len(), 4);
        assert_eq!(added[3], tag(&id, "Step 1"));
    }

    #[test]
    fn hides_an_arrow_whose_every_tag_is_hidden() {
        let s = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<String>>();
        let (core, both, none) = (s(&["core"]), s(&["core", "pay"]), s(&[]));
        let arrows = [
            ("a", core.as_slice()),
            ("b", both.as_slice()),
            ("c", none.as_slice()),
        ];
        let hidden = |h: &[&str]| {
            let h = h.iter().map(|s| (*s).to_owned()).collect();
            hidden_arrow_ids(arrows.iter().copied(), &h)
                .into_iter()
                .collect::<Vec<_>>()
        };
        assert_eq!(hidden(&["core"]), ["a"]);
        assert_eq!(hidden(&["core", "pay"]), ["a", "b"]);
        assert!(hidden(&[]).is_empty());
    }

    fn sequence_diagram() -> StoredDiagram {
        use crate::model::{DiagramMeta, DiagramTypeId};
        let connection = StoredConnection::from_json(&serde_json::json!({
            "id": "m", "sourceId": "a", "targetId": "b", "tags": ["pay", "core"]
        }))
        .unwrap();
        StoredDiagram {
            meta: DiagramMeta {
                id: "d".into(),
                diagram_type: DiagramTypeId::Sequence,
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
            scenes: vec![scene("a", &["core"])],
            content: StoredContent {
                elements: Vec::new(),
                connections: vec![connection],
            },
        }
    }

    #[test]
    fn a_sequence_diagrams_messages_are_counted_and_untagged() {
        let mut diagram = sequence_diagram();
        let counts = tag_counts(&diagram.content, true);
        assert_eq!(counts.get("pay"), Some(&1));
        assert!(tag_counts(&diagram.content, false).is_empty());
        let given = [(Some("pay".to_owned()), "Payments".to_owned())];
        replace_tags(&mut diagram, &given).unwrap();
        assert_eq!(connection_tags(&diagram.content.connections[0]), ["pay"]);
        assert!(diagram.scenes.is_empty());
        untag_connections(&mut diagram.content.connections, "pay");
        assert!(!diagram.content.connections[0].props.contains_key("tags"));
    }

    #[test]
    fn replacing_scenes_gives_one_without_tags_a_tag_of_its_own() {
        let mut diagram = sequence_diagram();
        replace_scenes(
            &mut diagram,
            &[
                (None, "Core".into(), Vec::new()),
                (None, "Sign in".into(), Vec::new()),
                (None, "All".into(), vec!["core".into(), "sign in".into()]),
            ],
        )
        .unwrap();
        assert_eq!(diagram.tags.len(), 4);
        let own = diagram.tags[3].id.clone();
        assert_eq!(diagram.tags[3].name, "Sign in");
        let shown: Vec<Vec<String>> = diagram.scenes.iter().map(|s| s.tags.clone()).collect();
        assert_eq!(
            shown,
            [
                vec!["core".to_owned()],
                vec![own.clone()],
                vec!["core".to_owned(), own]
            ]
        );
        // Nothing is kept when one is wrong, its own tag included.
        let before = diagram.clone();
        assert!(
            replace_scenes(
                &mut diagram,
                &[
                    (None, "New".into(), Vec::new()),
                    (None, "Bad".into(), vec!["nope".into()])
                ]
            )
            .is_err()
        );
        assert_eq!(diagram, before);
    }
}
