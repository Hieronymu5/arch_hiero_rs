//! The one place the app names every diagram type's editor.

use hiero_conceptual::Conceptual;
use hiero_core::model::DiagramTypeId;
use hiero_core::stored::{ContentError, StoredDiagram};
use hiero_deployed::DeployedComponent;
use hiero_editor::{DiagramEditor, Editor};
use hiero_physical::Physical;

/// An editor for `diagram`, of its type.
pub fn editor_for(diagram: &StoredDiagram) -> Result<Box<dyn DiagramEditor>, ContentError> {
    let salt = egui::Id::unique(("editor", diagram.meta.id.as_str()));
    let (content, colors) = (&diagram.content, &diagram.colors);
    Ok(match diagram.meta.diagram_type {
        DiagramTypeId::Conceptual => {
            Box::new(Editor::<Conceptual>::from_stored(content, colors, salt)?)
        }
        DiagramTypeId::Physical => {
            Box::new(Editor::<Physical>::from_stored(content, colors, salt)?)
        }
        DiagramTypeId::DeployedComponent => Box::new(Editor::<DeployedComponent>::from_stored(
            content, colors, salt,
        )?),
    })
}
