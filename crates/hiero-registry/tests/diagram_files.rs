//! The rules diagram files must follow to be imported, with the message for each. The
//! prototype applies the same rules with the same messages (its `import-file-sync.md`).

use hiero_core::file::{diagram_file_json, diagram_to_zip};
use hiero_core::model::DiagramTypeId;
use hiero_registry::edit::EditOp;
use hiero_registry::{diagram_from_file, new_diagram, ops, read_diagram_file};
use serde_json::{Value, json};

fn file(diagram: Value) -> Value {
    json!({"format": "arch-hiero-diagram", "version": 1, "diagram": diagram})
}

fn conceptual() -> Value {
    file(json!({
        "id": "11111111-1111-4111-8111-111111111111",
        "name": "Conceptual",
        "type": "conceptual",
        "description": "Line one\nline two",
        "colors": {"domains": "#eeeeee"},
        "elements": [
            {"id": "a1", "kind": "actor", "label": "Bot", "agent": true, "x": 40, "y": 40},
            {"id": "d1", "kind": "domain", "label": "Sales", "x": 200, "y": 40,
             "width": 600, "height": 400},
            {"id": "p1", "kind": "application", "label": "CRM", "description": "Customer\ndata",
             "cloudHosted": true, "parentId": "d1", "x": 220, "y": 100},
            {"id": "c1", "kind": "component", "label": "DB", "componentType": "database",
             "parentId": "p1", "x": 240, "y": 160},
            {"id": "r1", "kind": "process", "label": "Billing", "parentId": "c1", "x": 260, "y": 200.5},
            {"id": "e1", "kind": "entity", "label": "Order", "parentId": "p1", "x": 400, "y": 160},
            {"id": "f1", "kind": "file", "label": "Export", "x": 40, "y": 300},
            {"id": "j1", "kind": "job", "label": "Nightly", "x": 40, "y": 400},
            {"id": "s1", "kind": "section", "label": "Back office", "x": 40, "y": 600, "width": 800}
        ],
        "connections": [
            {"id": "k1", "sourceId": "a1", "targetId": "p1", "mode": "manual", "label": "uses",
             "bidirectional": true, "connector": "Web", "transformation": "Map fields",
             "manualOffset": 12},
            {"id": "k2", "sourceId": "j1", "targetId": "e1"}
        ],
        "createdAt": "2026-10-01T09:00:00.000Z",
        "updatedAt": "2026-10-02T09:00:00Z"
    }))
}

fn physical() -> Value {
    file(json!({
        "id": "22222222-2222-4222-8222-222222222222",
        "name": "Physical",
        "type": "physical",
        "physicalColors": {"servers": "#00ff00"},
        "elements": [
            {"id": "n1", "kind": "network", "label": "Intranet", "x": 40, "y": 300},
            {"id": "z1", "kind": "zone", "label": "App", "parentId": "n1", "x": 60, "y": 340},
            {"id": "z0", "kind": "zone", "label": "DMZ", "zoneColor": "red", "x": 40, "y": 40},
            {"id": "sv", "kind": "server", "label": "Web", "serverType": "container",
             "instanceCount": 3, "parentId": "z1", "x": 80, "y": 380},
            {"id": "db", "kind": "database", "label": "Main", "databaseType": "postgresql",
             "instanceCount": 2, "parentId": "z1", "x": 200, "y": 380},
            {"id": "u1", "kind": "user", "label": "Staff", "x": 600, "y": 40},
            {"id": "fw", "kind": "firewall", "label": "FW", "x": 700, "y": 40}
        ],
        "connections": [
            {"id": "q1", "sourceId": "sv", "targetId": "db", "protocol": "TCP", "port": "5432"},
            {"id": "q2", "sourceId": "u1", "targetId": "fw", "protocol": "HTTPS", "port": "443"}
        ],
        "createdAt": "2026-10-01T09:00:00.000Z",
        "updatedAt": "2026-10-02T09:00:00.000Z"
    }))
}

