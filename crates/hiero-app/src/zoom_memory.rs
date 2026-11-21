//! Each diagram's last zoom, kept between runs, so a diagram opens, and comes back from another
//! page, as large as it was last drawn.

use egui::{Context, Id};
use hiero_core::model::DiagramId;

/// How many diagrams' zooms are kept, the most recently zoomed first.
const KEPT: usize = 200;

fn key() -> Id {
    Id::unique("hiero-app.zooms")
}

/// The zooms kept, as percentages, the most recently zoomed first.
fn zooms(ctx: &Context) -> Vec<(String, u32)> {
    ctx.data_mut(|d| d.get_persisted(key())).unwrap_or_default()
}

/// The zoom `id` was last drawn at, if it's been zoomed.
pub fn remembered(ctx: &Context, id: &DiagramId) -> Option<u32> {
    zooms(ctx)
        .into_iter()
        .find(|(kept, _)| kept == id.as_str())
        .map(|(_, percent)| percent)
}

/// Keeps `percent` as the zoom `id` was last drawn at.
pub fn remember(ctx: &Context, id: &DiagramId, percent: u32) {
    let mut zooms = zooms(ctx);
    if zooms.first() == Some(&(id.to_string(), percent)) {
        return;
    }
    // The most recent first, so the oldest are the ones let go.
    zooms.retain(|(kept, _)| kept != id.as_str());
    zooms.insert(0, (id.to_string(), percent));
    zooms.truncate(KEPT);
    ctx.data_mut(|d| d.insert_persisted(key(), zooms));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_diagram_keeps_its_last_zoom_the_oldest_let_go() {
        let ctx = Context::default();
        let a = DiagramId("a".into());
        assert_eq!(remembered(&ctx, &a), None);
        remember(&ctx, &a, 150);
        remember(&ctx, &DiagramId("b".into()), 60);
        remember(&ctx, &a, 120);
        assert_eq!(remembered(&ctx, &a), Some(120));
        assert_eq!(remembered(&ctx, &DiagramId("b".into())), Some(60));
        for i in 0..KEPT {
            remember(&ctx, &DiagramId(format!("x{i}")), 100);
        }
        assert_eq!(remembered(&ctx, &a), None);
    }
}
