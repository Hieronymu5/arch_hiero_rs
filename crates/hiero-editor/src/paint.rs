//! Paints a [`Scene`] with egui, at any zoom. Paths are tessellated with lyon once, in world
//! coordinates, and cached; each frame only moves and scales their vertices.

use std::collections::HashMap;
use std::sync::Arc;

use egui::epaint::{CircleShape, EllipseShape, Mesh, PathStroke, RectShape};
use egui::{Color32, CornerRadius, Pos2, Shape, Stroke, StrokeKind, TextureHandle, Vec2};
use hiero_core::geometry::{Point, Rect};
use hiero_core::scene::{Anchor, Node, Paint, Scene, Seg, parse_path};
use hiero_core::text::font_metrics;
use lyon_path::builder::SvgPathBuilder;
use lyon_path::geom::{Angle, euclid::Vector2D};
use lyon_path::iterator::PathIterator;
use lyon_path::math::point;
use lyon_path::{ArcFlags, Path};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator,
    StrokeVertex, VertexBuffers,
};

use crate::fonts::font_id;

/// Maps world (canvas) coordinates to the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Where the world's origin is on screen.
    pub origin: Pos2,
    /// Screen points per world pixel.
    pub zoom: f32,
}

impl Transform {
    pub fn pos(&self, p: Point) -> Pos2 {
        Pos2::new(
            self.origin.x + p.x as f32 * self.zoom,
            self.origin.y + p.y as f32 * self.zoom,
        )
    }

    pub fn len(&self, v: f64) -> f32 {
        v as f32 * self.zoom
    }

    pub fn rect(&self, r: Rect) -> egui::Rect {
        egui::Rect::from_min_size(
            self.pos(r.position()),
            Vec2::new(self.len(r.width), self.len(r.height)),
        )
    }

    /// The world point under a screen position.
    pub fn world(&self, p: Pos2) -> Point {
        Point::new(
            f64::from((p.x - self.origin.x) / self.zoom),
            f64::from((p.y - self.origin.y) / self.zoom),
        )
    }
}

/// A `#rrggbb` colour, or transparent for "none" and anything unreadable.
pub fn color(hex: &str) -> Color32 {
    if !hiero_core::color::is_hex_color(hex) {
        return Color32::TRANSPARENT;
    }
    let [r, g, b] = hiero_core::color::channels(hex);
    Color32::from_rgb(r, g, b)
}

#[derive(Default)]
struct TessellatedPath {
    fill: Option<VertexBuffers<[f32; 2], u32>>,
    strokes: HashMap<u32, VertexBuffers<[f32; 2], u32>>,
    /// Each subpath flattened, for dashed strokes.
    polylines: Vec<Vec<[f32; 2]>>,
    path: Option<Path>,
}

/// Tessellated paths and loaded images, kept between frames.
#[derive(Default)]
pub struct PaintCache {
    paths: HashMap<String, TessellatedPath>,
    textures: HashMap<usize, TextureHandle>,
}

impl PaintCache {
    fn path(&mut self, d: &str) -> &mut TessellatedPath {
        if !self.paths.contains_key(d) {
            if self.paths.len() > 4000 {
                self.paths.clear();
            }
            let path = build_path(&parse_path(d));
            let polylines = flatten(&path);
            self.paths.insert(
                d.to_owned(),
                TessellatedPath {
                    path: Some(path),
                    polylines,
                    ..TessellatedPath::default()
                },
            );
        }
        self.paths.get_mut(d).expect("just inserted")
    }