fn deployed() -> Value {
    file(json!({
        "id": "33333333-3333-4333-8333-333333333333",
        "name": "Deployed",
        "type": "deployed-component",
        "elements": [
            {"id": "l1", "kind": "layer", "label": "Web",
             "characteristics": ["Internet facing", "PCI DSS"], "x": 20, "y": 20},
            {"id": "m1", "kind": "component", "label": "Portal", "description": "Angular\nSPA",
             "instanceCount": 2, "protocols": ["HTTPS", "JWT"], "techStack": ["aws", "angular"],
             "dependency": "external", "parentId": "l1", "x": 40, "y": 70},
            {"id": "m2", "kind": "component", "label": "API", "parentId": "l1", "x": 320, "y": 70}
        ],
        "createdAt": "2026-10-01T09:00:00.000Z",
        "updatedAt": "2026-10-02T09:00:00.000Z"
    }))
}

/// The message for `base` after `edit`.
fn refused(mut base: Value, edit: impl FnOnce(&mut Value)) -> String {
    edit(&mut base);
    match diagram_from_file(&base) {
        Ok(_) => panic!("accepted: {base:#}"),
        Err(e) => e.0,
    }
}

fn at<'a>(file: &'a mut Value, pointer: &str) -> &'a mut serde_json::Map<String, Value> {
    file.pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("no object at {pointer}"))
}

fn set(pointer: &str, key: &str, value: Value) -> impl FnOnce(&mut Value) {
    move |file| {
        at(file, pointer).insert(key.to_owned(), value);
    }
}

fn remove(pointer: &str, key: &str) -> impl FnOnce(&mut Value) {
    move |file| {
        at(file, pointer).remove(key);
    }
}

const D: &str = "/diagram";

#[test]
fn valid_files_import_and_export_unchanged() {
    for valid in [conceptual(), physical(), deployed()] {
        let diagram = diagram_from_file(&valid).unwrap_or_else(|e| panic!("{e}"));
        let back = read_diagram_file(&diagram_to_zip(&diagram)).unwrap();
        assert_eq!(back, diagram);
        let mut written: Value = serde_json::from_str(&diagram_file_json(&diagram)).unwrap();
        let mut expected = valid.clone();
        // Numbers come back as written, and ids, keys, and order are kept.
        for v in [&mut written, &mut expected] {
            at(v, D).remove("createdAt");
            at(v, D).remove("updatedAt");
        }
        assert_eq!(written, expected);
    }
}

#[test]
fn the_file_must_be_an_arch_hiero_diagram() {
    let not_ours = "The file isn’t an ArchHiero diagram.";
    assert_eq!(
        refused(conceptual(), set("", "format", json!("x"))),
        not_ours
    );
    for version in [json!(0), json!(1.5), json!("1"), Value::Null] {
        assert_eq!(refused(conceptual(), set("", "version", version)), not_ours);
    }
    assert_eq!(
        refused(conceptual(), set("", "version", json!(2))),
        "The diagram was saved by a newer version of ArchHiero. Update it and try again."
    );
    assert_eq!(
        refused(conceptual(), set("", "zeta", json!(1))),
        "The file has a field “zeta” that isn’t known."
    );
    assert_eq!(
        refused(conceptual(), set("", "diagram", json!([]))),
        not_ours
    );
}

