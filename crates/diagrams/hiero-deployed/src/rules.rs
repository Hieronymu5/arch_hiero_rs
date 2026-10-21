//! The deployed component diagram's rules: layers stacked from top to bottom, components side
//! by side in them, moved by swapping places.

use hiero_core::engine::{
    Created, DiagramType, Dir, El, Engine, Laid, Placement, list_labels, validate_label,
    validate_length,
};
use hiero_core::geometry::{CANVAS_WIDTH, Point, Size};
use hiero_core::model::{DiagramTypeId, Element, ElementId, KindInfo, NoColors, NoProps};

use crate::layout::{
    LAYERS_ORIGIN, MIN_LAYER_WIDTH, arrange_layers, characteristics, component_size,
    components_in_order, description_rows, empty_layer_height, in_order, is_layer, layers_in_order,
};
use crate::model::{
    CHARACTERISTICS_MAX, DESCRIPTION_MAX, DeployedComponent, DeployedKind, DeployedProps,
    MAX_DESCRIPTION_ROWS, MAX_INSTANCE_COUNT, MAX_PROTOCOLS, PALETTE, PROTOCOLS_MAX,
    component_colors,
};
use crate::tech_stack;

type DEngine = Engine<DeployedComponent>;
type DLaid = Laid<DeployedComponent>;

fn new_element(eng: &mut DEngine, kind: DeployedKind, label: &str) -> El<DeployedComponent> {
    Element {
        id: ElementId(eng.new_id()),
        kind,
        label: label.to_owned(),
        description: (kind == DeployedKind::Component).then(String::new),
        parent_id: None,
        props: DeployedProps::default(),
        width: None,
        height: None,
        x: 0.0,
        y: 0.0,
    }
}

/// Moves a layer up or down one place.
fn move_layer(eng: &mut DEngine, layer: &ElementId, step: i32) -> bool {
    let mut order: Vec<ElementId> = layers_in_order(eng.elements())
        .into_iter()
        .map(|l| l.id)
        .collect();
    let Some(index) = order.iter().position(|id| id == layer) else {
        return false;
    };
    let target = index as i64 + i64::from(step);
    if target < 0 || target as usize >= order.len() {
        return false;
    }
    let id = order.remove(index);
    order.insert(target as usize, id);
    let elements = in_order(eng.elements().to_vec(), &order, &[]);
    eng.set_elements(elements);
    if let Some(el) = eng.element(layer).cloned() {
        eng.announcement = format!("{}.", DeployedComponent::describe(eng, &el));
    }
    true
}

/// Swaps a component with the one beside it.
fn move_in_layer(eng: &mut DEngine, component: &ElementId, step: i32) -> bool {
    let Some(layer) = eng.element(component).and_then(|c| c.parent_id.clone()) else {
        return false;
    };
    let mut order: Vec<ElementId> = components_in_order(eng.elements(), &layer)
        .into_iter()
        .map(|c| c.id)
        .collect();
    let Some(index) = order.iter().position(|id| id == component) else {
        return false;
    };
    let target = index as i64 + i64::from(step);
    if target < 0 || target as usize >= order.len() {
        return false;
    }
    let id = order.remove(index);
    order.insert(target as usize, id);
    let elements = in_order(eng.elements().to_vec(), &[], &[(layer, order)]);
    eng.set_elements(elements);
    if let Some(el) = eng.element(component).cloned() {
        eng.announcement = format!("{}.", DeployedComponent::describe(eng, &el));
    }
    true
}

/// Moves a component to the layer above or below, as near its place there as it can go.
fn move_to_layer(eng: &mut DEngine, component: &ElementId, step: i32) -> bool {
    let layers: Vec<ElementId> = layers_in_order(eng.elements())
        .into_iter()
        .map(|l| l.id)
        .collect();
    let Some(from) = eng.element(component).and_then(|c| c.parent_id.clone()) else {
        return false;
    };
    let Some(from_index) = layers.iter().position(|l| *l == from) else {
        return false;
    };
    let to_index = from_index as i64 + i64::from(step);
    if to_index < 0 || to_index as usize >= layers.len() {
        return false;
    }
    let to = layers[to_index as usize].clone();
    let here = components_in_order(eng.elements(), &from);
    let index = here.iter().position(|c| c.id == *component).unwrap_or(0);
    let mut there: Vec<ElementId> = components_in_order(eng.elements(), &to)
        .into_iter()
        .map(|c| c.id)
        .collect();
    there.insert(index.min(there.len()), component.clone());
    let left: Vec<ElementId> = here
        .into_iter()
        .filter(|c| c.id != *component)
        .map(|c| c.id)
        .collect();
    let elements = in_order(eng.elements().to_vec(), &[], &[(from, left), (to, there)]);
    eng.set_elements(elements);
    if let Some(el) = eng.element(component).cloned() {
        eng.announcement = format!("{}.", DeployedComponent::describe(eng, &el));
    }
    true
}