    fn texture(&mut self, ctx: &egui::Context, png: &'static [u8]) -> Option<TextureHandle> {
        let key = png.as_ptr() as usize;
        if let Some(t) = self.textures.get(&key) {
            return Some(t.clone());
        }
        let image = image::load_from_memory(png).ok()?.to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        let handle = ctx.load_texture(
            format!("png-{key}"),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        self.textures.insert(key, handle.clone());
        Some(handle)
    }
}

fn build_path(segs: &[Seg]) -> Path {
    let mut builder = Path::builder().with_svg();
    for seg in segs {
        let p = |q: Point| point(q.x as f32, q.y as f32);
        match *seg {
            Seg::Move(to) => {
                builder.move_to(p(to));
            }
            Seg::Line(to) => {
                builder.line_to(p(to));
            }
            Seg::Quad(c, to) => {
                builder.quadratic_bezier_to(p(c), p(to));
            }
            Seg::Cubic(a, b, to) => {
                builder.cubic_bezier_to(p(a), p(b), p(to));
            }
            Seg::Arc {
                rx,
                ry,
                rotation,
                large,
                sweep,
                to,
            } => {
                builder.arc_to(
                    Vector2D::new(rx as f32, ry as f32),
                    Angle::degrees(rotation as f32),
                    ArcFlags {
                        large_arc: large,
                        sweep,
                    },
                    p(to),
                );
            }
            Seg::Close => builder.close(),
        }
    }
    builder.build()
}

fn flatten(path: &Path) -> Vec<Vec<[f32; 2]>> {
    use lyon_path::PathEvent;
    let mut lines: Vec<Vec<[f32; 2]>> = Vec::new();
    for event in path.iter().flattened(0.05) {
        match event {
            PathEvent::Begin { at } => lines.push(vec![at.to_array()]),
            PathEvent::Line { to, .. } => {
                if let Some(l) = lines.last_mut() {
                    l.push(to.to_array());
                }
            }
            PathEvent::End { first, close, .. } => {
                if close && let Some(l) = lines.last_mut() {
                    l.push(first.to_array());
                }
            }
            _ => {}
        }
    }
    lines
}

fn fill_buffers(path: &Path) -> VertexBuffers<[f32; 2], u32> {
    let mut geometry: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let _ = FillTessellator::new().tessellate_path(
        path,
        &FillOptions::default().with_tolerance(0.05),
        &mut BuffersBuilder::new(&mut geometry, |v: FillVertex| v.position().to_array()),
    );
    geometry
}

fn stroke_buffers(path: &Path, width: f32) -> VertexBuffers<[f32; 2], u32> {
    let mut geometry: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let _ = StrokeTessellator::new().tessellate_path(
        path,
        &StrokeOptions::default()
            .with_line_width(width)
            .with_tolerance(0.05),
        &mut BuffersBuilder::new(&mut geometry, |v: StrokeVertex| v.position().to_array()),
    );
    geometry
}

fn mesh(
    buffers: &VertexBuffers<[f32; 2], u32>,
    xf: &Transform,
    offset: Point,
    fill: Color32,
) -> Mesh {
    let mut mesh = Mesh::default();
    mesh.vertices.reserve(buffers.vertices.len());
    for v in &buffers.vertices {
        let p = xf.pos(Point::new(
            f64::from(v[0]) + offset.x,
            f64::from(v[1]) + offset.y,
        ));
        mesh.colored_vertex(p, fill);
    }
    mesh.indices.clone_from(&buffers.indices);
    mesh
}

struct Ctx<'a> {
    painter: &'a egui::Painter,
    xf: Transform,
    cache: &'a mut PaintCache,
    scene: &'a Scene,
    /// Fades everything, such as an element being dragged.
    opacity: f32,
}

impl Ctx<'_> {
    fn tone(&self, c: Color32) -> Color32 {
        if self.opacity >= 1.0 {
            c
        } else {
            c.gamma_multiply(self.opacity)
        }
    }

    fn fill_of(&self, paint: &Paint) -> Color32 {
        self.tone(paint.fill.as_deref().map_or(Color32::BLACK, color))
    }

    fn stroke_of(&self, paint: &Paint) -> Option<Stroke> {
        let c = paint.stroke.as_deref().map(color)?;
        if c == Color32::TRANSPARENT {
            return None;
        }
        let width = paint.stroke_width.unwrap_or(1.0);
        (width > 0.0).then(|| Stroke::new(self.xf.len(width), self.tone(c)))
    }
}

/// Paints `scene` with `xf`, fading it to `opacity`.
pub fn paint_scene(
    painter: &egui::Painter,
    scene: &Scene,
    xf: Transform,
    cache: &mut PaintCache,
    opacity: f32,
) {
    let root = Paint {
        fill: Some(scene.text_color.clone()),
        stroke: None,
        stroke_width: None,
        dash: None,
    };
    let mut ctx = Ctx {
        painter,
        xf,
        cache,
        scene,
        opacity,
    };
    for node in &scene.nodes {
        paint_node(&mut ctx, node, &root, Point::default());
    }
}

/// Paints nodes, such as an editor's overlay, in `scene`'s colours.
pub fn paint_nodes(
    painter: &egui::Painter,
    scene: &Scene,
    nodes: &[Node],
    xf: Transform,
    cache: &mut PaintCache,
) {
    let root = Paint::fill(scene.text_color.clone());
    let mut ctx = Ctx {
        painter,
        xf,
        cache,
        scene,
        opacity: 1.0,
    };
    for node in nodes {
        paint_node(&mut ctx, node, &root, Point::default());
    }
}