#[test]
fn the_diagram_s_own_fields() {
    let c = conceptual;
    assert_eq!(
        refused(c(), remove(D, "id")),
        "The diagram: “id” is missing."
    );
    assert_eq!(
        refused(c(), set(D, "id", json!(" "))),
        "The diagram: “id” must not be blank."
    );
    assert_eq!(
        refused(c(), set(D, "name", json!(" "))),
        "The diagram: Enter a name."
    );
    assert_eq!(
        refused(c(), set(D, "name", json!("😀".repeat(101)))),
        "The diagram: Name must be 100 characters or fewer."
    );
    assert!(
        diagram_from_file(&{
            let mut f = c();
            set(D, "name", json!("😀".repeat(100)))(&mut f);
            f
        })
        .is_ok()
    );
    assert_eq!(
        refused(c(), set(D, "type", json!("flowchart"))),
        "The diagram: “type” must be one of: conceptual, deployed-component, physical."
    );
    assert_eq!(
        refused(c(), set(D, "description", json!(""))),
        "The diagram: “description” must be left out when it’s empty or the default."
    );
    assert_eq!(
        refused(c(), set(D, "description", json!("x".repeat(1001)))),
        "The diagram: Description must be 1000 characters or fewer."
    );
    assert_eq!(
        refused(c(), set(D, "createdAt", json!("yesterday"))),
        "The diagram: “createdAt” must be a date and time."
    );
    assert_eq!(
        refused(c(), remove(D, "updatedAt")),
        "The diagram: “updatedAt” is missing."
    );
    assert_eq!(
        refused(c(), remove(D, "elements")),
        "The diagram: “elements” is missing."
    );
    assert_eq!(
        refused(c(), remove(D, "connections")),
        "The diagram: “connections” is missing."
    );
    assert_eq!(
        refused(deployed(), set(D, "connections", json!([]))),
        "The diagram: “connections” isn’t a field a diagram has."
    );
    assert_eq!(
        refused(c(), set(D, "createdBy", json!("hiero"))),
        "The diagram: “createdBy” isn’t a field a diagram has."
    );
    assert_eq!(
        refused(c(), |f| {
            set(D, "zeta", json!(1))(f);
            set(D, "alpha", json!(1))(f);
        }),
        "The diagram: “alpha” isn’t a field a diagram has."
    );
}

#[test]
fn colours() {
    let c = conceptual;
    assert_eq!(
        refused(c(), set(D, "colors", json!({}))),
        "The diagram: “colors” must be left out when it’s empty or the default."
    );
    assert_eq!(
        refused(c(), set(D, "colors", json!("blue"))),
        "The diagram: “colors” must be an object."
    );
    assert_eq!(
        refused(c(), set(D, "colors", json!({"nope": "#000000"}))),
        "The diagram: The colour “nope” isn’t one this type has."
    );
    assert_eq!(
        refused(c(), set(D, "colors", json!({"lines": "#FF0000"}))),
        "The diagram: The colour “lines” must be written as #rrggbb."
    );
    assert_eq!(
        refused(c(), set(D, "colors", json!({"domains": "#f5f5f5"}))),
        "The diagram: The colour “domains” must be left out when it’s the default."
    );
    assert_eq!(
        refused(physical(), set(D, "colors", json!({"servers": "#00ff00"}))),
        "The diagram: “colors” isn’t a field a diagram has."
    );
    assert_eq!(
        refused(deployed(), set(D, "colors", json!({"lines": "#000000"}))),
        "The diagram: “colors” isn’t a field a diagram has."
    );
}

