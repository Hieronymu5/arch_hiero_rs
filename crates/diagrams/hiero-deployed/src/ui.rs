//! The deployed component diagram's properties panel and its place in the shared editor.

use egui::{RichText, Ui};
use hiero_core::engine::{Engine, Laid, validate_length};
use hiero_core::model::ConnectionId;
use hiero_core::scene::Scene;
use hiero_editor::widgets::{select, text_field};
use hiero_editor::{DiagramView, PropsCx};

use crate::layout::is_layer;
use crate::model::{
    CHARACTERISTICS_MAX, DESCRIPTION_MAX, Dependency, DeployedComponent, MAX_DESCRIPTION_ROWS,
    MAX_INSTANCE_COUNT, MAX_PROTOCOLS, PROTOCOLS_MAX, list_from,
};
use crate::rules::toggle_tech;
use crate::scene::scene_of;
use crate::tech_stack::{TECH_STACK, TechCategory};

fn parse_count(text: &str) -> Result<Option<u32>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match text.parse::<u32>() {
        Ok(n) if (1..=MAX_INSTANCE_COUNT).contains(&n) => Ok(Some(n)),
        _ => Err(format!(
            "Enter a whole number from 1 to {MAX_INSTANCE_COUNT}."
        )),
    }
}

impl DiagramView for DeployedComponent {
    const HELP: &'static str = crate::HELP;

    fn scene(
        eng: &Engine<Self>,
        layout: &[Laid<Self>],
        _slide: Option<(&ConnectionId, f64)>,
    ) -> Scene {
        scene_of(eng, layout)
    }

    fn element_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, element: &Laid<Self>) {
        let mut with = element.el.clone();
        let mut field = None;
        let id = element.id().clone();
        if cx.name_field(ui, element, &mut with) {
            field = Some("label");
        }
        if is_layer(&element.el) {
            let current = element
                .el
                .props
                .characteristics
                .clone()
                .unwrap_or_default()
                .join(", ");
            if let Some(text) = text_field(
                ui,
                cx.field_id(&id, "characteristics"),
                "Characteristics, separated by commas",
                &current,
                false,
                |t| validate_length("Characteristics", t, CHARACTERISTICS_MAX),
            ) {
                with.props.characteristics = list_from(&text);
                field = Some("characteristics");
            }
            if let Some(field) = field {
                cx.commit(element, with, None, field);
            }
            return;
        }
        let parent = cx.parent_field(ui, element, element.el.parent_id.is_some());
        if parent.is_some() {
            field = Some("parent");
        }
        let options: Vec<(Dependency, String)> = Dependency::ALL
            .iter()
            .map(|d| (*d, d.colors().label.to_owned()))
            .collect();
        if let Some(v) = select(
            ui,
            cx.field_id(&id, "dependency"),
            "Type",
            &element.el.props.dependency,
            Some(hiero_core::palette::CORE_COMPONENT.label),
            &options,
        ) {
            with.props.dependency = v;
            field = Some("dependency");
        }
        let protocols = element
            .el
            .props
            .protocols
            .clone()
            .unwrap_or_default()
            .join(", ");
        if let Some(text) = text_field(
            ui,
            cx.field_id(&id, "protocols"),
            "Protocols and security, separated by commas",
            &protocols,
            false,
            |t| {
                validate_length("Protocols", t, PROTOCOLS_MAX)?;
                if t.split(',').count() > MAX_PROTOCOLS {
                    return Err(format!("Enter up to {MAX_PROTOCOLS}, separated by commas."));
                }
                Ok(())
            },
        ) {
            with.props.protocols = list_from(&text);
            field = Some("protocols");
        }
        if let Some(text) = text_field(
            ui,
            cx.field_id(&id, "description"),
            "Description, a bullet on each line",
            element.el.description.as_deref().unwrap_or_default(),
            true,
            |t| {
                validate_length("Description", t, DESCRIPTION_MAX)?;
                if t.split('\n').count() > MAX_DESCRIPTION_ROWS {
                    return Err(format!("Use {MAX_DESCRIPTION_ROWS} lines or fewer."));
                }
                Ok(())
            },
        ) {
            with.description = Some(text);
            field = Some("description");
        }
        let count = element
            .el
            .props
            .instance_count
            .map(|n| n.to_string())
            .unwrap_or_default();
        if let Some(text) = text_field(
            ui,
            cx.field_id(&id, "instances"),
            "Instance count",
            &count,
            false,
            |t| parse_count(t).map(|_| ()),
        ) && let Ok(n) = parse_count(&text)
        {
            with.props.instance_count = n;
            field = Some("instances");
        }
        if let Some(field) = field {
            cx.commit(element, with, parent, field);
        }

        ui.add_space(6.0);
        ui.label(RichText::new("Tech stack").strong());
        let chosen: Vec<String> = element.el.props.tech_stack.clone().unwrap_or_default();
        for category in TechCategory::ALL {
            let items: Vec<_> = TECH_STACK
                .iter()
                .filter(|i| i.category == category)
                .collect();
            let count = items
                .iter()
                .filter(|i| chosen.iter().any(|c| c == i.id))
                .count();
            let title = if count > 0 {
                format!("{} ({count})", category.label())
            } else {
                category.label().to_owned()
            };
            egui::CollapsingHeader::new(title)
                .id_salt(cx.field_id(&id, category.label()))
                .default_open(false)
                .show(ui, |ui| {
                    for item in items {
                        let mut has = chosen.iter().any(|c| c == item.id);
                        ui.horizontal(|ui| {
                            let image = egui::Image::from_bytes(
                                format!("bytes://tech/{}.png", item.id),
                                item.png,
                            )
                            .fit_to_exact_size(egui::vec2(18.0, 18.0));
                            ui.add(image);
                            if ui.checkbox(&mut has, item.label).changed() {
                                let (target, tech) = (id.clone(), item.id);
                                cx.apply(None, move |eng| toggle_tech(eng, &target, tech));
                            }
                        });
                    }
                });
        }
    }
}
