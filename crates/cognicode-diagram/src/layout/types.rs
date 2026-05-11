//! Layout data types for the Sugiyama layout engine, port assignment,
//! compound nodes, and SVG rendering.

use serde::{Deserialize, Serialize};

/// A 2D point with floating-point precision
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Which side of a node a port is on
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortSide {
    North,
    South,
    East,
    West,
}

/// A connection port on a node's boundary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    /// Which side of the node
    pub side: PortSide,
    /// Offset along that side (0.0 = left/top, 0.5 = center, 1.0 = right/bottom)
    pub offset: f64,
    /// Absolute position of this port on the canvas
    pub position: Point,
    /// ID of the edge connected to this port (optional)
    pub connected_edge: Option<String>,
}

/// A node with its computed layout position and dimensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutedNode {
    /// Unique ID matching the C4 element's ElementId
    pub id: String,
    /// Label text displayed on the node
    pub label: String,
    /// Top-left corner position
    pub position: Point,
    /// Width and height of the node
    pub size: (f64, f64),
    /// Connection ports (ordered by side)
    pub ports: Vec<Port>,
    /// CSS class or style name for rendering
    pub style_class: String,
    /// For compound nodes: children node IDs
    pub children: Vec<String>,
    /// For compound nodes: parent node ID
    pub parent: Option<String>,
    /// Node kind for SVG shape selection: "person", "system", "container", "component", "datastore", "code"
    pub kind: String,
    /// Technology label (e.g. "Rust", "SQLite")
    pub technology: Option<String>,
    /// Description shown below label
    pub description: Option<String>,
    /// Rendering z-order (higher = drawn on top)
    pub z_index: i32,
}

impl LayoutedNode {
    /// Get the center point of the node
    pub fn center(&self) -> Point {
        Point::new(
            self.position.x + self.size.0 / 2.0,
            self.position.y + self.size.1 / 2.0,
        )
    }

    /// Get the bounding box as (x, y, width, height)
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        (self.position.x, self.position.y, self.size.0, self.size.1)
    }

    /// Check if this node is a compound (has children)
    pub fn is_compound(&self) -> bool {
        !self.children.is_empty()
    }

    /// Check if this node has a parent
    pub fn is_child(&self) -> bool {
        self.parent.is_some()
    }
}

/// An edge with its computed layout (routing points)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutedEdge {
    /// Unique edge ID
    pub id: String,
    /// Source node ID
    pub source_id: String,
    /// Target node ID
    pub target_id: String,
    /// Source port position
    pub source_port: Point,
    /// Target port position
    pub target_port: Point,
    /// Intermediate bend points for orthogonal routing
    /// (empty = straight line from source_port to target_port)
    pub bend_points: Vec<Point>,
    /// Edge label (e.g. "Uses", "Reads/Writes")
    pub label: Option<String>,
    /// Relationship kind for arrow styling
    pub kind: String,
    /// CSS class or style name
    pub style_class: String,
    /// Render order
    pub z_index: i32,
}

impl LayoutedEdge {
    /// Get all routing points in order (source_port → bend_points → target_port)
    pub fn routing_points(&self) -> Vec<Point> {
        let mut points = vec![self.source_port];
        points.extend(&self.bend_points);
        points.push(self.target_port);
        points
    }
}

/// Layout direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutDirection {
    /// Top to Bottom
    TB,
    /// Left to Right
    LR,
    /// Bottom to Top
    BT,
    /// Right to Left
    RL,
}

/// Configuration for the layout algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Layout direction (default: TB)
    pub direction: LayoutDirection,
    /// Horizontal spacing between nodes in the same rank (default: 50.0)
    pub node_separation: f64,
    /// Vertical spacing between ranks (default: 80.0)
    pub rank_separation: f64,
    /// Margin around the entire diagram (default: 20.0)
    pub margin: f64,
    /// Minimum node width (default: 120.0)
    pub min_node_width: f64,
    /// Minimum node height (default: 60.0)
    pub min_node_height: f64,
    /// Maximum node width (default: 300.0)
    pub max_node_width: f64,
    /// Maximum node height (default: 200.0)
    pub max_node_height: f64,
    /// Whether to use orthogonal edge routing (default: true)
    pub orthogonal_routing: bool,
    /// Compound node padding between parent and children (default: 30.0)
    pub compound_padding: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            direction: LayoutDirection::TB,
            node_separation: 50.0,
            rank_separation: 80.0,
            margin: 20.0,
            min_node_width: 120.0,
            min_node_height: 60.0,
            max_node_width: 300.0,
            max_node_height: 200.0,
            orthogonal_routing: true,
            compound_padding: 30.0,
        }
    }
}