#[test]
fn elements_on_their_own() {
    let c = conceptual;
    let bot = "/diagram/elements/0";
    assert_eq!(
        refused(c(), |f| at(f, D)["elements"][0] = json!(3)),
        "An element must be an object."
    );
    assert_eq!(
        refused(c(), set(bot, "kind", json!("spaceship"))),
        "“Bot”: “kind” must be one of: actor, domain, application, component, process, job, \
         entity, file, section."
    );
    assert_eq!(
        refused(c(), set(bot, "label", json!(""))),
        "The element a1: Enter a name."
    );
    assert_eq!(refused(c(), remove(bot, "x")), "“Bot”: “x” is missing.");
    assert_eq!(
        refused(c(), set(bot, "y", json!("1"))),
        "“Bot”: “y” must be a number."
    );
    assert_eq!(
        refused(c(), set(bot, "width", json!(0))),
        "“Bot”: “width” must be greater than 0."
    );
    assert_eq!(
        refused(c(), set(bot, "parentId", json!(""))),
        "“Bot”: “parentId” must be left out when it’s empty or the default."
    );
    assert_eq!(
        refused(c(), set(bot, "description", json!("Hi"))),
        "“Bot”: “description” isn’t a field an Actor has."
    );
    assert_eq!(
        refused(c(), set(bot, "cloudHosted", json!(true))),
        "“Bot”: “cloudHosted” isn’t a field an Actor has."
    );
    assert_eq!(
        refused(c(), set(bot, "agent", json!(false))),
        "“Bot”: “agent” must be left out when it’s empty or the default."
    );
    assert_eq!(
        refused(c(), set(bot, "agent", json!("yes"))),
        "“Bot”: “agent” must be true."
    );
    let db = "/diagram/elements/3";
    assert_eq!(
        refused(c(), set(db, "componentType", json!("default"))),
        "“DB”: “componentType” must be left out when it’s empty or the default."
    );
    assert_eq!(
        refused(c(), set(db, "componentType", json!("cache"))),
        "“DB”: “componentType” must be one of: database, storage."
    );
    assert_eq!(
        refused(
            c(),
            set("/diagram/elements/2", "description", json!("x".repeat(501)))
        ),
        "“CRM”: Description must be 500 characters or fewer."
    );
    // An element's description may be empty.
    for empty in ["", "  "] {
        let mut file = c();
        set("/diagram/elements/2", "description", json!(empty))(&mut file);
        diagram_from_file(&file).unwrap_or_else(|e| panic!("{e}"));
    }
    assert_eq!(
        refused(c(), set("/diagram/elements/2", "description", json!(null))),
        "“CRM”: “description” must be text."
    );

    let p = physical;
    let web = "/diagram/elements/3";
    assert_eq!(
        refused(p(), set(web, "serverType", json!("physical"))),
        "“Web”: “serverType” must be left out when it’s empty or the default."
    );
    assert_eq!(
        refused(p(), set(web, "instanceCount", json!(1))),
        "“Web”: “instanceCount” must be left out when it’s empty or the default."
    );
    for count in [json!(0), json!(2.5), json!(10_000)] {
        assert_eq!(
            refused(p(), set(web, "instanceCount", count)),
            "“Web”: Enter a whole number from 1 to 9999."
        );
    }
    // A minimum or a range is text, in one spelling.
    for range in ["50+", "50-100"] {
        let mut file = p();
        set(web, "instanceCount", json!(range))(&mut file);
        diagram_from_file(&file).unwrap_or_else(|e| panic!("{e}"));
    }
    assert_eq!(
        refused(p(), set(web, "instanceCount", json!("3"))),
        "“Web”: “instanceCount” must be a number."
    );
    assert_eq!(
        refused(p(), set(web, "instanceCount", json!("50 - 100"))),
        "“Web”: “instanceCount” must be written as 50-100."
    );
    for bad in ["lots", "100-50", "0+", "50-10000"] {
        assert_eq!(
            refused(p(), set(web, "instanceCount", json!(bad))),
            "“Web”: Enter a whole number from 1 to 9999, or a range such as 50+ or 50-100.",
            "{bad}"
        );
    }
    assert_eq!(
        refused(p(), set("/diagram/elements/2", "zoneColor", json!("blue"))),
        "“DMZ”: “zoneColor” must be left out when it’s empty or the default."
    );
    assert_eq!(
        refused(p(), set("/diagram/elements/5", "instanceCount", json!(2))),
        "“Staff”: “instanceCount” isn’t a field a User has."
    );

    let d = deployed;
    let portal = "/diagram/elements/1";
    assert_eq!(
        refused(d(), set(portal, "description", json!("a\n".repeat(20)))),
        "“Portal”: Use 20 lines or fewer."
    );
    assert_eq!(
        refused(d(), set(portal, "protocols", json!([]))),
        "“Portal”: “protocols” must be left out when it’s empty or the default."
    );
    assert_eq!(
        refused(d(), set(portal, "protocols", json!(["", "JWT"]))),
        "“Portal”: “protocols” must be a list of text."
    );
    assert_eq!(
        refused(
            d(),
            set(portal, "protocols", json!(["a", "b", "c", "d", "e", "f"]))
        ),
        "“Portal”: Enter up to 5, separated by commas."
    );
    assert_eq!(
        refused(
            d(),
            set(portal, "protocols", json!(["x".repeat(50), "y".repeat(49)]))
        ),
        "“Portal”: Protocols must be 100 characters or fewer."
    );
    assert_eq!(
        refused(d(), set(portal, "techStack", json!(["nope"]))),
        "“Portal”: “nope” isn’t a known technology."
    );
    assert_eq!(
        refused(d(), set(portal, "techStack", json!(["aws", "aws"]))),
        "“Portal”: “aws” is listed twice."
    );
    assert_eq!(
        refused(d(), set(portal, "techStack", json!(["angular", "aws"]))),
        "“Portal”: “aws” must come before “angular”."
    );
    assert_eq!(
        refused(d(), set(portal, "dependency", json!("core"))),
        "“Portal”: “dependency” must be one of: internal, external."
    );
    assert_eq!(
        refused(
            d(),
            set(
                "/diagram/elements/0",
                "characteristics",
                json!(["x".repeat(201)])
            )
        ),
        "“Web”: Characteristics must be 200 characters or fewer."
    );
    assert_eq!(
        refused(
            d(),
            set("/diagram/elements/0", "protocols", json!(["HTTPS"]))
        ),
        "“Web”: “protocols” isn’t a field a Layer has."
    );
}

