//! Every diagram type Arch Hiero knows, behind one object-safe interface that doesn't need a
//! UI: new content, validation, layout, scenes and exports, and the edits agents make. The app
//! and the MCP server both use it, so a new diagram type is added in one place.

pub mod edit;

use std::collections::BTreeMap;
use std::marker::PhantomData;

use hiero_conceptual::Conceptual;
use hiero_core::engine::{DiagramType, Engine};
use hiero_core::export::drawio::Heading;
use hiero_core::export::svg::scene_to_svg;
use hiero_core::file::{DiagramFileError, diagram_from_json, read_zip_json};
use hiero_core::file_check;
use hiero_core::geometry::Rect;
use hiero_core::model::{ColorScheme, DiagramTypeId, ElementId, Resizable};
use hiero_core::scene::Scene;
use hiero_core::stored::{ContentError, StoredContent, StoredDiagram};
use hiero_deployed::DeployedComponent;
use hiero_physical::Physical;
use serde::Serialize;
use serde_json::{Map, Value};

pub use edit::{EditError, EditOp, EditOutcome};

/// A kind of element, as agents and the docs describe it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindDescription {
    pub kind: String,
    pub label: &'static str,
    pub has_description: bool,
    pub can_contain: Vec<String>,
    pub connects_to: Vec<String>,
    pub resizable: &'static str,
}

/// One of the values a prop can take, such as a technology for a deployed component's
/// `techStack`.
#[derive(Debug, Clone, Serialize)]
pub struct PropValue {
    pub id: &'static str,
    pub label: &'static str,
    /// What it's grouped under, such as "Database".
    pub group: &'static str,
}

/// A diagram type, without its UI.
pub trait TypeOps: Send + Sync {
    fn id(&self) -> DiagramTypeId;
    fn label(&self) -> &'static str {
        self.id().label()
    }
    /// The colours a diagram can change, with their labels.
    fn color_fields(&self) -> &'static [(&'static str, &'static str)];
    /// Every colour's default.
    fn default_colors(&self) -> BTreeMap<String, String>;
    /// Only the colours in `colors` that differ from the defaults, and are valid.
    fn normalize_colors(&self, colors: &BTreeMap<String, String>) -> BTreeMap<String, String>;
    /// What a new diagram starts with, such as a physical diagram's zones.
    fn new_content(&self) -> StoredContent;
    /// Checks that content can be opened as this type, and that every element and arrow is valid.
    fn validate(&self, content: &StoredContent) -> Result<(), ContentError>;
    /// Checks a file's diagram object of this type, from its description on, returning the
    /// message for the first rule it breaks (see `hiero_core::file_check`).
    fn check_file_diagram(&self, diagram: &Map<String, Value>) -> Result<(), String>;
    /// The content as the type arranges it, such as zones filling their networks.
    fn arranged(
        &self,
        content: &StoredContent,
        colors: &BTreeMap<String, String>,
    ) -> Result<StoredContent, ContentError>;
    /// Each element's laid-out area, in drawing order.
    fn layout(&self, diagram: &StoredDiagram) -> Result<Vec<(ElementId, Rect)>, ContentError>;
    /// The diagram drawn.
    fn scene(&self, diagram: &StoredDiagram) -> Result<Scene, ContentError>;
    /// The palette, for agents.
    fn palette(&self) -> Vec<KindDescription>;
    /// What agents can put in an element's and an arrow's `props`.
    fn props_doc(&self) -> &'static str;
    /// The values that props with a fixed list take, by prop name.
    fn prop_values(&self) -> BTreeMap<&'static str, Vec<PropValue>>;
    /// Applies agent edits, in order, all or nothing.
    fn apply(&self, diagram: &StoredDiagram, ops: &[EditOp]) -> Result<EditOutcome, EditError>;

    /// The diagram as a draw.io file, with its name and description above it.
    fn to_drawio(&self, diagram: &StoredDiagram) -> Result<String, ContentError>;

    /// The diagram as an SVG document.
    fn to_svg(&self, diagram: &StoredDiagram) -> Result<String, ContentError> {
        Ok(scene_to_svg(&diagram.meta.name, &self.scene(diagram)?))
    }
}