/// A complete layouted diagram ready for rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutedDiagram {
    /// All nodes with their computed positions
    pub nodes: Vec<LayoutedNode>,
    /// All edges with their computed routing
    pub edges: Vec<LayoutedEdge>,
    /// Total bounding box (computed from all node positions + sizes)
    pub bounds: (f64, f64, f64, f64), // (x, y, width, height)
    /// Layout configuration used
    pub config: LayoutConfig,
}

impl LayoutedDiagram {
    /// Calculate and update bounds based on node positions and sizes
    pub fn compute_bounds(&mut self) {
        if self.nodes.is_empty() {
            self.bounds = (0.0, 0.0, 0.0, 0.0);
            return;
        }
        let m = self.config.margin;
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        for node in &self.nodes {
            let (x, y, w, h) = node.bounds();
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + w);
            max_y = max_y.max(y + h);
        }
        self.bounds = (
            min_x - m,
            min_y - m,
            (max_x - min_x) + 2.0 * m,
            (max_y - min_y) + 2.0 * m,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_new() {
        let p = Point::new(3.0, 4.0);
        assert_eq!(p.x, 3.0);
        assert_eq!(p.y, 4.0);
    }

    #[test]
    fn test_layouted_node_center() {
        let node = LayoutedNode {
            id: "n1".into(),
            label: "Test".into(),
            position: Point::new(10.0, 20.0),
            size: (100.0, 50.0),
            ports: vec![],
            style_class: "default".into(),
            children: vec![],
            parent: None,
            kind: "system".into(),
            technology: None,
            description: None,
            z_index: 0,
        };
        let center = node.center();
        assert!((center.x - 60.0).abs() < 1e-9);
        assert!((center.y - 45.0).abs() < 1e-9);
    }

    #[test]
    fn test_layouted_node_is_compound() {
        let compound = LayoutedNode {
            id: "n1".into(),
            label: "Parent".into(),
            position: Point::new(0.0, 0.0),
            size: (200.0, 100.0),
            ports: vec![],
            style_class: "default".into(),
            children: vec!["c1".into(), "c2".into()],
            parent: None,
            kind: "system".into(),
            technology: None,
            description: None,
            z_index: 0,
        };
        assert!(compound.is_compound());

        let simple = LayoutedNode {
            id: "n2".into(),
            label: "Child".into(),
            position: Point::new(0.0, 0.0),
            size: (100.0, 50.0),
            ports: vec![],
            style_class: "default".into(),
            children: vec![],
            parent: None,
            kind: "container".into(),
            technology: None,
            description: None,
            z_index: 0,
        };
        assert!(!simple.is_compound());
    }

    #[test]
    fn test_layouted_node_is_child() {
        let child = LayoutedNode {
            id: "c1".into(),
            label: "Child".into(),
            position: Point::new(0.0, 0.0),
            size: (100.0, 50.0),
            ports: vec![],
            style_class: "default".into(),
            children: vec![],
            parent: Some("n1".into()),
            kind: "container".into(),
            technology: None,
            description: None,
            z_index: 0,
        };
        assert!(child.is_child());

        let standalone = LayoutedNode {
            id: "n2".into(),
            label: "Standalone".into(),
            position: Point::new(0.0, 0.0),
            size: (100.0, 50.0),
            ports: vec![],
            style_class: "default".into(),
            children: vec![],
            parent: None,
            kind: "system".into(),
            technology: None,
            description: None,
            z_index: 0,
        };
        assert!(!standalone.is_child());
    }

    #[test]
    fn test_layouted_edge_routing_points() {
        let edge = LayoutedEdge {
            id: "e1".into(),
            source_id: "n1".into(),
            target_id: "n2".into(),
            source_port: Point::new(0.0, 0.0),
            target_port: Point::new(100.0, 100.0),
            bend_points: vec![Point::new(50.0, 0.0), Point::new(50.0, 100.0)],
            label: Some("uses".into()),
            kind: "uses".into(),
            style_class: "default".into(),
            z_index: 0,
        };
        let points = edge.routing_points();
        assert_eq!(points.len(), 4);
        assert!((points[0].x - 0.0).abs() < 1e-9);
        assert!((points[0].y - 0.0).abs() < 1e-9);
        assert!((points[1].x - 50.0).abs() < 1e-9);
        assert!((points[2].x - 50.0).abs() < 1e-9);
        assert!((points[3].x - 100.0).abs() < 1e-9);
        assert!((points[3].y - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_layouted_diagram_compute_bounds_empty() {
        let mut diagram = LayoutedDiagram {
            nodes: vec![],
            edges: vec![],
            bounds: (0.0, 0.0, 0.0, 0.0),
            config: LayoutConfig::default(),
        };
        diagram.compute_bounds();
        assert_eq!(diagram.bounds, (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn test_layouted_diagram_compute_bounds() {
        let nodes = vec![
            LayoutedNode {
                id: "n1".into(),
                label: "Node1".into(),
                position: Point::new(10.0, 20.0),
                size: (100.0, 50.0),
                ports: vec![],
                style_class: "default".into(),
                children: vec![],
                parent: None,
                kind: "system".into(),
                technology: None,
                description: None,
                z_index: 0,
            },
            LayoutedNode {
                id: "n2".into(),
                label: "Node2".into(),
                position: Point::new(200.0, 150.0),
                size: (80.0, 60.0),
                ports: vec![],
                style_class: "default".into(),
                children: vec![],
                parent: None,
                kind: "container".into(),
                technology: None,
                description: None,
                z_index: 0,
            },
            LayoutedNode {
                id: "n3".into(),
                label: "Node3".into(),
                position: Point::new(50.0, 200.0),
                size: (120.0, 40.0),
                ports: vec![],
                style_class: "default".into(),
                children: vec![],
                parent: None,
                kind: "component".into(),
                technology: None,
                description: None,
                z_index: 0,
            },
        ];
        let mut diagram = LayoutedDiagram {
            nodes,
            edges: vec![],
            bounds: (0.0, 0.0, 0.0, 0.0),
            config: LayoutConfig::default(),
        };
        diagram.compute_bounds();
        // n1: x=10, y=20, w=100, h=50 -> right=110, bottom=70
        // n2: x=200, y=150, w=80, h=60 -> right=280, bottom=210
        // n3: x=50, y=200, w=120, h=40 -> right=170, bottom=240
        // min_x=10, min_y=20, max_x=280, max_y=240
        // margin=20
        let m = 20.0;
        assert!((diagram.bounds.0 - (10.0 - m)).abs() < 1e-9);
        assert!((diagram.bounds.1 - (20.0 - m)).abs() < 1e-9);
        assert!((diagram.bounds.2 - (280.0 - 10.0 + 2.0 * m)).abs() < 1e-9);
        assert!((diagram.bounds.3 - (240.0 - 20.0 + 2.0 * m)).abs() < 1e-9);
    }

    #[test]
    fn test_layout_config_default() {
        let config = LayoutConfig::default();
        assert_eq!(config.direction, LayoutDirection::TB);
        assert_eq!(config.node_separation, 50.0);
        assert_eq!(config.rank_separation, 80.0);
        assert_eq!(config.margin, 20.0);
        assert_eq!(config.min_node_width, 120.0);
        assert_eq!(config.min_node_height, 60.0);
        assert_eq!(config.max_node_width, 300.0);
        assert_eq!(config.max_node_height, 200.0);
        assert!(config.orthogonal_routing);
        assert_eq!(config.compound_padding, 30.0);
    }
}
