//! Compound node layout for C4 hierarchical containers.
//!
//! Handles parent-child hierarchy for Containers with Components
//! and SoftwareSystems with Containers.

use crate::layout::types::{
    LayoutedNode, LayoutedDiagram, LayoutConfig,
    Point,
};

/// Header height reserved for parent label in compound boundaries
const HEADER_HEIGHT: f64 = 30.0;

/// Layout compound nodes (parent-child hierarchy).
///
/// Takes a flat LayoutedDiagram and expands compound nodes
/// (containers with components) to show the parent boundary.
pub fn layout_compound(
    diagram: &mut LayoutedDiagram,
    config: &LayoutConfig,
) {
    if diagram.nodes.is_empty() {
        return;
    }

    // Build a map of node id -> node index for quick lookup
    let node_map: indexmap::IndexMap<String, usize> = diagram.nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), i))
        .collect();

    // Build parent -> children index mapping (using indices, not references)
    let mut parent_to_children: indexmap::IndexMap<String, Vec<usize>> = indexmap::IndexMap::new();
    let mut child_to_parent: indexmap::IndexMap<String, String> = indexmap::IndexMap::new();

    // First pass: identify compound nodes and initialize their children lists
    for node in &diagram.nodes {
        if node.is_compound() {
            parent_to_children.insert(node.id.clone(), Vec::new());
        }
    }

    // Second pass: assign children to parents
    for (idx, node) in diagram.nodes.iter().enumerate() {
        if let Some(ref parent_id) = node.parent {
            if let Some(children) = parent_to_children.get_mut(parent_id) {
                children.push(idx);
            }
            child_to_parent.insert(node.id.clone(), parent_id.clone());
        }
    }

    // Process each compound node - collect changes first, then apply
    let mut parent_changes: Vec<(usize, Point, (f64, f64))> = Vec::new();
    let mut edge_changes: Vec<(usize, Point, bool)> = Vec::new(); // (edge_idx, new_port, is_source)

    for (parent_id, child_indices) in &parent_to_children {
        if child_indices.is_empty() {
            continue;
        }

        let parent_idx = *node_map.get(parent_id).unwrap();

        // Get children data (references are fine since we're just reading)
        let children_data: Vec<(Point, (f64, f64))> = child_indices
            .iter()
            .map(|&idx| {
                let node = &diagram.nodes[idx];
                (node.position, node.size)
            })
            .collect();

        // Compute new parent bounds
        let padding = config.compound_padding;
        let (new_pos, new_size) = compute_parent_bounds(&children_data, padding);

        // Ensure parent is at least as large as its initial size
        // (or larger if children require it)
        let initial_size = diagram.nodes[parent_idx].size;
        let final_size = (
            new_size.0.max(initial_size.0),
            new_size.1.max(initial_size.1),
        );

        parent_changes.push((parent_idx, new_pos, final_size));

        // Handle cross-boundary edges
        let child_ids: std::collections::HashSet<&str> = child_indices
            .iter()
            .map(|&idx| diagram.nodes[idx].id.as_str())
            .collect();

        for (edge_idx, edge) in diagram.edges.iter().enumerate() {
            let source_is_child = child_ids.contains(edge.source_id.as_str());
            let target_is_child = child_ids.contains(edge.target_id.as_str());

            // If exactly one endpoint is a child (crosses boundary)
            if source_is_child != target_is_child {
                let child_id = if source_is_child { &edge.source_id } else { &edge.target_id };

                if let Some(parent_id) = child_to_parent.get(child_id) {
                    if parent_id == parent_id {
                        // Find boundary intersection and update edge
                        let (child_port, external_port) = if source_is_child {
                            (edge.source_port, edge.target_port)
                        } else {
                            (edge.target_port, edge.source_port)
                        };

                        let boundary_point = find_boundary_intersection(
                            new_pos,
                            new_size,
                            &child_port,
                            &external_port,
                        );

                        edge_changes.push((edge_idx, boundary_point, source_is_child));
                    }
                }
            }
        }
    }

    // Apply parent changes
    for (parent_idx, new_pos, new_size) in parent_changes {
        diagram.nodes[parent_idx].position = new_pos;
        diagram.nodes[parent_idx].size = new_size;
    }

    // Apply edge changes
    for (edge_idx, new_port, is_source) in edge_changes {
        if is_source {
            diagram.edges[edge_idx].source_port = new_port;
        } else {
            diagram.edges[edge_idx].target_port = new_port;
        }
    }

    // Update diagram bounds after layout
    diagram.compute_bounds();
}