/// Such as "2 of 3 in Web", or "layer 1 of 2". Empty for a component outside any layer.
pub fn place_of(eng: &DEngine, element: &El<DeployedComponent>) -> String {
    let elements = eng.elements();
    if is_layer(element) {
        let layers = layers_in_order(elements);
        let index = layers
            .iter()
            .position(|l| l.id == element.id)
            .map_or(0, |i| i + 1);
        return format!("layer {index} of {}", layers.len());
    }
    let current = eng.element(&element.id).unwrap_or(element);
    let Some(layer) = current.parent_id.as_ref().and_then(|p| eng.element(p)) else {
        return String::new();
    };
    let siblings = components_in_order(elements, &layer.id);
    let index = siblings
        .iter()
        .position(|c| c.id == element.id)
        .map_or(0, |i| i + 1);
    format!("{index} of {} in {}", siblings.len(), layer.label)
}

/// Adds or removes a technology from a component's tech stack, keeping the catalogue's order.
pub fn toggle_tech(eng: &mut DEngine, id: &ElementId, tech: &str) {
    eng.replace(id, |mut stored| {
        let mut ids: Vec<String> = stored.props.tech_stack.clone().unwrap_or_default();
        if let Some(i) = ids.iter().position(|t| t == tech) {
            ids.remove(i);
        } else {
            ids.push(tech.to_owned());
        }
        let ordered: Vec<String> = tech_stack::items(&ids)
            .iter()
            .map(|i| i.id.to_owned())
            .collect();
        stored.props.tech_stack = (!ordered.is_empty()).then_some(ordered);
        stored
    });
}

impl DiagramType for DeployedComponent {
    const ID: DiagramTypeId = DiagramTypeId::DeployedComponent;
    type Kind = DeployedKind;
    type Props = DeployedProps;
    type Arrow = NoProps;
    type Colors = NoColors;