#[test]
fn elements_together() {
    let c = conceptual;
    assert_eq!(
        refused(c(), set("/diagram/elements/1", "id", json!("a1"))),
        "The id a1 is used twice."
    );
    assert_eq!(
        refused(c(), set("/diagram/elements/2", "parentId", json!("zz"))),
        "“CRM”: The parent zz isn’t in the diagram."
    );
    assert_eq!(
        refused(c(), set("/diagram/elements/1", "parentId", json!("p1"))),
        "“Sales”: It can’t be inside itself."
    );
    assert_eq!(
        refused(c(), set("/diagram/elements/5", "parentId", json!("d1"))),
        "“Order”: A Domain can’t hold an Entity."
    );
}

#[test]
fn arrows() {
    let c = conceptual;
    let k1 = "/diagram/connections/0";
    assert_eq!(
        refused(c(), set(k1, "id", json!("a1"))),
        "The id a1 is used twice."
    );
    assert_eq!(
        refused(c(), set(k1, "targetId", json!("zz"))),
        "The arrow k1: There’s no element with id zz."
    );
    assert_eq!(
        refused(c(), set(k1, "targetId", json!("a1"))),
        "The arrow from “Bot” to “Bot”: An arrow can’t point to the element it starts from."
    );
    assert_eq!(
        refused(c(), |f| {
            set(k1, "sourceId", json!("e1"))(f);
            set(k1, "targetId", json!("a1"))(f);
        }),
        "The arrow from “Order” to “Bot”: An Entity can’t have an arrow to an Actor."
    );
    assert_eq!(
        refused(c(), |f| {
            set("/diagram/connections/1", "sourceId", json!("a1"))(f);
            set("/diagram/connections/1", "targetId", json!("p1"))(f);
        }),
        "The arrow from “Bot” to “CRM”: There’s already an arrow from “Bot” to “CRM”."
    );
    assert_eq!(
        refused(c(), set(k1, "color", json!("red"))),
        "The arrow from “Bot” to “CRM”: “color” isn’t a field an arrow has."
    );
    assert_eq!(
        refused(c(), set(k1, "mode", json!("automated"))),
        "The arrow from “Bot” to “CRM”: “mode” must be left out when it’s empty or the default."
    );
    assert_eq!(
        refused(c(), set(k1, "bidirectional", json!(false))),
        "The arrow from “Bot” to “CRM”: “bidirectional” must be left out when it’s empty or the \
         default."
    );
    assert_eq!(
        refused(c(), set(k1, "label", json!("x".repeat(101)))),
        "The arrow from “Bot” to “CRM”: Text must be 100 characters or fewer."
    );
    assert_eq!(
        refused(c(), set(k1, "manualOffset", json!("12"))),
        "The arrow from “Bot” to “CRM”: “manualOffset” must be a number."
    );

    let q1 = "/diagram/connections/0";
    assert_eq!(
        refused(physical(), remove(q1, "protocol")),
        "The arrow from “Web” to “Main”: Enter a protocol."
    );
    assert_eq!(
        refused(physical(), set(q1, "port", json!(" "))),
        "The arrow from “Web” to “Main”: Enter a port."
    );
    assert_eq!(
        refused(physical(), set(q1, "protocol", json!("x".repeat(21)))),
        "The arrow from “Web” to “Main”: Protocol must be 20 characters or fewer."
    );
}