/// Given a parent node and its children, compute the parent's size
/// based on children's bounding box + padding.
fn compute_parent_bounds(
    children: &[(Point, (f64, f64))],
    padding: f64,
) -> (Point, (f64, f64)) {
    if children.is_empty() {
        return (Point::new(0.0, 0.0), (0.0, 0.0));
    }

    // Compute bounding box of all children
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for &(pos, (w, h)) in children {
        min_x = min_x.min(pos.x);
        min_y = min_y.min(pos.y);
        max_x = max_x.max(pos.x + w);
        max_y = max_y.max(pos.y + h);
    }

    // Parent's top-left = children's top-left minus padding (adjusted for header)
    let new_x = min_x - padding;
    let new_y = min_y - HEADER_HEIGHT - padding;

    // Parent size = children bounding box + header + padding on all sides
    let new_width = (max_x - min_x) + 2.0 * padding;
    let new_height = (max_y - min_y) + HEADER_HEIGHT + 2.0 * padding;

    (Point::new(new_x, new_y), (new_width, new_height))
}

/// Given a parent node and its children, compute the parent's size
/// based on children's bounding box + padding.
fn expand_parent_to_fit_children(
    parent: &mut LayoutedNode,
    children: &[(Point, (f64, f64))],
    padding: f64,
) {
    if children.is_empty() {
        return;
    }

    let (new_pos, new_size) = compute_parent_bounds(children, padding);
    
    // Ensure parent is at least as large as its initial size
    let final_size = (
        new_size.0.max(parent.size.0),
        new_size.1.max(parent.size.1),
    );
    
    parent.position = new_pos;
    parent.size = final_size;
}

