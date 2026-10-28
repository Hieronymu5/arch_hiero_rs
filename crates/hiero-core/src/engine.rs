//! The editing engine every diagram type shares: elements added from a palette, moved,
//! resized, held in containers that grow to fit them, connected by arrows, and removed, with
//! what changed described for screen readers. A port of the prototype's `ElementEditor`.
//!
//! Each diagram type implements [`DiagramType`]. Its hooks receive the engine and can call the
//! `base_*` methods for the default behaviour, as the prototype's overrides call `super`.

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::file_check::{DescriptionRule, Field};
use crate::geometry::{CANVAS_HEIGHT, CANVAS_WIDTH, GRID, Point, Rect, Size, fmt_num, snap};
use crate::layout::{
    CANVAS, CANVAS_AREA, ContainerRect, MIN_CONTAINER_SIZE, MIN_ELEMENT_SIZE, child_placement_area,
    clamp_into, content_area, find_free_position, fit_container, resize_element,
    resize_from_top_left, snap_to_canvas,
};
use crate::model::{
    ColorScheme, Connection, ConnectionId, ContainerLook, DiagramTypeId, Element, ElementId,
    KindInfo, Resizable,
};
use crate::text::{MeasureText, font_metrics};

/// A diagram type's element.
pub type El<T> = Element<<T as DiagramType>::Kind, <T as DiagramType>::Props>;
/// A diagram type's arrow.
pub type Conn<T> = Connection<<T as DiagramType>::Arrow>;

/// Where an element goes: its position and the container it sits in, if any.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub position: Point,
    pub parent_id: Option<ElementId>,
}

/// The elements added when an element is placed, new one first, and the elements already
/// there if the type changed them.
pub struct Created<T: DiagramType> {
    pub added: Vec<El<T>>,
    pub existing: Option<Vec<El<T>>>,
}

/// The smallest an element can be resized to, and what it holds that it must stay around.
#[derive(Debug, Clone, PartialEq)]
pub struct ResizeLimits {
    pub min_size: Size,
    pub children: Vec<Rect>,
}

/// The corner a resize handle sits on, which moves when it's dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Corner {
    TopLeft,
    BottomRight,
}

/// An arrow key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

impl Dir {
    /// The step it moves an element by: a grid square, or five with Shift.
    pub fn offset(self, shift: bool) -> Point {
        let step = if shift { GRID * 5.0 } else { GRID };
        match self {
            Self::Left => Point::new(-step, 0.0),
            Self::Right => Point::new(step, 0.0),
            Self::Up => Point::new(0.0, -step),
            Self::Down => Point::new(0.0, step),
        }
    }

    pub const fn is_across(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// -1 for left and up, 1 for right and down.
    pub const fn sign(self) -> i32 {
        match self {
            Self::Left | Self::Up => -1,
            Self::Right | Self::Down => 1,
        }
    }
}

/// A placed element laid out, with what every diagram type needs to draw it.
pub struct Laid<T: DiagramType> {
    pub el: El<T>,
    /// Where it's drawn: its position, and its size once containers have grown to fit.
    pub rect: Rect,
    pub info: &'static KindInfo<T::Kind>,
    /// Whether it can hold other elements.
    pub is_container: bool,
    /// Whether it's drawn as a filled region with its name as a header.
    pub is_region: bool,
    pub header: f64,
    pub padding: f64,
    pub footer: f64,
    /// Whether its text sits at the top, above elements it holds.
    pub text_at_top: bool,
    pub is_resizable: bool,
    pub child_count: usize,
    pub parent_label: Option<String>,
}

impl<T: DiagramType> Clone for Laid<T> {
    fn clone(&self) -> Self {
        Self {
            el: self.el.clone(),
            rect: self.rect,
            info: self.info,
            is_container: self.is_container,
            is_region: self.is_region,
            header: self.header,
            padding: self.padding,
            footer: self.footer,
            text_at_top: self.text_at_top,
            is_resizable: self.is_resizable,
            child_count: self.child_count,
            parent_label: self.parent_label.clone(),
        }
    }
}

impl<T: DiagramType> std::fmt::Debug for Laid<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Laid")
            .field("el", &self.el)
            .field("rect", &self.rect)
            .finish_non_exhaustive()
    }
}

impl<T: DiagramType> Laid<T> {
    pub fn id(&self) -> &ElementId {
        &self.el.id
    }

    pub fn container_rect(&self) -> ContainerRect {
        ContainerRect {
            rect: self.rect,
            header: self.header,
            padding: self.padding,
            footer: self.footer,
        }
    }
}

/// Text read by screen readers, and shown as status, after a change.
fn list_of(labels: &[String]) -> String {
    match labels.len() {
        0 => String::new(),
        1 => labels[0].clone(),
        2 => format!("{} and {}", labels[0], labels[1]),
        n => format!("{}, and {}", labels[..n - 1].join(", "), labels[n - 1]),
    }
}

/// Such as "CRM", "CRM and Billing", or "CRM, Billing, and Orders".
pub fn list_labels(labels: &[String]) -> String {
    list_of(labels)
}

/// The ids of everything `id` holds, and everything that holds, and so on.
pub fn descendant_ids<K, P>(elements: &[Element<K, P>], id: &ElementId) -> HashSet<ElementId> {
    let mut found: HashSet<ElementId> = HashSet::new();
    let mut parents: Vec<ElementId> = vec![id.clone()];
    while !parents.is_empty() {
        let children: Vec<ElementId> = elements
            .iter()
            .filter(|e| {
                e.parent_id.as_ref().is_some_and(|p| parents.contains(p))
                    && !found.contains(&e.id)
                    && e.id != *id
            })
            .map(|e| e.id.clone())
            .collect();
        found.extend(children.iter().cloned());
        parents = children;
    }
    found
}