/// The scene and agent docs a type adds to its rules.
pub trait Drawn: DiagramType {
    fn scene(eng: &Engine<Self>) -> Scene;
    fn drawio(eng: &Engine<Self>, title: &str, heading: &Heading) -> String;
    const PROPS_DOC: &'static str;
    /// The values that props with a fixed list take, by prop name.
    fn prop_values() -> BTreeMap<&'static str, Vec<PropValue>> {
        BTreeMap::new()
    }
}

impl Drawn for Conceptual {
    fn drawio(eng: &Engine<Self>, title: &str, heading: &Heading) -> String {
        hiero_conceptual::scene::to_drawio(eng, title, Some(heading))
    }
    fn scene(eng: &Engine<Self>) -> Scene {
        hiero_conceptual::scene::scene(eng)
    }
    const PROPS_DOC: &'static str = "Element props: actor {agent: bool}; application {cloudHosted: bool} and a description (≤ 500); component {componentType: \"database\" | \"storage\"}; file has none (a page in the data colour, held like an entity). Entities and files take arrows but don't start them. Actors take arrows from other actors, applications, components, jobs, and processes, such as a system that sends someone a report. Arrow props: {label (≤ 100), mode: \"manual\" (dashed; omit for automated), bidirectional: bool, connector (≤ 20, e.g. \"API\"), transformation (≤ 40), manualOffset: number (pins it straight there; arrows run straight where their sides overlap anyway)}.";
}

impl Drawn for Physical {
    fn drawio(eng: &Engine<Self>, title: &str, heading: &Heading) -> String {
        hiero_physical::scene::to_drawio(eng, title, Some(heading))
    }
    fn scene(eng: &Engine<Self>) -> Scene {
        hiero_physical::scene::scene(eng)
    }
    const PROPS_DOC: &'static str = "Element props: server {serverType: \"virtual-machine\" | \"container\" (omit for physical), instanceCount: 1-9999, or text for a minimum \"50+\" or a range \"50-100\"}; database {databaseType: \"postgresql\" | \"mysql\" | \"oracle\" | \"mongodb\" | \"mssql\" | \"cassandra\", instanceCount (the node count, as for a server)}; zone {firewall: true (omit for none): each arrow from outside the zone shows a firewall where it crosses the zone's edge; and, outside a network, zoneColor: \"red\" | \"yellow\" | \"green\" (omit for blue)}. Zones go in networks and fill them; users, servers, databases, load balancers, gateways, and storage go in zones; firewalls stand on their own. Arrow props: {protocol (required, ≤ 20), port (required, ≤ 20)}; a new arrow starts with the target's usual protocol and port.";
}

impl Drawn for DeployedComponent {
    fn drawio(eng: &Engine<Self>, title: &str, heading: &Heading) -> String {
        hiero_deployed::drawio::to_drawio(eng, title, Some(heading))
    }
    fn scene(eng: &Engine<Self>) -> Scene {
        hiero_deployed::scene::scene(eng)
    }
    const PROPS_DOC: &'static str = "No arrows. Layer props: {characteristics: [string]} (joined ≤ 200). Component props: {protocols: [string] (≤ 5, joined ≤ 100), techStack: [id from propValues.techStack], dependency: \"internal\" | \"external\" (omit for a core component), instanceCount: 1-9999, or text for a minimum \"50+\" or a range \"50-100\"} and a description with one bullet per line (≤ 2000, ≤ 20 lines). Layers stack top to bottom and components sit side by side in them in order; positions only set the order.";
    fn prop_values() -> BTreeMap<&'static str, Vec<PropValue>> {
        let tech = hiero_deployed::tech_stack::TECH_STACK
            .iter()
            .map(|t| PropValue {
                id: t.id,
                label: t.label,
                group: t.category.label(),
            })
            .collect();
        BTreeMap::from([("techStack", tech)])
    }
}

struct Entry<T>(PhantomData<fn() -> T>);