fn offset_point(p: Point, offset: Point) -> Point {
    Point::new(p.x + offset.x, p.y + offset.y)
}

fn paint_node(ctx: &mut Ctx<'_>, node: &Node, parent: &Paint, offset: Point) {
    match node {
        Node::Rect { rect, rx, paint } => {
            let paint = paint.over(parent);
            let r = ctx.xf.rect(Rect {
                x: rect.x + offset.x,
                y: rect.y + offset.y,
                ..*rect
            });
            let radius = CornerRadius::same(ctx.xf.len(*rx).round().clamp(0.0, 255.0) as u8);
            let fill = ctx.fill_of(&paint);
            let stroke = ctx.stroke_of(&paint);
            match (paint.dash, stroke) {
                (Some((dash, gap)), Some(stroke)) => {
                    ctx.painter.add(RectShape::filled(r, radius, fill));
                    let corners = [
                        r.left_top(),
                        r.right_top(),
                        r.right_bottom(),
                        r.left_bottom(),
                        r.left_top(),
                    ];
                    ctx.painter.extend(Shape::dashed_line(
                        &corners,
                        stroke,
                        ctx.xf.len(dash),
                        ctx.xf.len(gap),
                    ));
                }
                _ => {
                    ctx.painter.add(RectShape::new(
                        r,
                        radius,
                        fill,
                        stroke.unwrap_or(Stroke::NONE),
                        StrokeKind::Middle,
                    ));
                }
            }
        }
        Node::Circle { center, r, paint } => {
            let paint = paint.over(parent);
            ctx.painter.add(CircleShape {
                center: ctx.xf.pos(offset_point(*center, offset)),
                radius: ctx.xf.len(*r),
                fill: ctx.fill_of(&paint),
                stroke: ctx.stroke_of(&paint).unwrap_or(Stroke::NONE),
            });
        }
        Node::Ellipse {
            center,
            rx,
            ry,
            paint,
        } => {
            let paint = paint.over(parent);
            let mut shape = EllipseShape::filled(
                ctx.xf.pos(offset_point(*center, offset)),
                Vec2::new(ctx.xf.len(*rx), ctx.xf.len(*ry)),
                ctx.fill_of(&paint),
            );
            shape.stroke = ctx.stroke_of(&paint).unwrap_or(Stroke::NONE);
            ctx.painter.add(shape);
        }
        Node::Line {
            from,
            to,
            paint,
            marker,
        } => {
            let paint = paint.over(parent);
            let Some(stroke) = ctx.stroke_of(&paint) else {
                return;
            };
            let a = ctx.xf.pos(offset_point(*from, offset));
            let mut b = ctx.xf.pos(offset_point(*to, offset));
            if let Some(marker) = marker {
                let size = ctx.xf.len(marker.size());
                let dir = (b - a).normalized();
                // The line stops inside the head, so its end doesn't poke through the tip.
                let tip = b;
                b = tip - dir * size * 0.5;
                let base = tip - dir * size;
                let side = Vec2::new(-dir.y, dir.x) * size / 2.0;
                let head = ctx.tone(color(&ctx.scene.marker_color));
                ctx.painter.add(Shape::convex_polygon(
                    vec![tip, base + side, base - side],
                    head,
                    Stroke::NONE,
                ));
            }
            match paint.dash {
                Some((dash, gap)) => {
                    ctx.painter.extend(Shape::dashed_line(
                        &[a, b],
                        stroke,
                        ctx.xf.len(dash),
                        ctx.xf.len(gap),
                    ));
                }
                None => {
                    ctx.painter.line_segment([a, b], stroke);
                }
            }
        }
        Node::Path { d, paint } => {
            let paint = paint.over(parent);
            let fill = ctx.fill_of(&paint);
            let stroke = ctx.stroke_of(&paint);
            let xf = ctx.xf;
            let entry = ctx.cache.path(d);
            if fill != Color32::TRANSPARENT {
                if entry.fill.is_none() {
                    entry.fill = entry.path.as_ref().map(fill_buffers);
                }
                if let Some(buffers) = &entry.fill {
                    ctx.painter
                        .add(Shape::mesh(mesh(buffers, &xf, offset, fill)));
                }
            }
            if let Some(stroke) = stroke {
                let world_width = paint.stroke_width.unwrap_or(1.0) as f32;
                if let Some((dash, gap)) = paint.dash {
                    let lines: Vec<Vec<Pos2>> = entry
                        .polylines
                        .iter()
                        .map(|l| {
                            l.iter()
                                .map(|v| {
                                    xf.pos(Point::new(
                                        f64::from(v[0]) + offset.x,
                                        f64::from(v[1]) + offset.y,
                                    ))
                                })
                                .collect()
                        })
                        .collect();
                    for line in lines {
                        ctx.painter.extend(Shape::dashed_line(
                            &line,
                            stroke,
                            xf.len(dash),
                            xf.len(gap),
                        ));
                    }
                } else {
                    let key = world_width.to_bits();
                    if !entry.strokes.contains_key(&key)
                        && let Some(path) = &entry.path
                    {
                        entry.strokes.insert(key, stroke_buffers(path, world_width));
                    }
                    if let Some(buffers) = entry.strokes.get(&key) {
                        ctx.painter
                            .add(Shape::mesh(mesh(buffers, &xf, offset, stroke.color)));
                    }
                }
            }
        }
        Node::Text {
            at,
            anchor,
            size,
            weight,
            text,
            fill,
            halo,
        } => {
            let color_hex = fill
                .clone()
                .or_else(|| parent.fill.clone())
                .unwrap_or_default();
            let text_color = ctx.tone(color(&color_hex));
            let font_size = ctx.xf.len(*size);
            if font_size < 1.0 {
                return;
            }
            let font = font_id(font_size, *weight);
            let galley = ctx.painter.layout_no_wrap(text.clone(), font, text_color);
            // Line the galley's baseline up with the scene's.
            let baseline_in_galley = galley
                .rows
                .first()
                .and_then(|row| row.glyphs.first().map(|g| row.pos.y + g.pos.y))
                .unwrap_or_else(|| font_size * font_metrics().ascent(*weight) as f32);
            let p = ctx.xf.pos(offset_point(*at, offset));
            let width = galley.size().x;
            let left = match anchor {
                Anchor::Start => p.x,
                Anchor::Middle => p.x - width / 2.0,
                Anchor::End => p.x - width,
            };
            let top_left = Pos2::new(left, p.y - baseline_in_galley);
            if let Some((halo_hex, halo_width)) = halo {
                let halo_color = ctx.tone(color(halo_hex));
                let w = ctx.xf.len(*halo_width) / 2.0;
                let outline = ctx.painter.layout_no_wrap(
                    text.clone(),
                    font_id(font_size, *weight),
                    halo_color,
                );
                for (dx, dy) in [
                    (-w, 0.0),
                    (w, 0.0),
                    (0.0, -w),
                    (0.0, w),
                    (-w, -w),
                    (w, w),
                    (-w, w),
                    (w, -w),
                ] {
                    ctx.painter
                        .galley(top_left + Vec2::new(dx, dy), outline.clone(), halo_color);
                }
            }
            ctx.painter.galley(top_left, galley, text_color);
        }
        Node::Image { rect, png } => {
            if let Some(texture) = ctx.cache.texture(ctx.painter.ctx(), png) {
                let r = ctx.xf.rect(Rect {
                    x: rect.x + offset.x,
                    y: rect.y + offset.y,
                    ..*rect
                });
                ctx.painter.image(
                    texture.id(),
                    r,
                    egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    ctx.tone(Color32::WHITE),
                );
            }
        }
        Node::Group {
            translate,
            paint,
            children,
        } => {
            let paint = paint.over(parent);
            let offset = translate.map_or(offset, |t| offset_point(t, offset));
            for child in children {
                paint_node(ctx, child, &paint, offset);
            }
        }
    }
}

/// A stroke along a polyline in world coordinates, such as a pending arrow.
pub fn world_line(
    painter: &egui::Painter,
    xf: Transform,
    points: &[Point],
    stroke: Stroke,
    dashed: bool,
) {
    let pts: Vec<Pos2> = points.iter().map(|p| xf.pos(*p)).collect();
    if dashed {
        painter.extend(Shape::dashed_line(&pts, stroke, 6.0, 4.0));
    } else {
        painter.add(Shape::line(
            pts,
            PathStroke::new(stroke.width, stroke.color),
        ));
    }
}

/// Keeps the cache's textures for logos alive, for callers that want them outside a scene.
pub fn texture_for(
    cache: &mut PaintCache,
    ctx: &egui::Context,
    png: &'static [u8],
) -> Option<TextureHandle> {
    cache.texture(ctx, png)
}

/// The shared cache used by an editor.
pub type SharedCache = Arc<std::sync::Mutex<PaintCache>>;