    fn palette() -> &'static [KindInfo<DeployedKind>] {
        &PALETTE
    }

    fn arrange(_eng: &DEngine, elements: Vec<El<Self>>) -> Vec<El<Self>> {
        arrange_layers(elements)
    }

    /// A component takes the size its description needs; a layer, the size of an empty one.
    fn natural_size(element: &El<Self>, _info: &'static KindInfo<DeployedKind>) -> Size {
        if is_layer(element) {
            Size::new(
                MIN_LAYER_WIDTH,
                empty_layer_height(characteristics(element)),
            )
        } else {
            component_size(element.description.as_deref())
        }
    }

    /// A layer moves only up and down. A component lands in the layer its middle is over, or
    /// the nearest one.
    fn constrain_move(eng: &DEngine, element: &DLaid, placement: Placement) -> Placement {
        if is_layer(&element.el) {
            return Placement {
                position: Point::new(element.rect.x, placement.position.y),
                parent_id: None,
            };
        }
        let layers: Vec<&DLaid> = eng.layout().iter().filter(|e| is_layer(&e.el)).collect();
        if layers.is_empty() {
            return placement;
        }
        let middle = placement.position.y + element.rect.height / 2.0;
        let distance = |l: &DLaid| {
            if middle < l.rect.y {
                l.rect.y - middle
            } else {
                (middle - l.rect.y - l.rect.height).max(0.0)
            }
        };
        let layer = layers
            .iter()
            .copied()
            .reduce(|nearest, each| {
                if distance(each) < distance(nearest) {
                    each
                } else {
                    nearest
                }
            })
            .expect("there's at least one layer");
        Placement {
            parent_id: Some(layer.id().clone()),
            ..placement
        }
    }

    /// A new layer, or a new component in the layer it's placed in. The first component on the
    /// diagram comes with a layer to hold it.
    fn created(
        eng: &mut DEngine,
        kind: &'static KindInfo<DeployedKind>,
        placement: Placement,
    ) -> Created<Self> {
        if kind.kind == DeployedKind::Layer {
            let mut layer = new_element(eng, DeployedKind::Layer, kind.label);
            layer.x = LAYERS_ORIGIN.x;
            layer.y = placement.position.y;
            return Created {
                added: vec![layer],
                existing: None,
            };
        }
        let mut component = new_element(eng, DeployedKind::Component, kind.label);
        component.x = placement.position.x;
        component.y = placement.position.y;
        if placement.parent_id.is_some() || eng.elements().iter().any(is_layer) {
            component.parent_id = placement.parent_id;
            return Created {
                added: vec![component],
                existing: None,
            };
        }
        let mut layer = new_element(eng, DeployedKind::Layer, "Layer");
        layer.x = LAYERS_ORIGIN.x;
        layer.y = LAYERS_ORIGIN.y;
        component.parent_id = Some(layer.id.clone());
        Created {
            added: vec![component, layer],
            existing: None,
        }
    }

    /// A layer goes under the selected one, or the selected component's, or the last. A
    /// component goes after the selected one, or at the end of the selected layer, or the last.
    fn add_at_free_position(eng: &mut DEngine, kind: &'static KindInfo<DeployedKind>) {
        let selected = eng.selected().cloned();
        let layers: Vec<DLaid> = eng
            .layout()
            .iter()
            .filter(|e| is_layer(&e.el))
            .cloned()
            .collect();
        let layer = match &selected {
            Some(s) if is_layer(&s.el) => Some(s.clone()),
            Some(s) => layers
                .iter()
                .find(|l| Some(l.id()) == s.el.parent_id.as_ref())
                .cloned(),
            None => None,
        }
        .or_else(|| layers.last().cloned());
        if kind.kind == DeployedKind::Layer {
            let height = empty_layer_height(&[]);
            let y = layer.as_ref().map_or(LAYERS_ORIGIN.y, |l| {
                l.rect.y + l.rect.height / 2.0 - height / 2.0
            });
            eng.add(
                kind,
                Placement {
                    position: Point::new(LAYERS_ORIGIN.x, y),
                    parent_id: None,
                },
            );
            return;
        }
        let after = selected.filter(|s| {
            !is_layer(&s.el) && s.el.parent_id.as_ref() == layer.as_ref().map(Laid::id)
        });
        let width = component_size(Some("")).width;
        let x = after.as_ref().map_or(CANVAS_WIDTH, |a| {
            a.rect.x + a.rect.width / 2.0 - width / 2.0
        });
        eng.add(
            kind,
            Placement {
                position: Point::new(x, layer.as_ref().map_or(LAYERS_ORIGIN.y, |l| l.rect.y)),
                parent_id: layer.map(|l| l.id().clone()),
            },
        );
    }

    /// The arrow keys swap a component with the one beside it, move it to the layer above or
    /// below, and swap a layer with the one above or below.
    fn element_key(eng: &mut DEngine, id: &ElementId, dir: Dir, _shift: bool) -> bool {
        if eng.in_group(id) {
            eng.announcement = "Select just one element to move it with the arrow keys.".into();
            return true;
        }
        let Some(element) = eng.element(id).cloned() else {
            return true;
        };
        let step = dir.sign();
        if is_layer(&element) {
            if !dir.is_across() {
                move_layer(eng, id, step);
            }
        } else if dir.is_across() {
            move_in_layer(eng, id, step);
        } else {
            move_to_layer(eng, id, step);
        }
        true
    }

    /// Removing a layer removes its components too.
    fn remove(eng: &mut DEngine, id: &ElementId) {
        let Some(element) = eng.element(id).cloned() else {
            return;
        };
        if !is_layer(&element) {
            eng.base_remove(id);
            return;
        }
        let held: Vec<ElementId> = eng
            .elements()
            .iter()
            .filter(|e| e.parent_id.as_ref() == Some(id))
            .map(|e| e.id.clone())
            .collect();
        let remaining = eng
            .elements()
            .iter()
            .filter(|e| e.parent_id.as_ref() != Some(id))
            .cloned()
            .collect();
        eng.set_elements(remaining);
        eng.base_remove(id);
        if !held.is_empty() {
            eng.announcement = format!(
                "{} and its {} {} removed.",
                element.label,
                held.len(),
                if held.len() == 1 {
                    "component"
                } else {
                    "components"
                }
            );
        }
    }

    /// Such as "API, 2 of 3 in Web", or "Web, layer 1 of 2", where it is once arranged.
    fn describe(eng: &DEngine, element: &El<Self>) -> String {
        let place = place_of(eng, element);
        if place.is_empty() {
            element.label.clone()
        } else {
            format!("{}, {place}", element.label)
        }
    }

    /// Where it is, then a layer's characteristics, or a component's protocols, description,
    /// tech stack, and instances.
    fn hidden_details(eng: &DEngine, element: &DLaid) -> String {
        let el = &element.el;
        let mut parts: Vec<String> = Vec::new();
        if is_layer(el) {
            parts.push("Layer".into());
            parts.push(place_of(eng, el));
            if let Some(c) = el.props.characteristics.as_ref().filter(|c| !c.is_empty()) {
                parts.push(format!("characteristics {}", list_labels(c)));
            }
        } else {
            parts.push(component_colors(el.props.dependency).label.into());
            parts.push(place_of(eng, el));
            if let Some(p) = el.props.protocols.as_ref().filter(|p| !p.is_empty()) {
                parts.push(format!("protocols {}", list_labels(p)));
            }
            let rows = description_rows(el.description.as_deref());
            if !rows.is_empty() {
                parts.push(format!("description: {}", rows.join("; ")));
            }
            let tech = tech_stack::items(el.props.tech_stack.as_deref().unwrap_or_default());
            if !tech.is_empty() {
                let labels: Vec<String> = tech.iter().map(|t| t.label.to_owned()).collect();
                parts.push(format!("built with {}", list_labels(&labels)));
            }
            let count = el.props.instance_count.unwrap_or(1);
            if count > 1 {
                parts.push(format!("{count} instances"));
            }
        }
        parts.retain(|p| !p.is_empty());
        format!(", {}", parts.join(", "))
    }

    /// A layer keeps only its name and characteristics. A component moved to another layer
    /// goes at its end.
    fn apply_element(
        eng: &mut DEngine,
        id: &ElementId,
        mut with: El<Self>,
        parent_id: Option<ElementId>,
    ) {
        let Some(selected) = eng.element(id).cloned() else {
            return;
        };
        if is_layer(&selected) {
            let label = with.label.clone();
            let characteristics = with.props.characteristics.clone().filter(|c| !c.is_empty());
            eng.replace(id, |mut layer| {
                layer.label = label;
                layer.props.characteristics = characteristics;
                layer
            });
            return;
        }
        with.props.characteristics = None;
        if with.props.instance_count == Some(1) {
            with.props.instance_count = None;
        }
        with.props.protocols = with.props.protocols.filter(|p| !p.is_empty());
        if parent_id == selected.parent_id {
            eng.replace(id, |_| with);
        } else {
            let y = eng.laid(id).map_or(selected.y, |l| l.rect.y);
            eng.base_commit_move(
                id,
                Placement {
                    position: Point::new(CANVAS_WIDTH, y),
                    parent_id,
                },
                Some(with),
            );
        }
    }

    fn validate_element(element: &El<Self>) -> Result<(), String> {
        validate_label(&element.label)?;
        let characteristics = element
            .props
            .characteristics
            .clone()
            .unwrap_or_default()
            .join(", ");
        validate_length("Characteristics", &characteristics, CHARACTERISTICS_MAX)?;
        let protocols = element.props.protocols.clone().unwrap_or_default();
        validate_length("Protocols", &protocols.join(", "), PROTOCOLS_MAX)?;
        if protocols.len() > MAX_PROTOCOLS {
            return Err(format!("Enter up to {MAX_PROTOCOLS}, separated by commas."));
        }
        let description = element.description.as_deref().unwrap_or_default();
        validate_length("Description", description, DESCRIPTION_MAX)?;
        if description.split('\n').count() > MAX_DESCRIPTION_ROWS {
            return Err(format!("Use {MAX_DESCRIPTION_ROWS} lines or fewer."));
        }
        match element.props.instance_count {
            Some(n) if n == 0 || n > MAX_INSTANCE_COUNT => Err(format!(
                "Enter a whole number from 1 to {MAX_INSTANCE_COUNT}."
            )),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    fn engine() -> DEngine {
        let mut n = 0;
        Engine::new(Vec::new(), Vec::new(), NoColors {})
            .with_measure(&ESTIMATE)
            .with_ids(move || {
                n += 1;
                format!("id{n}")
            })
    }

    fn id(s: &str) -> ElementId {
        s.into()
    }

    fn labels_in(e: &DEngine, layer: &str) -> Vec<String> {
        components_in_order(e.elements(), &id(layer))
            .into_iter()
            .map(|c| c.label)
            .collect()
    }

    fn rename(e: &mut DEngine, i: &str, label: &str) {
        let label = label.to_owned();
        e.replace(&id(i), |mut el| {
            el.label = label;
            el
        });
    }

    #[test]
    fn the_first_component_comes_with_a_layer() {
        let mut e = engine();
        e.add_at_free_position(DeployedKind::Component);
        assert_eq!(e.elements().len(), 2);
        let layer = e.laid(&id("id2")).unwrap();
        assert_eq!(
            layer.rect,
            hiero_core::geometry::Rect::new(40.0, 40.0, 860.0, 250.0)
        );
        let component = e.laid(&id("id1")).unwrap();
        assert_eq!(
            component.rect,
            hiero_core::geometry::Rect::new(60.0, 80.0, 260.0, 190.0)
        );
    }

    #[test]
    fn components_go_after_the_selected_one_and_swap() {
        let mut e = engine();
        e.add_at_free_position(DeployedKind::Component);
        rename(&mut e, "id1", "A");
        e.add_at_free_position(DeployedKind::Component);
        rename(&mut e, "id3", "B");
        e.select(&id("id1"));
        e.add_at_free_position(DeployedKind::Component);
        rename(&mut e, "id4", "C");
        assert_eq!(labels_in(&e, "id2"), ["A", "C", "B"]);
        e.select(&id("id4"));
        e.arrow_key(&id("id4"), Dir::Right, false);
        assert_eq!(labels_in(&e, "id2"), ["A", "B", "C"]);
        assert_eq!(e.announcement, "C, 3 of 3 in Layer.");
    }

    #[test]
    fn components_move_between_layers_and_layers_reorder() {
        let mut e = engine();
        e.add_at_free_position(DeployedKind::Component);
        e.add_at_free_position(DeployedKind::Layer);
        e.arrow_key(&id("id1"), Dir::Down, false);
        assert_eq!(e.element(&id("id1")).unwrap().parent_id, Some(id("id3")));
        e.arrow_key(&id("id3"), Dir::Up, false);
        let order: Vec<ElementId> = layers_in_order(e.elements())
            .into_iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(order, [id("id3"), id("id2")]);
    }

    #[test]
    fn removing_a_layer_removes_its_components() {
        let mut e = engine();
        e.add_at_free_position(DeployedKind::Component);
        e.add_at_free_position(DeployedKind::Component);
        e.select(&id("id2"));
        e.delete_selection();
        assert!(e.elements().is_empty());
        assert_eq!(e.announcement, "Layer and its 2 components removed.");
    }

    #[test]
    fn tech_stack_keeps_catalogue_order() {
        let mut e = engine();
        e.add_at_free_position(DeployedKind::Component);
        toggle_tech(&mut e, &id("id1"), "docker");
        toggle_tech(&mut e, &id("id1"), "aws");
        assert_eq!(
            e.element(&id("id1")).unwrap().props.tech_stack.as_deref(),
            Some(&["aws".to_owned(), "docker".to_owned()][..])
        );
        toggle_tech(&mut e, &id("id1"), "aws");
        toggle_tech(&mut e, &id("id1"), "docker");
        assert_eq!(e.element(&id("id1")).unwrap().props.tech_stack, None);
    }

    #[test]
    fn hidden_details_read_like_the_bars() {
        let mut e = engine();
        e.add_at_free_position(DeployedKind::Component);
        e.replace(&id("id1"), |mut c| {
            c.props.protocols = Some(vec!["HTTPS".into(), "JWT".into()]);
            c.props.instance_count = Some(3);
            c.description = Some("Serves pages\nCaches".into());
            c
        });
        let laid = e.laid(&id("id1")).unwrap().clone();
        assert_eq!(
            e.accessible_label(&laid),
            "Component, Core Component, 1 of 1 in Layer, protocols HTTPS and JWT, description: Serves pages; Caches, 3 instances"
        );
    }
}
