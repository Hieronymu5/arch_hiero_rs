//! The physical diagram's properties panel and its place in the shared editor.

use egui::Ui;
use hiero_core::count::InstanceCount;
use hiero_core::engine::{Conn, DiagramType, Engine, Laid};
use hiero_core::model::ConnectionId;
use hiero_core::scene::Scene;
use hiero_editor::widgets::{select, text_field};
use hiero_editor::{ArrowLine, DiagramView, PropsCx};

use crate::model::{
    DatabaseType, GENERIC_DATABASE, MAX_INSTANCE_COUNT, Physical, PhysicalArrow, ServerType,
    ZoneColor,
};
use crate::rules::{is_stacked, stack_holder_of};
use crate::scene::{drawn_arrows, scene_of};

/// Reads an instance count: empty is one; otherwise a whole number from 1 to 9999.
/// A count as typed, such as "3", "50+", or "50-100". Empty means one.
fn parse_count(text: &str) -> Result<Option<InstanceCount>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    InstanceCount::parse(text, MAX_INSTANCE_COUNT).map(Some)
}

impl DiagramView for Physical {
    const HELP: &'static str = crate::HELP;

    fn scene(
        eng: &Engine<Self>,
        layout: &[Laid<Self>],
        _slide: Option<(&ConnectionId, f64)>,
    ) -> Scene {
        scene_of(eng, layout)
    }

    fn arrow_lines(
        eng: &Engine<Self>,
        layout: &[Laid<Self>],
        _slide: Option<(&ConnectionId, f64)>,
    ) -> Vec<ArrowLine> {
        drawn_arrows(eng, layout)
            .into_iter()
            .map(|a| ArrowLine {
                id: a.id,
                from: a.look.from,
                to: a.look.to,
                boxes: a.look.tag.iter().map(|t| t.rect).collect(),
            })
            .collect()
    }

    fn element_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, element: &Laid<Self>) {
        let mut with = element.el.clone();
        let kind = element.el.kind;
        let mut field = None;
        if cx.name_field(ui, element, &mut with) {
            field = Some("label");
        }
        if kind.has_server_type() {
            let options: Vec<(ServerType, String)> = ServerType::ALL
                .iter()
                .map(|t| (*t, t.label().to_owned()))
                .collect();
            if let Some(v) = select(
                ui,
                cx.field_id(element.id(), "server-type"),
                "Server type",
                &element.el.props.server_type,
                Some("Physical"),
                &options,
            ) {
                with.props.server_type = v;
                field = Some("server-type");
            }
        }
        if kind.has_database_type() {
            let options: Vec<(DatabaseType, String)> = DatabaseType::ALL
                .iter()
                .map(|t| {
                    (
                        *t,
                        format!("{} ({})", t.info().label, t.info().abbreviation),
                    )
                })
                .collect();
            let generic = format!(
                "{} ({})",
                GENERIC_DATABASE.label, GENERIC_DATABASE.abbreviation
            );
            if let Some(v) = select(
                ui,
                cx.field_id(element.id(), "database-type"),
                "Database type",
                &element.el.props.database_type,
                Some(&generic),
                &options,
            ) {
                with.props.database_type = v;
                field = Some("database-type");
            }
        }
        if let Some((label, _)) = kind.instances() {
            let current = element
                .el
                .props
                .instance_count
                .map(|c| c.to_string())
                .unwrap_or_default();
            if let Some(text) = text_field(
                ui,
                cx.field_id(element.id(), "instances"),
                label,
                &current,
                false,
                |t| parse_count(t).map(|_| ()),
            ) && let Ok(count) = parse_count(&text)
            {
                with.props.instance_count = count;
                field = Some("instances");
            }
        }
        if kind.has_zone_color() && !is_stacked(cx.eng, &element.el) {
            let options: Vec<(ZoneColor, String)> = ZoneColor::ALL
                .iter()
                .map(|c| (*c, c.info().label.to_owned()))
                .collect();
            if let Some(v) = select(
                ui,
                cx.field_id(element.id(), "zone-color"),
                "Color",
                &element.el.props.zone_color,
                Some("Blue"),
                &options,
            ) {
                with.props.zone_color = v;
                field = Some("zone-color");
            }
        }
        // A zone in a network stays in one.
        let required = stack_holder_of(cx.eng, element).is_some();
        let parent = cx.parent_field(ui, element, required);
        if parent.is_some() {
            field = Some("parent");
        }
        if let Some(field) = field {
            cx.commit(element, with, parent, field);
        }
    }

    fn arrow_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, connection: &Conn<Self>) {
        let id = connection.id.clone();
        let current = connection.props.clone();
        let mut arrow = current.clone();
        let mut field = None;
        let required = |name: &'static str, max: usize| {
            move |t: &str| {
                if t.trim().is_empty() {
                    Err(format!("Enter a {}.", name.to_lowercase()))
                } else {
                    hiero_core::engine::validate_length(name, t, max)
                }
            }
        };
        if let Some(v) = text_field(
            ui,
            cx.arrow_field_id(&id, "protocol"),
            "Protocol",
            &current.protocol,
            false,
            required("Protocol", crate::model::PROTOCOL_MAX),
        ) {
            arrow.protocol = v.trim().to_owned();
            field = Some("protocol");
        }
        if let Some(v) = text_field(
            ui,
            cx.arrow_field_id(&id, "port"),
            "Port",
            &current.port,
            false,
            required("Port", crate::model::PORT_MAX),
        ) {
            arrow.port = v.trim().to_owned();
            field = Some("port");
        }
        if let Some(field) = field
            && Physical::validate_arrow(&arrow).is_ok()
            && arrow != current
        {
            cx.apply(Some(format!("{id}:{field}")), move |eng| {
                eng.set_connection_props(&id, arrow);
            });
        }
    }

    fn arrow_title(
        _eng: &Engine<Self>,
        connection: &Conn<Self>,
        outgoing: bool,
        other: &str,
    ) -> String {
        let tag = connection.props.tag();
        let over = if tag.is_empty() {
            String::new()
        } else {
            format!(" over {tag}")
        };
        format!("{} {other}{over}", if outgoing { "To" } else { "From" })
    }
}

/// The arrow type, for callers that build arrows by hand.
pub type Arrow = PhysicalArrow;