/// A diagram type's rules. Every hook has a default; a type overrides what it needs, calling
/// the engine's `base_*` methods for the usual behaviour.
pub trait DiagramType: Sized + 'static {
    const ID: DiagramTypeId;
    type Kind: Copy + Eq + Hash + Debug + Serialize + DeserializeOwned + Send + Sync + 'static;
    type Props: Clone
        + Default
        + PartialEq
        + Debug
        + Serialize
        + DeserializeOwned
        + Send
        + Sync
        + 'static;
    type Arrow: Clone
        + Default
        + PartialEq
        + Debug
        + Serialize
        + DeserializeOwned
        + Send
        + Sync
        + 'static;
    type Colors: ColorScheme;

    /// The fields an arrow can have, in the order a diagram file checks them.
    const ARROW_FIELDS: &'static [Field] = &[];
    /// How long the description of a kind with one can be.
    const DESCRIPTION: DescriptionRule = DescriptionRule {
        max: 0,
        max_lines: None,
    };

    /// The fields an element of `kind` can have beyond the common ones, in the order a diagram
    /// file checks them.
    fn file_fields(_kind: Self::Kind) -> &'static [Field] {
        &[]
    }

    /// An arrow's properties as stored, with each value in its one canonical form, such as
    /// blank text left out.
    fn normalized_arrow(arrow: Self::Arrow) -> Self::Arrow {
        arrow
    }

    /// The elements offered in the palette, in order.
    fn palette() -> &'static [KindInfo<Self::Kind>];

    fn info(kind: Self::Kind) -> &'static KindInfo<Self::Kind> {
        Self::palette()
            .iter()
            .find(|k| k.kind == kind)
            .expect("every kind is in the palette")
    }

    /// What a new diagram starts with.
    fn starting_elements(_eng: &mut Engine<Self>) -> Vec<El<Self>> {
        Vec::new()
    }

    /// The elements arranged by the type's own rules, such as zones filling their network.
    fn arrange(_eng: &Engine<Self>, elements: Vec<El<Self>>) -> Vec<El<Self>> {
        elements
    }

    /// The elements with one resized to `rect`, and anything that follows it.
    fn resized(
        eng: &Engine<Self>,
        elements: Vec<El<Self>>,
        id: &ElementId,
        rect: Rect,
    ) -> Vec<El<Self>> {
        eng.base_resized(elements, id, rect)
    }

    /// The size an element takes when it hasn't been resized.
    fn natural_size(_element: &El<Self>, info: &'static KindInfo<Self::Kind>) -> Size {
        info.size()
    }

    /// Whether an element holds others.
    fn holds_elements(_element: &El<Self>, info: &'static KindInfo<Self::Kind>) -> bool {
        !info.can_contain.is_empty()
    }

    /// The elements added when an element of `kind` is placed.
    fn created(
        eng: &mut Engine<Self>,
        kind: &'static KindInfo<Self::Kind>,
        placement: Placement,
    ) -> Created<Self> {
        eng.base_created(kind, placement)
    }

    /// The size a new element of `kind` takes.
    fn new_size_of(_eng: &Engine<Self>, kind: &'static KindInfo<Self::Kind>) -> Size {
        kind.size()
    }

    /// Where a new element of `kind` would best go when added from the palette.
    fn preferred_position(
        _eng: &Engine<Self>,
        _kind: &'static KindInfo<Self::Kind>,
    ) -> Option<Point> {
        None
    }

    /// Adds an element at a free spot, as a palette click does.
    fn add_at_free_position(eng: &mut Engine<Self>, kind: &'static KindInfo<Self::Kind>) {
        eng.base_add_at_free_position(kind);
    }

    fn resize_limits(eng: &Engine<Self>, element: &Laid<Self>) -> ResizeLimits {
        eng.base_resize_limits(element)
    }

    /// Where a moved element lands under the type's own rules.
    fn constrain_move(
        _eng: &Engine<Self>,
        _element: &Laid<Self>,
        placement: Placement,
    ) -> Placement {
        placement
    }

    /// Moves an element (and anything it holds) and records its container. `with` is the
    /// element with property changes applied, when the properties panel moved it.
    fn commit_move(
        eng: &mut Engine<Self>,
        id: &ElementId,
        placement: Placement,
        with: Option<El<Self>>,
    ) {
        eng.base_commit_move(id, placement, with);
    }

    /// Where an element of `kind` dropped at `position` sits in `parent`.
    fn drop_position(
        eng: &Engine<Self>,
        kind: &'static KindInfo<Self::Kind>,
        position: Point,
        parent: &Laid<Self>,
    ) -> Point {
        eng.base_drop_position(kind, position, parent)
    }

    /// Removes an element.
    fn remove(eng: &mut Engine<Self>, id: &ElementId) {
        eng.base_remove(id);
    }

    /// Handles an arrow key on an element before the usual move. Returns whether it did.
    fn element_key(_eng: &mut Engine<Self>, _id: &ElementId, _dir: Dir, _shift: bool) -> bool {
        false
    }

    /// Such as "Customer Portal, in Sales, column 3, row 5".
    fn describe(eng: &Engine<Self>, element: &El<Self>) -> String {
        eng.base_describe(element)
    }

    /// Such as "Actor (bot)": the element's kind, and anything that sets it apart.
    fn kind_description(_eng: &Engine<Self>, element: &Laid<Self>) -> String {
        element.info.label.to_owned()
    }

    /// What else to read after an element's name, such as its arrows.
    fn extra_details(_eng: &Engine<Self>, _element: &Laid<Self>) -> Vec<String> {
        Vec::new()
    }

    /// Text read after an element's name.
    fn hidden_details(eng: &Engine<Self>, element: &Laid<Self>) -> String {
        eng.base_hidden_details(element)
    }

    /// Whether `source` can draw an arrow to `target`.
    fn can_connect(_eng: &Engine<Self>, source: &Laid<Self>, target: &Laid<Self>) -> bool {
        source.id() != target.id() && source.info.connects_to.contains(&target.el.kind)
    }

    /// A new arrow's properties.
    fn new_arrow(_eng: &Engine<Self>, _source: &ElementId, _target: &ElementId) -> Self::Arrow {
        Self::Arrow::default()
    }

    /// Said when an arrow is added.
    fn arrow_added(eng: &Engine<Self>, connection: &Conn<Self>) -> String {
        format!(
            "{} added.",
            eng.arrow_name(&connection.source_id, &connection.target_id)
        )
    }

    /// Where an arrow was moved to by hand, if its type supports that.
    fn manual_offset(_arrow: &Self::Arrow) -> Option<f64> {
        None
    }

    /// The area an arrow meets on an element: the element, or its figure.
    fn arrow_rect(_eng: &Engine<Self>, element: &Laid<Self>) -> Rect {
        element.rect
    }

    /// Applies an edited element from the properties panel or an agent, with any side effects
    /// the type has, such as a component that becomes a figure letting go of what it held.
    fn apply_element(
        eng: &mut Engine<Self>,
        id: &ElementId,
        with: El<Self>,
        parent_id: Option<ElementId>,
    ) {
        eng.apply_changes(id, with, parent_id);
    }

    /// Checks an edited element, returning the message to show when it isn't valid.
    fn validate_element(element: &El<Self>) -> Result<(), String> {
        validate_label(&element.label)
    }

    /// Checks an edited arrow, returning the message to show when it isn't valid.
    fn validate_arrow(_arrow: &Self::Arrow) -> Result<(), String> {
        Ok(())
    }
}

/// The longest element name.
pub const LABEL_MAX: usize = 100;

/// Checks an element's name.
pub fn validate_label(label: &str) -> Result<(), String> {
    if label.trim().is_empty() {
        Err("Enter a name.".into())
    } else if label.chars().count() > LABEL_MAX {
        Err("Name must be 100 characters or fewer.".into())
    } else {
        Ok(())
    }
}

/// Checks that `text` is at most `max` characters, naming the field in the message.
pub fn validate_length(field: &str, text: &str, max: usize) -> Result<(), String> {
    if text.chars().count() > max {
        Err(format!("{field} must be {max} characters or fewer."))
    } else {
        Ok(())
    }
}

/// A diagram being edited: its elements and arrows, their layout, the selection, and what
/// the last change was.
pub struct Engine<T: DiagramType> {
    elements: Vec<El<T>>,
    connections: Vec<Conn<T>>,
    layout: Vec<Laid<T>>,
    colors: T::Colors,
    /// The selected elements' ids, in the order they were selected.
    pub selection: Vec<ElementId>,
    pub selected_connection: Option<ConnectionId>,
    /// What the last change did, for screen readers and the status line.
    pub announcement: String,
    measure: &'static dyn MeasureText,
    next_id: Box<dyn FnMut() -> String + Send>,
    changes: u64,
}

impl<T: DiagramType> Engine<T> {
    /// An engine for existing content.
    pub fn new(elements: Vec<El<T>>, connections: Vec<Conn<T>>, colors: T::Colors) -> Self {
        let mut engine = Self {
            elements,
            connections,
            layout: Vec::new(),
            colors,
            selection: Vec::new(),
            selected_connection: None,
            announcement: String::new(),
            measure: font_metrics(),
            next_id: Box::new(|| uuid::Uuid::new_v4().to_string()),
            changes: 0,
        };
        engine.relayout();
        engine
    }

