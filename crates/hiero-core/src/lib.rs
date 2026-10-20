//! UI-independent diagram model.
//!
//! Everything in this crate must stay free of UI/rendering dependencies so it
//! can be unit-tested headlessly and reused by any front end.

use serde::{Deserialize, Serialize};

/// Stable identifier for a node within a [`Diagram`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub u64);

/// A 2D point in diagram (world) coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A 2D size in diagram (world) units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub label: String,
    /// Top-left corner in world coordinates.
    pub position: Point,
    pub size: Size,
}

impl Node {
    #[must_use]
    pub fn center(&self) -> Point {
        Point::new(
            self.position.x + self.size.width / 2.0,
            self.position.y + self.size.height / 2.0,
        )
    }

    #[must_use]
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.position.x
            && p.x <= self.position.x + self.size.width
            && p.y >= self.position.y
            && p.y <= self.position.y + self.size.height
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Diagram {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    next_id: u64,
}

impl Diagram {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node and returns its id.
    pub fn add_node(&mut self, label: impl Into<String>, position: Point) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        self.nodes.push(Node {
            id,
            label: label.into(),
            position,
            size: Size {
                width: 140.0,
                height: 60.0,
            },
        });
        id
    }

    /// Connects two existing nodes. Returns `false` if either id is unknown.
    pub fn connect(&mut self, from: NodeId, to: NodeId) -> bool {
        if self.node(from).is_none() || self.node(to).is_none() {
            return false;
        }
        self.edges.push(Edge { from, to });
        true
    }

    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Returns the top-most node under `p` (last drawn wins).
    #[must_use]
    pub fn hit_test(&self, p: Point) -> Option<NodeId> {
        self.nodes
            .iter()
            .rev()
            .find(|n| n.contains(p))
            .map(|n| n.id)
    }

    /// Removes a node and every edge attached to it.
    pub fn remove_node(&mut self, id: NodeId) {
        self.nodes.retain(|n| n.id != id);
        self.edges.retain(|e| e.from != id && e.to != id);
    }

    /// A small sample diagram used on first launch.
    #[must_use]
    pub fn sample() -> Self {
        let mut d = Self::new();
        let client = d.add_node("Client", Point::new(40.0, 120.0));
        let gateway = d.add_node("API Gateway", Point::new(260.0, 120.0));
        let service = d.add_node("Service", Point::new(480.0, 40.0));
        let db = d.add_node("Database", Point::new(480.0, 200.0));
        d.connect(client, gateway);
        d.connect(gateway, service);
        d.connect(gateway, db);
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_connect() {
        let mut d = Diagram::new();
        let a = d.add_node("A", Point::new(0.0, 0.0));
        let b = d.add_node("B", Point::new(200.0, 0.0));
        assert_ne!(a, b);
        assert!(d.connect(a, b));
        assert!(!d.connect(a, NodeId(999)));
        assert_eq!(d.edges.len(), 1);
    }

    #[test]
    fn hit_test_picks_topmost() {
        let mut d = Diagram::new();
        let _a = d.add_node("A", Point::new(0.0, 0.0));
        let b = d.add_node("B", Point::new(10.0, 10.0));
        assert_eq!(d.hit_test(Point::new(20.0, 20.0)), Some(b));
        assert_eq!(d.hit_test(Point::new(-5.0, -5.0)), None);
    }

    #[test]
    fn remove_node_drops_edges() {
        let mut d = Diagram::sample();
        let gateway = d.nodes[1].id;
        d.remove_node(gateway);
        assert_eq!(d.edges, Vec::new());
        assert_eq!(d.nodes.len(), 3);
    }

    #[test]
    fn ids_unique_after_removal() {
        let mut d = Diagram::new();
        let a = d.add_node("A", Point::default());
        d.remove_node(a);
        let b = d.add_node("B", Point::default());
        assert_ne!(a, b);
    }
}