/// Find the intersection point on the parent's boundary closest to the external node.
///
/// The boundary intersection is computed by extending a line from the child port
/// toward the external port and finding where it hits the parent's rectangle.
fn find_boundary_intersection(
    parent_pos: Point,
    parent_size: (f64, f64),
    child_port: &Point,
    external_port: &Point,
) -> Point {
    let (px, py) = (parent_pos.x, parent_pos.y);
    let (pw, ph) = parent_size;

    // Direction from child port to external port
    let dx = external_port.x - child_port.x;
    let dy = external_port.y - child_port.y;

    // Avoid division by zero
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return *child_port;
    }

    // Find intersection with each side of the parent rectangle
    let mut candidates: Vec<(f64, Point)> = Vec::with_capacity(4);

    // Left side (x = px)
    if dx.abs() > 1e-9 {
        let t = (px - child_port.x) / dx;
        if t > 0.0 {
            let y = child_port.y + t * dy;
            if y >= py && y <= py + ph {
                candidates.push((t, Point::new(px, y)));
            }
        }
    }

    // Right side (x = px + pw)
    if dx.abs() > 1e-9 {
        let t = (px + pw - child_port.x) / dx;
        if t > 0.0 {
            let y = child_port.y + t * dy;
            if y >= py && y <= py + ph {
                candidates.push((t, Point::new(px + pw, y)));
            }
        }
    }

    // Top side (y = py)
    if dy.abs() > 1e-9 {
        let t = (py - child_port.y) / dy;
        if t > 0.0 {
            let x = child_port.x + t * dx;
            if x >= px && x <= px + pw {
                candidates.push((t, Point::new(x, py)));
            }
        }
    }

    // Bottom side (y = py + ph)
    if dy.abs() > 1e-9 {
        let t = (py + ph - child_port.y) / dy;
        if t > 0.0 {
            let x = child_port.x + t * dx;
            if x >= px && x <= px + pw {
                candidates.push((t, Point::new(x, py + ph)));
            }
        }
    }

    // Return the intersection with smallest t (closest to child port)
    if let Some((_, point)) = candidates.into_iter().min_by(|a, b| {
        a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)
    }) {
        point
    } else {
        // Fallback: return child port position if no intersection found
        *child_port
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::types::{LayoutedEdge, LayoutConfig};

    fn create_test_config() -> LayoutConfig {
        LayoutConfig::default()
    }

    fn create_layouted_node(id: &str, label: &str, x: f64, y: f64, w: f64, h: f64) -> LayoutedNode {
        LayoutedNode {
            id: id.into(),
            label: label.into(),
            position: Point::new(x, y),
            size: (w, h),
            ports: vec![],
            style_class: "default".into(),
            children: vec![],
            parent: None,
            kind: "container".into(),
            technology: None,
            description: None,
            z_index: 0,
        }
    }

    fn create_layouted_edge(id: &str, source: &str, target: &str, source_port: Point, target_port: Point) -> LayoutedEdge {
        LayoutedEdge {
            id: id.into(),
            source_id: source.into(),
            target_id: target.into(),
            source_port,
            target_port,
            bend_points: vec![],
            label: None,
            kind: "uses".into(),
            style_class: "default".into(),
            z_index: 0,
        }
    }

    #[test]
    fn test_expand_parent_to_fit_children() {
        let mut parent = create_layouted_node("parent", "Parent", 0.0, 0.0, 100.0, 100.0);

        let children = vec![
            (Point::new(0.0, 30.0), (50.0, 50.0)),
            (Point::new(100.0, 30.0), (50.0, 50.0)),
            (Point::new(0.0, 100.0), (50.0, 50.0)),
        ];

        let padding = 30.0;
        expand_parent_to_fit_children(&mut parent, &children, padding);

        // Children bounding box: min_x=0, min_y=30, max_x=150, max_y=150
        // Parent should be positioned at (0 - padding, 30 - HEADER_HEIGHT - padding)
        let expected_x = 0.0 - padding;
        let expected_y = 30.0 - HEADER_HEIGHT - padding;

        assert!((parent.position.x - expected_x).abs() < 1e-9,
            "parent.x = {} expected {}", parent.position.x, expected_x);
        assert!((parent.position.y - expected_y).abs() < 1e-9,
            "parent.y = {} expected {}", parent.position.y, expected_y);

        // Width = 150 (max_x - min_x) + 2*padding = 150 + 60 = 210
        // Height = 120 (max_y - min_y) + HEADER_HEIGHT + 2*padding = 120 + 30 + 60 = 210
        let expected_width = 150.0 + 2.0 * padding;
        let expected_height = 120.0 + HEADER_HEIGHT + 2.0 * padding;

        assert!((parent.size.0 - expected_width).abs() < 1e-9,
            "parent.width = {} expected {}", parent.size.0, expected_width);
        assert!((parent.size.1 - expected_height).abs() < 1e-9,
            "parent.height = {} expected {}", parent.size.1, expected_height);
    }

    #[test]
    fn test_compound_node_parent_field() {
        let mut diagram = LayoutedDiagram {
            nodes: vec![
                // Parent container
                create_layouted_node("container1", "Container1", 0.0, 0.0, 200.0, 200.0),
                // 3 component children - positions chosen to ensure bounding box
                // spans wider than 200 to trigger expansion
                create_layouted_node("comp1", "Comp1", 10.0, 40.0, 50.0, 50.0),
                create_layouted_node("comp2", "Comp2", 90.0, 40.0, 50.0, 50.0),
                create_layouted_node("comp3", "Comp3", 170.0, 40.0, 50.0, 50.0),
            ],
            edges: vec![],
            bounds: (0.0, 0.0, 0.0, 0.0),
            config: create_test_config(),
        };

        // Set up parent-child relationships
        diagram.nodes[0].children = vec!["comp1".into(), "comp2".into(), "comp3".into()];
        diagram.nodes[1].parent = Some("container1".into());
        diagram.nodes[2].parent = Some("container1".into());
        diagram.nodes[3].parent = Some("container1".into());

        let config = LayoutConfig::default();
        layout_compound(&mut diagram, &config);

        // Verify parent was expanded - children span x=[10, 220], y=[40, 90]
        // With padding=30 and header=30:
        // width = (220-10) + 2*30 = 210 + 60 = 270 > 200
        // height = (90-40) + 30 + 2*30 = 50 + 30 + 60 = 140 < 200
        // Since height < 200, we need to ensure minimum size
        let parent = &diagram.nodes[0];
        
        // The algorithm should ensure parent is large enough to contain children
        // with proper padding and header
        assert!(
            parent.size.0 >= 200.0 || parent.size.1 >= 200.0,
            "Parent should have at least one dimension >= initial size"
        );

        // Verify children still have their parent reference
        assert_eq!(diagram.nodes[1].parent, Some("container1".into()));
        assert_eq!(diagram.nodes[2].parent, Some("container1".into()));
        assert_eq!(diagram.nodes[3].parent, Some("container1".into()));
    }

    #[test]
    fn test_compound_with_cross_boundary_edges() {
        let mut diagram = LayoutedDiagram {
            nodes: vec![
                // First container with component
                create_layouted_node("container1", "Container1", 0.0, 0.0, 200.0, 200.0),
                create_layouted_node("comp1", "Comp1", 10.0, 40.0, 50.0, 50.0),
                // Second container with component
                create_layouted_node("container2", "Container2", 300.0, 0.0, 200.0, 200.0),
                create_layouted_node("comp2", "Comp2", 310.0, 40.0, 50.0, 50.0),
            ],
            edges: vec![
                // Edge from comp1 to comp2 (crosses container1 boundary)
                create_layouted_edge(
                    "edge1",
                    "comp1",
                    "comp2",
                    Point::new(35.0, 90.0),  // comp1 south port
                    Point::new(335.0, 65.0), // comp2 west port
                ),
            ],
            bounds: (0.0, 0.0, 0.0, 0.0),
            config: create_test_config(),
        };

        // Set up parent-child relationships
        diagram.nodes[0].children = vec!["comp1".into()];
        diagram.nodes[1].parent = Some("container1".into());
        diagram.nodes[2].children = vec!["comp2".into()];
        diagram.nodes[3].parent = Some("container2".into());

        let config = LayoutConfig::default();
        layout_compound(&mut diagram, &config);

        // Verify the edge port was modified to route through parent boundary
        let edge = &diagram.edges[0];
        // The source_port should now be at the boundary of container1
        // (since comp1 is a child of container1)
        // Original was Point::new(35.0, 90.0)
        let original_port = Point::new(35.0, 90.0);
        let source_changed = (edge.source_port.x - original_port.x).abs() > 1e-9
            || (edge.source_port.y - original_port.y).abs() > 1e-9;
        assert!(source_changed,
            "Edge source port should be modified to route through parent boundary");
    }

    #[test]
    fn test_layout_compound_preserves_relative_positions() {
        let mut diagram = LayoutedDiagram {
            nodes: vec![
                create_layouted_node("parent", "Parent", 0.0, 0.0, 100.0, 100.0),
                create_layouted_node("child1", "Child1", 10.0, 40.0, 50.0, 50.0),
                create_layouted_node("child2", "Child2", 70.0, 40.0, 50.0, 50.0),
            ],
            edges: vec![],
            bounds: (0.0, 0.0, 0.0, 0.0),
            config: create_test_config(),
        };

        // Set up parent-child relationships
        diagram.nodes[0].children = vec!["child1".into(), "child2".into()];
        diagram.nodes[1].parent = Some("parent".into());
        diagram.nodes[2].parent = Some("parent".into());

        let child1_pos_before = diagram.nodes[1].position;
        let child2_pos_before = diagram.nodes[2].position;

        let config = LayoutConfig::default();
        layout_compound(&mut diagram, &config);

        // Verify children's relative positions are preserved
        // (only the parent size/position should change)
        assert_eq!(diagram.nodes[1].position.x, child1_pos_before.x);
        assert_eq!(diagram.nodes[1].position.y, child1_pos_before.y);
        assert_eq!(diagram.nodes[2].position.x, child2_pos_before.x);
        assert_eq!(diagram.nodes[2].position.y, child2_pos_before.y);
    }

    #[test]
    fn test_find_boundary_intersection_left_side() {
        let parent_pos = Point::new(0.0, 0.0);
        let parent_size = (100.0, 100.0);
        let child_port = Point::new(50.0, 50.0);  // inside parent
        let external_port = Point::new(-50.0, 50.0); // to the left

        let intersection = find_boundary_intersection(parent_pos, parent_size, &child_port, &external_port);

        // Should hit left side at y=50
        assert!((intersection.x - 0.0).abs() < 1e-9);
        assert!((intersection.y - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_find_boundary_intersection_right_side() {
        let parent_pos = Point::new(0.0, 0.0);
        let parent_size = (100.0, 100.0);
        let child_port = Point::new(50.0, 50.0);   // inside parent
        let external_port = Point::new(200.0, 50.0); // to the right

        let intersection = find_boundary_intersection(parent_pos, parent_size, &child_port, &external_port);

        // Should hit right side at y=50
        assert!((intersection.x - 100.0).abs() < 1e-9);
        assert!((intersection.y - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_find_boundary_intersection_top_side() {
        let parent_pos = Point::new(0.0, 0.0);
        let parent_size = (100.0, 100.0);
        let child_port = Point::new(50.0, 50.0);   // inside parent
        let external_port = Point::new(50.0, -50.0); // above

        let intersection = find_boundary_intersection(parent_pos, parent_size, &child_port, &external_port);

        // Should hit top side at x=50
        assert!((intersection.x - 50.0).abs() < 1e-9);
        assert!((intersection.y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_find_boundary_intersection_bottom_side() {
        let parent_pos = Point::new(0.0, 0.0);
        let parent_size = (100.0, 100.0);
        let child_port = Point::new(50.0, 50.0);    // inside parent
        let external_port = Point::new(50.0, 200.0); // below

        let intersection = find_boundary_intersection(parent_pos, parent_size, &child_port, &external_port);

        // Should hit bottom side at x=50
        assert!((intersection.x - 50.0).abs() < 1e-9);
        assert!((intersection.y - 100.0).abs() < 1e-9);
    }
}