#[test]
fn seeds_are_valid_files() {
    let seeds: Value =
        serde_json::from_str(include_str!("../../hiero-store/src/seed.json")).unwrap();
    for seed in seeds.as_array().unwrap() {
        diagram_from_file(&file(seed.clone())).unwrap_or_else(|e| panic!("{e}"));
    }
}

/// Diagrams built by edits, with values given in forms that aren't canonical, export as
/// files that import.
#[test]
fn edited_diagrams_export_valid_files() {
    let fresh = |diagram_type| {
        let mut diagram = new_diagram(diagram_type, "Edited", None);
        diagram.meta.created_at = "2026-10-01T09:00:00.000Z".into();
        diagram.meta.updated_at = diagram.meta.created_at.clone();
        diagram
    };
    let build = |mut diagram: hiero_core::stored::StoredDiagram, edits: Value| {
        let diagram_type = diagram.meta.diagram_type;
        let edits: Vec<EditOp> = serde_json::from_value(edits).unwrap();
        diagram.content = ops(diagram_type)
            .apply(&diagram, &edits)
            .unwrap_or_else(|e| panic!("{e:?}"))
            .content;
        diagram
    };
    let conceptual = build(
        fresh(DiagramTypeId::Conceptual),
        json!([
            {"op": "add_element", "ref": "a", "kind": "actor", "label": "User",
             "props": {"agent": false}},
            {"op": "add_element", "ref": "p", "kind": "application", "label": "CRM",
             "description": "  ", "props": {"cloudHosted": false}},
            {"op": "add_connection", "source_id": "a", "target_id": "p",
             "props": {"mode": "automated", "label": " ", "connector": "", "bidirectional": false}}
        ]),
    );
    let physical = {
        let diagram = fresh(DiagramTypeId::Physical);
        let zone = diagram.content.elements[2].id.0.clone();
        build(
            diagram,
            json!([
                {"op": "add_element", "ref": "s", "kind": "server", "label": "Web",
                 "parent_id": zone, "props": {"instanceCount": 1}},
                {"op": "add_element", "ref": "d", "kind": "database", "label": "DB",
                 "parent_id": zone},
                {"op": "add_connection", "source_id": "s", "target_id": "d"}
            ]),
        )
    };
    let deployed = build(
        fresh(DiagramTypeId::DeployedComponent),
        json!([
            {"op": "add_element", "ref": "l", "kind": "layer", "label": "Web",
             "props": {"characteristics": [" ", "PCI DSS "]}},
            {"op": "add_element", "kind": "component", "label": "API", "parent_id": "l",
             "description": "", "props": {"instanceCount": 1, "protocols": ["", "HTTPS"],
             "techStack": ["java", "docker", "java"]}}
        ]),
    );
    for diagram in [conceptual, physical, deployed] {
        read_diagram_file(&diagram_to_zip(&diagram)).unwrap_or_else(|e| {
            panic!("{e}\n{}", diagram_file_json(&diagram));
        });
    }
}