    /// An engine for a new diagram, with what the type starts with.
    pub fn new_diagram(colors: T::Colors) -> Self {
        let mut engine = Self::new(Vec::new(), Vec::new(), colors);
        let starting = T::starting_elements(&mut engine);
        engine.elements = starting;
        engine.relayout();
        engine
    }

    /// Uses `next` for new ids, such as a counter in tests.
    pub fn with_ids(mut self, next: impl FnMut() -> String + Send + 'static) -> Self {
        self.next_id = Box::new(next);
        self
    }

    /// Uses `measure` for text widths, such as the estimate in tests.
    pub fn with_measure(mut self, measure: &'static dyn MeasureText) -> Self {
        self.measure = measure;
        self.relayout();
        self
    }

    pub fn new_id(&mut self) -> String {
        (self.next_id)()
    }

    pub fn measure(&self) -> &'static dyn MeasureText {
        self.measure
    }

    pub fn elements(&self) -> &[El<T>] {
        &self.elements
    }

    pub fn connections(&self) -> &[Conn<T>] {
        &self.connections
    }

    /// The elements laid out, in drawing order: containers before what they hold.
    pub fn layout(&self) -> &[Laid<T>] {
        &self.layout
    }

    pub fn colors(&self) -> &T::Colors {
        &self.colors
    }

    /// How many changes have been made, so callers can tell whether an action changed anything.
    pub fn changes(&self) -> u64 {
        self.changes
    }

    /// Replaces the content, such as when undoing or when it changed outside the editor. The
    /// selection keeps only what still exists.
    pub fn replace_content(&mut self, elements: Vec<El<T>>, connections: Vec<Conn<T>>) {
        self.elements = elements;
        self.connections = connections;
        self.relayout();
        let ids: HashSet<&ElementId> = self.elements.iter().map(|e| &e.id).collect();
        self.selection.retain(|id| ids.contains(id));
        if let Some(c) = &self.selected_connection
            && !self.connections.iter().any(|x| &x.id == c)
        {
            self.selected_connection = None;
        }
        self.changes += 1;
    }

    pub fn set_colors(&mut self, colors: T::Colors) {
        self.colors = colors;
        self.relayout();
        self.changes += 1;
    }

    fn relayout(&mut self) {
        let arranged = T::arrange(self, self.elements.clone());
        self.layout = self.lay_out(&arranged);
    }

    /// Stores a new element list, arranged by the type's rules.
    pub fn set_elements(&mut self, elements: Vec<El<T>>) {
        self.elements = T::arrange(self, elements);
        self.relayout();
        self.changes += 1;
    }

    pub fn set_connections(&mut self, connections: Vec<Conn<T>>) {
        self.connections = connections;
        self.changes += 1;
    }

    /// Changes one element.
    pub fn replace(&mut self, id: &ElementId, change: impl FnOnce(El<T>) -> El<T>) {
        let mut change = Some(change);
        let elements = self
            .elements
            .iter()
            .cloned()
            .map(|e| {
                if e.id == *id {
                    (change.take().expect("ids are unique"))(e)
                } else {
                    e
                }
            })
            .collect();
        self.set_elements(elements);
    }

    pub fn element(&self, id: &ElementId) -> Option<&El<T>> {
        self.elements.iter().find(|e| e.id == *id)
    }

    pub fn laid(&self, id: &ElementId) -> Option<&Laid<T>> {
        self.layout.iter().find(|e| e.el.id == *id)
    }

    pub fn connection(&self, id: &ConnectionId) -> Option<&Conn<T>> {
        self.connections.iter().find(|c| c.id == *id)
    }

    pub fn kind_of(&self, element: &El<T>) -> &'static KindInfo<T::Kind> {
        T::info(element.kind)
    }

    pub fn label_of(&self, id: &ElementId) -> Option<&str> {
        self.element(id).map(|e| e.label.as_str())
    }

    // Selection

    /// The selected element's id while just one is selected.
    pub fn selected_id(&self) -> Option<&ElementId> {
        match self.selection.as_slice() {
            [one] => Some(one),
            _ => None,
        }
    }

    pub fn selected(&self) -> Option<&Laid<T>> {
        self.selected_id().and_then(|id| self.laid(id))
    }

    /// Every selected element still on the diagram, in drawing order.
    pub fn selected_elements(&self) -> Vec<&Laid<T>> {
        let ids: HashSet<&ElementId> = self.selection.iter().collect();
        self.layout
            .iter()
            .filter(|e| ids.contains(&e.el.id))
            .collect()
    }

    pub fn is_selected(&self, id: &ElementId) -> bool {
        self.selection.contains(id)
    }

    /// Whether the element is one of two or more selected, which move and are removed together.
    pub fn in_group(&self, id: &ElementId) -> bool {
        self.selection.len() > 1 && self.is_selected(id)
    }

    /// Selects just this element.
    pub fn select(&mut self, id: &ElementId) {
        self.selection = vec![id.clone()];
        self.selected_connection = None;
    }

    /// Adds the element to `base`, or takes it out, keeping the rest selected.
    pub fn toggle_selected(&mut self, id: &ElementId, base: Option<Vec<ElementId>>) {
        let selection = base.unwrap_or_else(|| self.selection.clone());
        let adding = !selection.contains(id);
        let ids: Vec<ElementId> = if adding {
            selection.into_iter().chain([id.clone()]).collect()
        } else {
            selection.into_iter().filter(|x| x != id).collect()
        };
        let count = format!(
            "{} {} selected",
            ids.len(),
            if ids.len() == 1 {
                "element"
            } else {
                "elements"
            }
        );
        self.selection = ids;
        self.selected_connection = None;
        let label = self.label_of(id).unwrap_or_default().to_owned();
        self.announcement = format!(
            "{label} {} the selection, {count}.",
            if adding { "added to" } else { "removed from" }
        );
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.selected_connection = None;
    }

    /// Kinds of element the selected element could sit in, such as domains.
    pub fn parent_kinds(&self) -> Vec<&'static KindInfo<T::Kind>> {
        let Some(kind) = self.selected().map(|s| s.el.kind) else {
            return Vec::new();
        };
        T::palette()
            .iter()
            .filter(|k| k.can_contain.contains(&kind))
            .collect()
    }

    /// Containers the selected element could sit in, naming their kind when it could sit in
    /// more than one, such as "CRM (Application)".
    pub fn parent_options(&self) -> Vec<(ElementId, String)> {
        let Some(selected) = self.selected() else {
            return Vec::new();
        };
        let mixed = self.parent_kinds().len() > 1;
        self.containers_for(selected.el.kind, &self.layout)
            .into_iter()
            .filter(|c| c.el.id != selected.el.id)
            .map(|c| {
                let label = if mixed {
                    format!("{} ({})", c.el.label, c.info.label)
                } else {
                    c.el.label.clone()
                };
                (c.el.id.clone(), label)
            })
            .collect()
    }

    /// Such as "Domain" or "Application or component".
    pub fn parent_field_label(&self) -> Option<String> {
        let labels: Vec<&str> = self.parent_kinds().iter().map(|k| k.label).collect();
        let (first, rest) = labels.split_first()?;
        Some(
            std::iter::once((*first).to_owned())
                .chain(rest.iter().map(|l| l.to_lowercase()))
                .collect::<Vec<_>>()
                .join(" or "),
        )
    }

    /// Applies changes from the properties panel. A new container moves the element to a
    /// free spot in that container, or out onto the canvas.
    pub fn apply_changes(&mut self, id: &ElementId, with: El<T>, parent_id: Option<ElementId>) {
        let Some(selected) = self.laid(id).cloned() else {
            return;
        };
        if parent_id == selected.el.parent_id {
            self.replace(id, |_| with);
            return;
        }
        let others: Vec<&Laid<T>> = self.layout.iter().filter(|e| e.el.id != *id).collect();
        let parent = parent_id
            .as_ref()
            .and_then(|p| others.iter().find(|e| e.el.id == *p));
        let position = match parent {
            Some(parent) => find_free_position(
                &others
                    .iter()
                    .filter(|e| e.el.parent_id.as_ref() == Some(&parent.el.id))
                    .map(|e| e.rect)
                    .collect::<Vec<_>>(),
                selected.rect.size(),
                child_placement_area(&parent.container_rect()),
            ),
            None => find_free_position(
                &others.iter().map(|e| e.rect).collect::<Vec<_>>(),
                selected.rect.size(),
                CANVAS_AREA,
            ),
        };
        T::commit_move(
            self,
            id,
            Placement {
                position,
                parent_id,
            },
            Some(with),
        );
    }

    // Adding

    /// Adds an element of `kind` at a free spot, as a palette click does.
    pub fn add_at_free_position(&mut self, kind: T::Kind) {
        T::add_at_free_position(self, T::info(kind));
    }

    pub fn base_add_at_free_position(&mut self, kind: &'static KindInfo<T::Kind>) {
        let size = T::new_size_of(self, kind);
        let container = self
            .containers_for(kind.kind, &self.layout)
            .into_iter()
            .find(|c| Some(&c.el.id) == self.selected_id())
            .cloned();
        if let Some(container) = container {
            let siblings: Vec<Rect> = self
                .layout
                .iter()
                .filter(|e| e.el.parent_id.as_ref() == Some(&container.el.id))
                .map(|e| e.rect)
                .collect();
            let position = find_free_position(
                &siblings,
                size,
                child_placement_area(&container.container_rect()),
            );
            self.add(
                kind,
                Placement {
                    position,
                    parent_id: Some(container.el.id.clone()),
                },
            );
            return;
        }
        let preferred = T::preferred_position(self, kind);
        let free = preferred.is_some_and(|p| {
            let spot = Rect::from_parts(p, size);
            CANVAS_AREA.contains_rect(&spot)
                && !self
                    .layout
                    .iter()
                    .any(|e| e.el.parent_id.is_none() && e.rect.overlaps(&spot))
        });
        let position = match preferred {
            Some(p) if free => p,
            _ => find_free_position(
                &self.layout.iter().map(|e| e.rect).collect::<Vec<_>>(),
                size,
                CANVAS_AREA,
            ),
        };
        self.add(
            kind,
            Placement {
                position,
                parent_id: None,
            },
        );
    }

    /// Adds a kind dropped from the palette, centred on the drop point.
    pub fn palette_drop(&mut self, kind: T::Kind, point: Point) {
        let kind = T::info(kind);
        let size = T::new_size_of(self, kind);
        let desired = Point::new(point.x - size.width / 2.0, point.y - size.height / 2.0);
        let placement = self.drop_placement(kind, desired);
        self.add(kind, placement);
    }

    /// Adds an element of `kind` at `placement`, with anything added with it, and selects it.
    pub fn add(&mut self, kind: &'static KindInfo<T::Kind>, placement: Placement) {
        let Created { added, existing } = T::created(self, kind, placement);
        let existing = existing.unwrap_or_else(|| self.elements.clone());
        let Some(first) = added.first().cloned() else {
            return;
        };
        let with: Vec<El<T>> = added[1..].to_vec();
        self.set_elements(existing.into_iter().chain(added).collect());
        self.select(&first.id);
        let holding = if with.is_empty() {
            String::new()
        } else {
            let kind_label = self.kind_of(&with[0]).label.to_lowercase();
            format!(
                ", with {kind_label}{} {}",
                if with.len() == 1 { "" } else { "s" },
                with.iter()
                    .map(|c| c.label.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let current = self.element(&first.id).cloned().unwrap_or(first);
        self.announcement = format!("{}{holding} added.", T::describe(self, &current));
    }

    /// The usual new element: the kind's label, snapped onto the canvas.
    pub fn base_created(
        &mut self,
        kind: &'static KindInfo<T::Kind>,
        placement: Placement,
    ) -> Created<T> {
        let position = snap_to_canvas(placement.position, kind.size());
        let element = Element {
            id: ElementId(self.new_id()),
            kind: kind.kind,
            label: kind.label.to_owned(),
            description: None,
            parent_id: placement.parent_id,
            props: T::Props::default(),
            width: None,
            height: None,
            x: position.x,
            y: position.y,
        };
        Created {
            added: vec![element],
            existing: None,
        }
    }

    // Moving

    /// Where a moved element lands; anything it holds moves with it. An element sits in a
    /// container while it's fully inside the container's content area.
    pub fn move_placement(&self, element: &Laid<T>, desired: Point) -> Placement {
        let position = snap_to_canvas(desired, element.rect.size());
        let rect = Rect::from_parts(position, element.rect.size());
        let inside = descendant_ids(&self.elements, element.id());
        let parent = self
            .containers_for(element.el.kind, &self.layout)
            .into_iter()
            .filter(|c| c.el.id != *element.id() && !inside.contains(&c.el.id))
            .rev()
            .find(|c| content_area(&c.container_rect()).contains_rect(&rect));
        T::constrain_move(
            self,
            element,
            Placement {
                position,
                parent_id: parent.map(|p| p.el.id.clone()),
            },
        )
    }

    /// The selected elements that move when the group moves: those not inside another selected
    /// element, which carries them along with it.
    pub fn group_roots(&self) -> Vec<&Laid<T>> {
        let ids: HashSet<&ElementId> = self.selection.iter().collect();
        let by_id: HashMap<&ElementId, &El<T>> = self.elements.iter().map(|e| (&e.id, e)).collect();
        let inside_selected = |element: &El<T>| {
            let mut seen = HashSet::new();
            let mut current = element.parent_id.as_ref();
            while let Some(id) = current {
                if seen.contains(id) {
                    break;
                }
                if ids.contains(id) {
                    return true;
                }
                seen.insert(id);
                current = by_id.get(id).and_then(|e| e.parent_id.as_ref());
            }
            false
        };
        self.selected_elements()
            .into_iter()
            .filter(|e| !inside_selected(&e.el))
            .collect()
    }

    /// Where each selected element lands when the group moves by `delta`: snapped to the grid,
    /// and all kept on the canvas.
    pub fn group_placements(&self, delta: Point) -> Vec<(ElementId, Placement)> {
        let roots = self.group_roots();
        if roots.is_empty() {
            return Vec::new();
        }
        let left = roots.iter().map(|e| e.rect.x).fold(f64::INFINITY, f64::min);
        let top = roots.iter().map(|e| e.rect.y).fold(f64::INFINITY, f64::min);
        let right = roots
            .iter()
            .map(|e| e.rect.right())
            .fold(f64::NEG_INFINITY, f64::max);
        let bottom = roots
            .iter()
            .map(|e| e.rect.bottom())
            .fold(f64::NEG_INFINITY, f64::max);
        let dx = snap(delta.x).max(-left).min(CANVAS_WIDTH - right);
        let dy = snap(delta.y).max(-top).min(CANVAS_HEIGHT - bottom);
        let moving: HashSet<ElementId> = roots
            .iter()
            .flat_map(|root| {
                std::iter::once(root.id().clone()).chain(descendant_ids(&self.elements, root.id()))
            })
            .collect();
        roots
            .iter()
            .map(|root| {
                let rect = Rect::new(
                    root.rect.x + dx,
                    root.rect.y + dy,
                    root.rect.width,
                    root.rect.height,
                );
                let parent = self
                    .containers_for(root.el.kind, &self.layout)
                    .into_iter()
                    .filter(|c| !moving.contains(&c.el.id))
                    .rev()
                    .find(|c| content_area(&c.container_rect()).contains_rect(&rect));
                let placement = Placement {
                    position: rect.position(),
                    parent_id: parent.map(|p| p.el.id.clone()),
                };
                (root.id().clone(), T::constrain_move(self, root, placement))
            })
            .collect()
    }

    /// Moves the selected elements together (and anything they hold).
    pub fn commit_group_move(&mut self, placements: &[(ElementId, Placement)]) {
        let moved = placements.iter().any(|(id, p)| {
            self.element(id).is_some_and(|stored| {
                stored.x != p.position.x
                    || stored.y != p.position.y
                    || stored.parent_id != p.parent_id
            })
        });
        if !moved {
            return;
        }
        let elements = self.apply_placements(self.elements.clone(), placements);
        self.set_elements(elements);
        let ids: HashSet<&ElementId> = placements.iter().map(|(id, _)| id).collect();
        let labels: Vec<String> = self
            .elements
            .iter()
            .filter(|e| ids.contains(&e.id))
            .map(|e| e.label.clone())
            .collect();
        self.announcement = format!("{} moved.", list_of(&labels));
    }

    /// Where a dropped element lands: inside a container when its centre falls on one.
    pub fn drop_placement(&self, kind: &'static KindInfo<T::Kind>, desired: Point) -> Placement {
        let size = T::new_size_of(self, kind);
        let position = snap_to_canvas(desired, size);
        let center = Rect::from_parts(position, size).center();
        let parent = self
            .containers_for(kind.kind, &self.layout)
            .into_iter()
            .rev()
            .find(|c| c.rect.contains_point(center));
        match parent {
            Some(parent) => Placement {
                position: T::drop_position(self, kind, position, parent),
                parent_id: Some(parent.el.id.clone()),
            },
            None => Placement {
                position,
                parent_id: None,
            },
        }
    }

    pub fn base_drop_position(
        &self,
        kind: &'static KindInfo<T::Kind>,
        position: Point,
        parent: &Laid<T>,
    ) -> Point {
        let size = T::new_size_of(self, kind);
        snap_to_canvas(
            clamp_into(position, size, content_area(&parent.container_rect())),
            size,
        )
    }

    /// Containers in `layout` that can hold elements of `kind`. Later ones draw on top, so
    /// callers search from the end.
    pub fn containers_for<'a>(&self, kind: T::Kind, layout: &'a [Laid<T>]) -> Vec<&'a Laid<T>> {
        layout
            .iter()
            .filter(|e| e.is_container && e.info.can_contain.contains(&kind))
            .collect()
    }

    /// Moves an element (and anything it holds) to `placement`.
    pub fn commit_move(&mut self, id: &ElementId, placement: Placement) {
        T::commit_move(self, id, placement, None);
    }

    pub fn base_commit_move(&mut self, id: &ElementId, placement: Placement, with: Option<El<T>>) {
        let Some(stored) = self.element(id).cloned() else {
            return;
        };
        let unchanged = stored.x == placement.position.x
            && stored.y == placement.position.y
            && stored.parent_id == placement.parent_id;
        if unchanged && with.is_none() {
            return;
        }
        let dx = placement.position.x - stored.x;
        let dy = placement.position.y - stored.y;
        let mut moved = with.unwrap_or_else(|| stored.clone());
        moved.x = placement.position.x;
        moved.y = placement.position.y;
        moved.parent_id = placement.parent_id;
        let inside = descendant_ids(&self.elements, id);
        let elements = self
            .elements
            .iter()
            .map(|e| {
                if e.id == *id {
                    moved.clone()
                } else if inside.contains(&e.id) {
                    let mut e = e.clone();
                    e.x += dx;
                    e.y += dy;
                    e
                } else {
                    e.clone()
                }
            })
            .collect();
        self.set_elements(elements);
        if !unchanged {
            self.announcement = format!("{}.", T::describe(self, &moved));
        }
    }

    /// Moves elements to their placements, each with anything it holds.
    pub fn apply_placements(
        &self,
        elements: Vec<El<T>>,
        placements: &[(ElementId, Placement)],
    ) -> Vec<El<T>> {
        let mut shifts: HashMap<ElementId, Point> = HashMap::new();
        for (id, placement) in placements {
            if let Some(e) = elements.iter().find(|e| e.id == *id) {
                let shift = Point::new(placement.position.x - e.x, placement.position.y - e.y);
                for child in descendant_ids(&elements, id) {
                    shifts.insert(child, shift);
                }
            }
        }
        elements
            .into_iter()
            .map(|mut e| {
                if let Some((_, p)) = placements.iter().find(|(id, _)| *id == e.id) {
                    e.x = p.position.x;
                    e.y = p.position.y;
                    e.parent_id.clone_from(&p.parent_id);
                } else if let Some(shift) = shifts.get(&e.id) {
                    e.x += shift.x;
                    e.y += shift.y;
                }
                e
            })
            .collect()
    }

    /// What's drawn while elements are being dragged to `placements`.
    pub fn preview_placements(&self, placements: &[(ElementId, Placement)]) -> Vec<Laid<T>> {
        self.lay_out(&self.apply_placements(self.elements.clone(), placements))
    }

    /// What's drawn while an element is being resized to `rect`.
    pub fn preview_resize(&self, id: &ElementId, rect: Rect) -> Vec<Laid<T>> {
        let resized = T::resized(self, self.elements.clone(), id, rect);
        self.lay_out(&T::arrange(self, resized))
    }

    // Keyboard

    /// An arrow key pressed on an element: the type's own rule, or a move by a grid step.
    pub fn arrow_key(&mut self, id: &ElementId, dir: Dir, shift: bool) {
        if T::element_key(self, id, dir, shift) {
            return;
        }
        self.base_arrow_key(id, dir, shift);
    }

    pub fn base_arrow_key(&mut self, id: &ElementId, dir: Dir, shift: bool) {
        let offset = dir.offset(shift);
        if self.in_group(id) {
            let placements = self.group_placements(offset);
            self.commit_group_move(&placements);
            return;
        }
        let Some(element) = self.laid(id).cloned() else {
            return;
        };
        let desired = Point::new(element.rect.x + offset.x, element.rect.y + offset.y);
        let placement = self.move_placement(&element, desired);
        self.commit_move(id, placement);
    }

    /// Delete or Backspace: removes the selected elements, or else the selected arrow.
    pub fn delete_selection(&mut self) -> bool {
        if !self.selected_elements().is_empty() {
            self.remove_selected();
            true
        } else if let Some(id) = self.selected_connection.clone() {
            self.remove_connection(&id);
            true
        } else {
            false
        }
    }

    // Resizing

    /// Where a resize handle sits, centred on its corner, or on the middle of its end for an
    /// element resized only across. Returns the handle's centre.
    pub fn handle_position(element: &Laid<T>, corner: Corner) -> Point {
        let r = element.rect;
        let y = if element.info.resizable == Resizable::Width {
            r.y + r.height / 2.0
        } else if corner == Corner::TopLeft {
            r.y
        } else {
            r.bottom()
        };
        let x = if corner == Corner::TopLeft {
            r.x
        } else {
            r.right()
        };
        Point::new(x, y)
    }

    /// Such as "Resize Sales from the top-left corner", or "from the left end" across only.
    pub fn handle_label(element: &Laid<T>, corner: Corner) -> String {
        let where_ = if element.info.resizable == Resizable::Width {
            if corner == Corner::TopLeft {
                "left end"
            } else {
                "right end"
            }
        } else if corner == Corner::TopLeft {
            "top-left corner"
        } else {
            "bottom-right corner"
        };
        format!("Resize {} from the {where_}", element.el.label)
    }

    pub fn base_resize_limits(&self, element: &Laid<T>) -> ResizeLimits {
        ResizeLimits {
            min_size: if element.is_region {
                MIN_CONTAINER_SIZE
            } else {
                MIN_ELEMENT_SIZE
            },
            children: if element.is_container {
                self.layout
                    .iter()
                    .filter(|c| c.el.parent_id.as_ref() == Some(element.id()))
                    .map(|c| c.rect)
                    .collect()
            } else {
                Vec::new()
            },
        }
    }

    /// Where an element goes when one of its corners is moved by `delta`; the opposite corner
    /// stays put.
    pub fn resize_bounds(&self, element: &Laid<T>, corner: Corner, delta: Point) -> Rect {
        if element.info.resizable == Resizable::Width {
            let rect = self.resize_bounds_for(element, corner, Point::new(delta.x, 0.0));
            return Rect {
                y: element.rect.y,
                height: element.rect.height,
                ..rect
            };
        }
        self.resize_bounds_for(element, corner, delta)
    }

    fn resize_bounds_for(&self, element: &Laid<T>, corner: Corner, delta: Point) -> Rect {
        let ResizeLimits { min_size, children } = T::resize_limits(self, element);
        let r = element.rect;
        if corner == Corner::BottomRight {
            let desired = Size::new(r.width + delta.x, r.height + delta.y);
            let size = resize_element(&element.container_rect(), desired, min_size, &children);
            return Rect::from_parts(r.position(), size);
        }
        let parent = element
            .el
            .parent_id
            .as_ref()
            .and_then(|p| self.laid(p))
            .map(|p| content_area(&p.container_rect()));
        resize_from_top_left(
            &element.container_rect(),
            Point::new(r.x + delta.x, r.y + delta.y),
            min_size,
            &children,
            parent.unwrap_or(CANVAS),
        )
    }

    /// An arrow key on a resize handle: moves that corner by a grid step.
    pub fn resize_key(&mut self, id: &ElementId, corner: Corner, dir: Dir, shift: bool) {
        let Some(element) = self.laid(id).cloned() else {
            return;
        };
        let rect = self.resize_bounds(&element, corner, dir.offset(shift));
        self.commit_resize(id, rect);
    }

    pub fn commit_resize(&mut self, id: &ElementId, rect: Rect) {
        let Some(stored) = self.laid(id).cloned() else {
            return;
        };
        if stored.rect == rect {
            return;
        }
        let resized = T::resized(self, self.elements.clone(), id, rect);
        self.set_elements(resized);
        self.announcement = format!(
            "{} resized to {} by {} grid squares.",
            stored.el.label,
            fmt_num(rect.width / GRID),
            fmt_num(rect.height / GRID)
        );
    }

    pub fn base_resized(&self, elements: Vec<El<T>>, id: &ElementId, rect: Rect) -> Vec<El<T>> {
        elements
            .into_iter()
            .map(|mut e| {
                if e.id == *id {
                    e.x = rect.x;
                    e.y = rect.y;
                    e.width = Some(rect.width);
                    e.height = Some(rect.height);
                }
                e
            })
            .collect()
    }

    // Removing

    /// Removes every selected element.
    pub fn remove_selected(&mut self) {
        let selected: Vec<(ElementId, String)> = self
            .selected_elements()
            .iter()
            .map(|e| (e.id().clone(), e.el.label.clone()))
            .collect();
        if let [(id, _)] = selected.as_slice() {
            T::remove(self, id);
            return;
        }
        for (id, _) in &selected {
            if self.element(id).is_some() {
                T::remove(self, id);
            }
        }
        let labels: Vec<String> = selected.into_iter().map(|(_, l)| l).collect();
        self.announcement = format!("{} removed.", list_of(&labels));
    }

    /// Removes an element and its arrows. A removed container's children stay on the diagram,
    /// outside it.
    pub fn base_remove(&mut self, id: &ElementId) {
        let Some(element) = self.element(id).cloned() else {
            return;
        };
        let elements = self
            .elements
            .iter()
            .filter(|e| e.id != *id)
            .cloned()
            .map(|e| {
                if e.parent_id.as_ref() == Some(id) {
                    e.with_parent(None)
                } else {
                    e
                }
            })
            .collect();
        self.set_elements(elements);
        self.remove_connections_of(id);
        self.clear_selection();
        self.announcement = format!("{} removed.", element.label);
    }

    /// Removes the arrows to and from an element.
    pub fn remove_connections_of(&mut self, id: &ElementId) {
        let before = self.connections.len();
        let remaining: Vec<Conn<T>> = self
            .connections
            .iter()
            .filter(|c| c.source_id != *id && c.target_id != *id)
            .cloned()
            .collect();
        if remaining.len() != before {
            self.set_connections(remaining);
        }
    }

    // Arrows

    pub fn can_connect_from(element: &Laid<T>) -> bool {
        !element.info.connects_to.is_empty()
    }

    pub fn can_connect(&self, source: &ElementId, target: &ElementId) -> bool {
        match (self.laid(source), self.laid(target)) {
            (Some(s), Some(t)) => T::can_connect(self, s, t),
            _ => false,
        }
    }

    /// Adds an arrow, unless it's to the same element or one like it is already there.
    pub fn add_connection(
        &mut self,
        source: &ElementId,
        target: &ElementId,
    ) -> Option<ConnectionId> {
        let duplicate = self
            .connections
            .iter()
            .any(|c| c.source_id == *source && c.target_id == *target);
        if duplicate
            || source == target
            || self.element(source).is_none()
            || self.element(target).is_none()
        {
            return None;
        }
        let props = T::new_arrow(self, source, target);
        let connection = Connection {
            id: ConnectionId(self.new_id()),
            source_id: source.clone(),
            target_id: target.clone(),
            props,
        };
        let id = connection.id.clone();
        self.announcement = T::arrow_added(self, &connection);
        let mut connections = self.connections.clone();
        connections.push(connection);
        self.set_connections(connections);
        Some(id)
    }

    pub fn remove_connection(&mut self, id: &ConnectionId) {
        let Some(connection) = self.connection(id).cloned() else {
            return;
        };
        let remaining = self
            .connections
            .iter()
            .filter(|c| c.id != *id)
            .cloned()
            .collect();
        self.set_connections(remaining);
        if self.selected_connection.as_ref() == Some(id) {
            self.selected_connection = None;
        }
        self.announcement = format!(
            "{} removed.",
            self.arrow_name(&connection.source_id, &connection.target_id)
        );
    }

    /// Changes an arrow's properties.
    pub fn set_connection_props(&mut self, id: &ConnectionId, props: T::Arrow) {
        let props = T::normalized_arrow(props);
        if self.connection(id).is_none_or(|c| c.props == props) {
            return;
        }
        let connections = self
            .connections
            .iter()
            .cloned()
            .map(|mut c| {
                if c.id == *id {
                    c.props = props.clone();
                }
                c
            })
            .collect();
        self.set_connections(connections);
    }

    pub fn select_connection(&mut self, id: &ConnectionId) {
        self.selection.clear();
        self.selected_connection = Some(id.clone());
    }

    pub fn arrow_name(&self, source: &ElementId, target: &ElementId) -> String {
        format!(
            "Arrow from {} to {}",
            self.label_of(source).unwrap_or("?"),
            self.label_of(target).unwrap_or("?")
        )
    }

    /// Elements the selected element doesn't point to yet and could, sorted by label.
    pub fn arrow_target_options(&self) -> Vec<(ElementId, String)> {
        let Some(selected) = self.selected() else {
            return Vec::new();
        };
        if !Self::can_connect_from(selected) {
            return Vec::new();
        }
        let existing: HashSet<&ElementId> = self
            .connections
            .iter()
            .filter(|c| c.source_id == *selected.id())
            .map(|c| &c.target_id)
            .collect();
        let mixed = selected.info.connects_to.len() > 1;
        let mut options: Vec<(ElementId, String)> = self
            .layout
            .iter()
            .filter(|e| T::can_connect(self, selected, e) && !existing.contains(e.id()))
            .map(|e| {
                let label = if mixed {
                    format!("{} ({})", e.el.label, e.info.label)
                } else {
                    e.el.label.clone()
                };
                (e.id().clone(), label)
            })
            .collect();
        options.sort_by_key(|o| o.1.to_lowercase());
        options
    }

    /// The topmost element at `point` that `source` can point to.
    pub fn connect_target_at(&self, source: &ElementId, point: Point) -> Option<ElementId> {
        let source = self.laid(source)?;
        self.layout
            .iter()
            .rev()
            .find(|e| T::can_connect(self, source, e) && e.rect.contains_point(point))
            .map(|e| e.id().clone())
    }

    /// Each selected element's arrows: id, whether it leaves the element, and the other end.
    pub fn arrows_of(&self, id: &ElementId) -> Vec<(&Conn<T>, bool, String)> {
        self.connections
            .iter()
            .filter(|c| c.source_id == *id || c.target_id == *id)
            .map(|c| {
                let outgoing = c.source_id == *id;
                let other = self
                    .label_of(if outgoing { &c.target_id } else { &c.source_id })
                    .unwrap_or_default()
                    .to_owned();
                (c, outgoing, other)
            })
            .collect()
    }

    // Describing

    pub fn position_text(p: Point) -> String {
        format!(
            "column {}, row {}",
            fmt_num(p.x / GRID + 1.0),
            fmt_num(p.y / GRID + 1.0)
        )
    }

    pub fn base_describe(&self, element: &El<T>) -> String {
        let parent = element.parent_id.as_ref().and_then(|p| self.element(p));
        let mut parts = vec![element.label.clone()];
        if let Some(parent) = parent {
            parts.push(format!("in {}", parent.label));
        }
        parts.push(Self::position_text(Point::new(element.x, element.y)));
        parts.join(", ")
    }

    pub fn base_hidden_details(&self, element: &Laid<T>) -> String {
        let contents = format!(
            "contains {} {}",
            element.child_count,
            if element.child_count == 1 {
                "element"
            } else {
                "elements"
            }
        );
        let mut parts = vec![T::kind_description(self, element)];
        if let Some(parent) = &element.parent_label {
            parts.push(format!("in {parent}"));
        }
        if element.is_region || element.child_count > 0 {
            parts.push(contents);
        }
        parts.extend(T::extra_details(self, element));
        parts.push(Self::position_text(element.rect.position()));
        format!(", {}", parts.join(", "))
    }

    /// The element's full accessible description: its name, then its details.
    pub fn accessible_label(&self, element: &Laid<T>) -> String {
        format!("{}{}", element.el.label, T::hidden_details(self, element))
    }

    // Layout

    /// Adds each element's size and kind, grows containers around what they hold, and orders
    /// the elements so each container comes right before what it holds.
    pub fn lay_out(&self, elements: &[El<T>]) -> Vec<Laid<T>> {
        struct Sized<'a, K: 'static, P> {
            el: &'a Element<K, P>,
            info: &'static KindInfo<K>,
            rect: Rect,
            is_container: bool,
            is_region: bool,
            header: f64,
            padding: f64,
            footer: f64,
        }
        let sized: Vec<Sized<'_, T::Kind, T::Props>> = elements
            .iter()
            .map(|el| {
                let info = T::info(el.kind);
                let size = T::natural_size(el, info);
                let is_container = T::holds_elements(el, info);
                Sized {
                    el,
                    info,
                    rect: Rect::new(
                        el.x,
                        el.y,
                        el.width.unwrap_or(size.width),
                        el.height.unwrap_or(size.height),
                    ),
                    is_container,
                    is_region: is_container && info.container_look == ContainerLook::Region,
                    header: info.header(),
                    padding: info.padding(),
                    footer: info.footer(),
                }
            })
            .collect();
        let index: HashMap<&ElementId, usize> = sized
            .iter()
            .enumerate()
            .map(|(i, s)| (&s.el.id, i))
            .collect();
        // Only containers hold elements: anything else's children are drawn on their own.
        let holder_of = |i: usize| -> Option<usize> {
            let parent = sized[i].el.parent_id.as_ref()?;
            let p = *index.get(parent)?;
            sized[p].is_container.then_some(p)
        };
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); sized.len()];
        for i in 0..sized.len() {
            if let Some(p) = holder_of(i) {
                children[p].push(i);
            }
        }

        fn fit<K, P>(
            i: usize,
            seen: &mut Vec<usize>,
            sized: &[Sized<'_, K, P>],
            children: &[Vec<usize>],
            fitted: &mut HashMap<usize, Size>,
        ) -> Size {
            if let Some(size) = fitted.get(&i) {
                return *size;
            }
            seen.push(i);
            let kids: Vec<Rect> = children[i]
                .iter()
                .filter(|c| !seen.contains(c))
                .copied()
                .collect::<Vec<_>>()
                .into_iter()
                .map(|c| {
                    Rect::from_parts(
                        sized[c].rect.position(),
                        fit(c, seen, sized, children, fitted),
                    )
                })
                .collect();
            seen.pop();
            let s = &sized[i];
            let size = fit_container(
                &ContainerRect {
                    rect: s.rect,
                    header: s.header,
                    padding: s.padding,
                    footer: s.footer,
                },
                &kids,
            );
            fitted.insert(i, size);
            size
        }

        let mut fitted: HashMap<usize, Size> = HashMap::new();
        let mut laid: Vec<Option<Laid<T>>> = (0..sized.len())
            .map(|i| {
                let size = fit(i, &mut Vec::new(), &sized, &children, &mut fitted);
                let s = &sized[i];
                let child_count = children[i].len();
                Some(Laid {
                    el: s.el.clone(),
                    rect: Rect::from_parts(s.rect.position(), size),
                    info: s.info,
                    is_container: s.is_container,
                    is_region: s.is_region,
                    header: s.header,
                    padding: s.padding,
                    footer: s.footer,
                    text_at_top: s.is_container && !s.is_region && child_count > 0,
                    is_resizable: s.is_container || s.info.resizable != Resizable::No,
                    child_count,
                    parent_label: s
                        .el
                        .parent_id
                        .as_ref()
                        .and_then(|p| index.get(p))
                        .map(|&p| sized[p].el.label.clone()),
                })
            })
            .collect();

        fn ordered<T: DiagramType>(
            i: usize,
            seen: &mut Vec<usize>,
            children: &[Vec<usize>],
            laid: &mut Vec<Option<Laid<T>>>,
            out: &mut Vec<Laid<T>>,
        ) {
            if let Some(item) = laid[i].take() {
                out.push(item);
            }
            seen.push(i);
            for &c in &children[i] {
                if !seen.contains(&c) {
                    ordered(c, seen, children, laid, out);
                }
            }
            seen.pop();
        }

        let mut roots: Vec<usize> = (0..sized.len())
            .filter(|&i| holder_of(i).is_none())
            .collect();
        roots.sort_by_key(|&i| sized[i].info.layer);
        let mut out = Vec::with_capacity(sized.len());
        for root in roots {
            ordered(root, &mut Vec::new(), &children, &mut laid, &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NoColors, NoProps};
    use crate::text::EstimateText;
    use serde::Deserialize;

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

    #[test]
    fn palette_clicks_find_free_spots() {
        let mut e = engine();
        e.add_at_free_position(Kind::Item);
        e.add_at_free_position(Kind::Item);
        let rects: Vec<Rect> = e.layout().iter().map(|l| l.rect).collect();
        assert_eq!(rects[0], Rect::new(20.0, 20.0, 160.0, 60.0));
        assert_eq!(rects[1], Rect::new(200.0, 20.0, 160.0, 60.0));
        assert_eq!(e.selection, vec![id("id2")]);
        assert_eq!(e.announcement, "Item, column 11, row 2 added.");
    }

    #[test]
    fn dropping_on_a_container_puts_it_inside_and_grows_it() {
        let mut e = engine();
        e.palette_drop(Kind::Group, Point::new(240.0, 140.0));
        let group = e.layout()[0].clone();
        assert_eq!(group.rect, Rect::new(40.0, 40.0, 400.0, 200.0));
        e.palette_drop(Kind::Item, Point::new(400.0, 200.0));
        let item = e.laid(&id("id2")).unwrap().clone();
        assert_eq!(item.el.parent_id, Some(id("id1")));
        // Kept inside the content area.
        assert_eq!(item.rect.position(), Point::new(280.0, 180.0));
        // Containers come first, then what they hold.
        assert_eq!(e.layout()[0].id(), &id("id1"));
        assert_eq!(e.laid(&id("id1")).unwrap().child_count, 1);
    }

    #[test]
    fn moving_a_container_moves_its_children() {
        let mut e = engine();
        e.palette_drop(Kind::Group, Point::new(240.0, 140.0));
        e.palette_drop(Kind::Item, Point::new(200.0, 140.0));
        e.select(&id("id1"));
        e.arrow_key(&id("id1"), Dir::Right, true);
        assert_eq!(e.element(&id("id1")).unwrap().x, 140.0);
        assert_eq!(e.element(&id("id2")).unwrap().x, 220.0);
    }

    #[test]
    fn stepping_out_of_a_container_takes_it_out() {
        let mut e = engine();
        e.palette_drop(Kind::Group, Point::new(240.0, 140.0));
        e.palette_drop(Kind::Item, Point::new(200.0, 140.0));
        // Move the item up until it leaves the content area.
        for _ in 0..3 {
            e.arrow_key(&id("id2"), Dir::Up, false);
        }
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, None);
    }

    #[test]
    fn removing_a_container_keeps_its_children() {
        let mut e = engine();
        e.palette_drop(Kind::Group, Point::new(240.0, 140.0));
        e.palette_drop(Kind::Item, Point::new(200.0, 140.0));
        e.select(&id("id1"));
        e.delete_selection();
        assert_eq!(e.elements().len(), 1);
        assert_eq!(e.elements()[0].parent_id, None);
        assert_eq!(e.announcement, "Group removed.");
    }

    #[test]
    fn group_moves_keep_relative_places() {
        let mut e = engine();
        e.add_at_free_position(Kind::Item);
        e.add_at_free_position(Kind::Item);
        e.selection = vec![id("id1"), id("id2")];
        e.arrow_key(&id("id1"), Dir::Down, false);
        assert_eq!(e.element(&id("id1")).unwrap().y, 40.0);
        assert_eq!(e.element(&id("id2")).unwrap().y, 40.0);
        assert_eq!(e.announcement, "Item and Item moved.");
        // Kept on the canvas.
        e.arrow_key(&id("id1"), Dir::Left, true);
        assert_eq!(e.element(&id("id1")).unwrap().x, 0.0);
    }

    #[test]
    fn arrows_are_added_once_and_removed_with_elements() {
        let mut e = engine();
        e.add_at_free_position(Kind::Item);
        e.add_at_free_position(Kind::Item);
        assert!(e.add_connection(&id("id1"), &id("id2")).is_some());
        assert!(e.add_connection(&id("id1"), &id("id2")).is_none());
        assert!(e.add_connection(&id("id1"), &id("id1")).is_none());
        assert_eq!(e.announcement, "Arrow from Item to Item added.");
        e.select(&id("id1"));
        e.delete_selection();
        assert!(e.connections().is_empty());
    }

    #[test]
    fn resizing_snaps_and_stays_around_children() {
        let mut e = engine();
        e.palette_drop(Kind::Group, Point::new(240.0, 140.0));
        e.palette_drop(Kind::Item, Point::new(200.0, 140.0));
        let group = e.laid(&id("id1")).unwrap().clone();
        let rect = e.resize_bounds(&group, Corner::BottomRight, Point::new(-1000.0, -1000.0));
        // Around the item at (120,120) 160×60, plus padding.
        assert_eq!(rect, Rect::new(40.0, 40.0, 260.0, 160.0));
        e.commit_resize(&id("id1"), rect);
        assert_eq!(e.element(&id("id1")).unwrap().width, Some(260.0));
        assert_eq!(e.announcement, "Group resized to 13 by 8 grid squares.");
    }

    #[test]
    fn toggling_selection_announces() {
        let mut e = engine();
        e.add_at_free_position(Kind::Item);
        e.add_at_free_position(Kind::Item);
        e.toggle_selected(&id("id1"), None);
        assert_eq!(e.selection.len(), 2);
        assert_eq!(
            e.announcement,
            "Item added to the selection, 2 elements selected."
        );
    }

    #[test]
    fn hidden_details_read_kind_container_and_position() {
        let mut e = engine();
        e.palette_drop(Kind::Group, Point::new(240.0, 140.0));
        let group = e.laid(&id("id1")).unwrap().clone();
        assert_eq!(
            e.accessible_label(&group),
            "Group, Group, contains 0 elements, column 3, row 3"
        );
    }

    #[test]
    fn descendants_include_grandchildren() {
        let mk = |i: &str, p: Option<&str>| Element::<Kind, NoProps> {
            id: id(i),
            kind: Kind::Item,
            label: i.into(),
            description: None,
            parent_id: p.map(id),
            props: NoProps {},
            width: None,
            height: None,
            x: 0.0,
            y: 0.0,
        };
        let els = vec![
            mk("a", None),
            mk("b", Some("a")),
            mk("c", Some("b")),
            mk("d", None),
        ];
        let found = descendant_ids(&els, &id("a"));
        assert_eq!(found, [id("b"), id("c")].into_iter().collect());
    }
}
