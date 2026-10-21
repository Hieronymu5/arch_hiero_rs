//! The generic egui diagram editor: painting scenes, the zoomable canvas, the palette,
//! selection, moving, resizing, connecting, and the properties panel frame. Each diagram type
//! plugs in through [`DiagramView`].

pub mod editor;
pub mod fonts;
pub mod paint;
pub mod view;
pub mod widgets;

pub use editor::{ArrowLine, Change, DiagramEditor, DiagramView, Editor, PropsCx};