fn kind_name<K: Serialize>(kind: &K) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// An engine for stored content, measuring text with the bundled font.
pub fn engine_for<T: DiagramType>(
    content: &StoredContent,
    colors: &BTreeMap<String, String>,
) -> Result<Engine<T>, ContentError> {
    let (elements, connections) = content.to_typed()?;
    Ok(Engine::new(
        elements,
        connections,
        T::Colors::from_overrides(colors),
    ))
}

impl<T: Drawn> TypeOps for Entry<T> {
    fn id(&self) -> DiagramTypeId {
        T::ID
    }

    fn color_fields(&self) -> &'static [(&'static str, &'static str)] {
        T::Colors::FIELDS
    }

    fn default_colors(&self) -> BTreeMap<String, String> {
        let defaults = T::Colors::default();
        T::Colors::FIELDS
            .iter()
            .filter_map(|(k, _)| defaults.get(k).map(|v| ((*k).to_owned(), v)))
            .collect()
    }

    fn normalize_colors(&self, colors: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        T::Colors::from_overrides(colors).overrides()
    }

    fn new_content(&self) -> StoredContent {
        let eng = Engine::<T>::new_diagram(T::Colors::default());
        StoredContent::from_typed(eng.elements(), eng.connections())
    }

    fn validate(&self, content: &StoredContent) -> Result<(), ContentError> {
        let (elements, connections) = content.to_typed::<T::Kind, T::Props, T::Arrow>()?;
        for element in &elements {
            T::validate_element(element)
                .map_err(|e| ContentError(format!("“{}”: {e}", element.label)))?;
        }
        for connection in &connections {
            T::validate_arrow(&connection.props).map_err(ContentError)?;
        }
        if !T::ID.has_arrows() && !connections.is_empty() {
            return Err(ContentError(format!(
                "{} diagrams have no arrows.",
                T::ID.label()
            )));
        }
        Ok(())
    }

    fn check_file_diagram(&self, diagram: &Map<String, Value>) -> Result<(), String> {
        file_check::check_diagram::<T>(diagram)
    }

    fn arranged(
        &self,
        content: &StoredContent,
        colors: &BTreeMap<String, String>,
    ) -> Result<StoredContent, ContentError> {
        let mut eng = engine_for::<T>(content, colors)?;
        let elements = eng.elements().to_vec();
        eng.set_elements(elements);
        Ok(StoredContent::from_typed(eng.elements(), eng.connections()))
    }

    fn layout(&self, diagram: &StoredDiagram) -> Result<Vec<(ElementId, Rect)>, ContentError> {
        let eng = engine_for::<T>(&diagram.content, &diagram.colors)?;
        Ok(eng
            .layout()
            .iter()
            .map(|l| (l.el.id.clone(), l.rect))
            .collect())
    }

    fn scene(&self, diagram: &StoredDiagram) -> Result<Scene, ContentError> {
        let eng = engine_for::<T>(&diagram.content, &diagram.colors)?;
        Ok(T::scene(&eng))
    }

    fn palette(&self) -> Vec<KindDescription> {
        T::palette()
            .iter()
            .map(|k| KindDescription {
                kind: kind_name(&k.kind),
                label: k.label,
                has_description: k.has_description,
                can_contain: k.can_contain.iter().map(kind_name).collect(),
                connects_to: k.connects_to.iter().map(kind_name).collect(),
                resizable: match k.resizable {
                    Resizable::No if !k.can_contain.is_empty() => "both",
                    Resizable::No => "no",
                    Resizable::Both => "both",
                    Resizable::Width => "width",
                },
            })
            .collect()
    }

    fn props_doc(&self) -> &'static str {
        T::PROPS_DOC
    }

    fn prop_values(&self) -> BTreeMap<&'static str, Vec<PropValue>> {
        T::prop_values()
    }

    fn apply(&self, diagram: &StoredDiagram, ops: &[EditOp]) -> Result<EditOutcome, EditError> {
        edit::apply::<T>(diagram, ops)
    }

    fn to_drawio(&self, diagram: &StoredDiagram) -> Result<String, ContentError> {
        let eng = engine_for::<T>(&diagram.content, &diagram.colors)?;
        let heading = Heading {
            name: diagram.meta.name.clone(),
            description: diagram.meta.description.clone(),
        };
        Ok(T::drawio(&eng, &diagram.meta.name, &heading))
    }
}

static CONCEPTUAL: Entry<Conceptual> = Entry(PhantomData);
static PHYSICAL: Entry<Physical> = Entry(PhantomData);
static DEPLOYED: Entry<DeployedComponent> = Entry(PhantomData);

/// Every diagram type, in menu order.
pub fn types() -> [&'static dyn TypeOps; 3] {
    [&CONCEPTUAL, &DEPLOYED, &PHYSICAL]
}

/// The operations of a diagram type.
pub fn ops(id: DiagramTypeId) -> &'static dyn TypeOps {
    match id {
        DiagramTypeId::Conceptual => &CONCEPTUAL,
        DiagramTypeId::DeployedComponent => &DEPLOYED,
        DiagramTypeId::Physical => &PHYSICAL,
    }
}

/// The diagram in a parsed diagram file, if the file follows every rule in
/// `hiero_core::file_check`. Otherwise, the message for the first rule it breaks.
pub fn diagram_from_file(file: &Value) -> Result<StoredDiagram, DiagramFileError> {
    let (diagram, diagram_type) = file_check::check_file(file).map_err(DiagramFileError)?;
    let ops = ops(diagram_type);
    ops.check_file_diagram(diagram).map_err(DiagramFileError)?;
    let stored = diagram_from_json(&file["diagram"])?;
    // The file's rules include the type's own, so this only guards against the two drifting
    // apart and letting in a diagram the editor can't open.
    ops.validate(&stored.content)
        .map_err(|e| DiagramFileError(e.0))?;
    Ok(stored)
}

/// The diagram in a `.zip` diagram file, checked as `diagram_from_file` checks it.
pub fn read_diagram_file(bytes: &[u8]) -> Result<StoredDiagram, DiagramFileError> {
    diagram_from_file(&read_zip_json(bytes)?)
}

/// A new diagram of `diagram_type`, with what it starts with.
pub fn new_diagram(
    diagram_type: DiagramTypeId,
    name: &str,
    description: Option<String>,
) -> StoredDiagram {
    StoredDiagram {
        meta: hiero_core::model::DiagramMeta {
            id: hiero_core::model::DiagramId::new_v4(),
            diagram_type,
            name: name.trim().to_owned(),
            description: description
                .map(|d| d.trim().to_owned())
                .filter(|d| !d.is_empty()),
            created_at: String::new(),
            updated_at: String::new(),
        },
        colors: BTreeMap::new(),
        content: ops(diagram_type).new_content(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_has_an_entry() {
        for t in types() {
            assert_eq!(ops(t.id()).id(), t.id());
            assert!(!t.palette().is_empty());
        }
        assert_eq!(ops(DiagramTypeId::Physical).new_content().elements.len(), 5);
        assert!(
            ops(DiagramTypeId::Conceptual)
                .new_content()
                .elements
                .is_empty()
        );
        assert_eq!(ops(DiagramTypeId::Conceptual).default_colors().len(), 9);
        assert_eq!(ops(DiagramTypeId::Physical).color_fields().len(), 12);
        assert!(
            ops(DiagramTypeId::DeployedComponent)
                .color_fields()
                .is_empty()
        );
    }

    #[test]
    fn seeds_draw_as_svg() {
        if let Ok(diagram) = hiero_core::file::diagram_from_json(&serde_json::json!({
            "id": "d", "name": "T", "type": "conceptual",
            "elements": [{"id": "a", "kind": "domain", "label": "Sales", "x": 40, "y": 40}]
        })) {
            let svg = ops(diagram.meta.diagram_type).to_svg(&diagram).unwrap();
            assert!(svg.contains(">Sales</text>"));
        }
    }

    #[test]
    fn validation_rejects_unknown_kinds() {
        let content = StoredContent {
            elements: vec![
                hiero_core::stored::StoredElement::from_json(&serde_json::json!({
                    "id": "a", "kind": "spaceship", "label": "X", "x": 0, "y": 0
                }))
                .unwrap(),
            ],
            connections: vec![],
        };
        assert!(ops(DiagramTypeId::Conceptual).validate(&content).is_err());
    }
}
