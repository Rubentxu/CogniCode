//! Port assignment for edges based on layout direction.
//!
//! Assigns ports to nodes based on the layout direction (TB or LR) and
//! distributes multiple edges on the same side evenly to avoid overlap.

use std::collections::HashMap;

use crate::layout::types::{
    LayoutDirection, LayoutedDiagram, LayoutedNode, Point, Port, PortSide,
};

/// Assign ports to all edges in a layouted diagram based on layout direction.
///
/// Modifies the nodes' ports and edges' source_port/target_port in place.
pub fn assign_ports(diagram: &mut LayoutedDiagram) {
    let direction = diagram.config.direction;

    // Group edges by source node id and source side, and by target node id and target side
    let mut source_side_map: HashMap<(String, PortSide), Vec<usize>> = HashMap::new();
    let mut target_side_map: HashMap<(String, PortSide), Vec<usize>> = HashMap::new();

    // First pass: group edge indices by their port sides
    for (idx, edge) in diagram.edges.iter().enumerate() {
        let (source_side, target_side) = port_sides_for_direction(direction);

        source_side_map
            .entry((edge.source_id.clone(), source_side))
            .or_default()
            .push(idx);

        target_side_map
            .entry((edge.target_id.clone(), target_side))
            .or_default()
            .push(idx);
    }

    // Compute offset distribution for each side of each node
    let mut source_offset_map: HashMap<(String, PortSide), Vec<f64>> = HashMap::new();
    let mut target_offset_map: HashMap<(String, PortSide), Vec<f64>> = HashMap::new();

    for ((node_id, side), edge_indices) in source_side_map.iter() {
        let offsets = distribute_offsets(edge_indices.len());
        source_offset_map.insert((node_id.clone(), *side), offsets);
    }

    for ((node_id, side), edge_indices) in target_side_map.iter() {
        let offsets = distribute_offsets(edge_indices.len());
        target_offset_map.insert((node_id.clone(), *side), offsets);
    }

    // Create a map from node id to node for quick lookup
    let node_map: HashMap<&str, &LayoutedNode> = diagram
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    // Second pass: assign ports to each edge
    for edge_idx in 0..diagram.edges.len() {
        let edge = &diagram.edges[edge_idx];
        let source_node = node_map.get(edge.source_id.as_str());
        let target_node = node_map.get(edge.target_id.as_str());

        if source_node.is_none() || target_node.is_none() {
            continue;
        }

        let source_node = *source_node.unwrap();
        let target_node = *target_node.unwrap();

        // Get the offset indices for this edge
        let (source_side, target_side) = port_sides_for_direction(direction);

        // Find this edge's position within its source side group
        let source_edge_indices = source_side_map
            .get(&(edge.source_id.clone(), source_side))
            .cloned()
            .unwrap_or_default();
        let source_pos = source_edge_indices
            .iter()
            .position(|&i| i == edge_idx)
            .unwrap_or(0);

        // Find this edge's position within its target side group
        let target_edge_indices = target_side_map
            .get(&(edge.target_id.clone(), target_side))
            .cloned()
            .unwrap_or_default();
        let target_pos = target_edge_indices
            .iter()
            .position(|&i| i == edge_idx)
            .unwrap_or(0);

        // Get the offset arrays for source and target
        let source_offsets = source_offset_map
            .get(&(edge.source_id.clone(), source_side))
            .map(|v| v.as_slice())
            .unwrap_or(&[0.5]);
        let target_offsets = target_offset_map
            .get(&(edge.target_id.clone(), target_side))
            .map(|v| v.as_slice())
            .unwrap_or(&[0.5]);

        // Compute ports and bend points
        let (source_port, target_port, bend_points) = compute_ports_and_path_internal(
            source_node,
            target_node,
            direction,
            source_pos,
            target_pos,
            source_offsets,
            target_offsets,
        );

        // Update the edge
        diagram.edges[edge_idx].source_port = source_port.position;
        diagram.edges[edge_idx].target_port = target_port.position;
        diagram.edges[edge_idx].bend_points = bend_points;
    }

    // Third pass: populate node.ports from all edges that connect to it
    // We need to collect all ports per node
    let mut node_ports: HashMap<String, Vec<Port>> = HashMap::new();

    for edge in &diagram.edges {
        // Get the computed port positions from the edge
        let source_pos = edge.source_port;
        let target_pos = edge.target_port;

        // Create ports for source and target
        let (src_side, tgt_side) = port_sides_for_direction(direction);

        node_ports
            .entry(edge.source_id.clone())
            .or_default()
            .push(Port {
                side: src_side,
                offset: 0.5, // Will be updated with correct offset
                position: source_pos,
                connected_edge: Some(edge.id.clone()),
            });

        node_ports
            .entry(edge.target_id.clone())
            .or_default()
            .push(Port {
                side: tgt_side,
                offset: 0.5, // Will be updated with correct offset
                position: target_pos,
                connected_edge: Some(edge.id.clone()),
            });
    }

    // Update node ports
    for node in &mut diagram.nodes {
        if let Some(ports) = node_ports.get(&node.id) {
            node.ports = ports.clone();
        }
    }
}

/// Given a source and target node (with positions and sizes),
/// compute the optimal port positions and bend points.
fn compute_ports_and_path(
    source: &LayoutedNode,
    target: &LayoutedNode,
    direction: LayoutDirection,
) -> (Port, Port, Vec<Point>) {
    compute_ports_and_path_internal(source, target, direction, 0, 0, &[0.5], &[0.5])
}

/// Internal implementation of port and path computation with specific offset positions
fn compute_ports_and_path_internal(
    source: &LayoutedNode,
    target: &LayoutedNode,
    direction: LayoutDirection,
    source_pos: usize,
    target_pos: usize,
    source_offsets: &[f64],
    target_offsets: &[f64],
) -> (Port, Port, Vec<Point>) {
    let (source_side, target_side) = port_sides_for_direction(direction);

    // Use the distributed offsets to avoid port overlap
    let source_offset = source_offsets.get(source_pos).copied().unwrap_or(0.5);
    let target_offset = target_offsets.get(target_pos).copied().unwrap_or(0.5);

    // Compute port positions based on node geometry
    let source_port_pos = compute_port_position(source, source_side, source_offset);
    let target_port_pos = compute_port_position(target, target_side, target_offset);

    let source_port = Port {
        side: source_side,
        offset: source_offset,
        position: source_port_pos,
        connected_edge: None,
    };

    let target_port = Port {
        side: target_side,
        offset: target_offset,
        position: target_port_pos,
        connected_edge: None,
    };

    // Compute bend points for orthogonal routing
    let bend_points = compute_bend_points(source_port_pos, target_port_pos, direction);

    (source_port, target_port, bend_points)
}

/// Get the port sides for source and target based on layout direction.
fn port_sides_for_direction(direction: LayoutDirection) -> (PortSide, PortSide) {
    match direction {
        LayoutDirection::TB | LayoutDirection::BT => {
            // TB: source on South, target on North
            // BT: source on North, target on South
            if direction == LayoutDirection::TB {
                (PortSide::South, PortSide::North)
            } else {
                (PortSide::North, PortSide::South)
            }
        }
        LayoutDirection::LR | LayoutDirection::RL => {
            // LR: source on East, target on West
            // RL: source on West, target on East
            if direction == LayoutDirection::LR {
                (PortSide::East, PortSide::West)
            } else {
                (PortSide::West, PortSide::East)
            }
        }
    }
}

/// Compute the absolute position of a port on a node's boundary.
fn compute_port_position(node: &LayoutedNode, side: PortSide, offset: f64) -> Point {
    let (x, y) = (node.position.x, node.position.y);
    let (w, h) = (node.size.0, node.size.1);

    match side {
        PortSide::South => Point::new(x + w * offset, y + h),
        PortSide::North => Point::new(x + w * offset, y),
        PortSide::East => Point::new(x + w, y + h * offset),
        PortSide::West => Point::new(x, y + h * offset),
    }
}

/// Distribute offsets evenly along a side to avoid port overlap.
///
/// - 1 edge → offset 0.5 (center)
/// - 2 edges → offsets 0.25, 0.75
/// - 3 edges → offsets 0.166, 0.5, 0.833
/// - N edges → distribute evenly avoiding extremes
fn distribute_offsets(count: usize) -> Vec<f64> {
    if count == 0 {
        return vec![];
    }
    if count == 1 {
        return vec![0.5];
    }

    // Use a small epsilon to avoid extreme positions (0.0 and 1.0)
    let epsilon = 0.001;
    let range = 1.0 - 2.0 * epsilon;

    (0..count)
        .map(|i| {
            let normalized = (i as f64 + 0.5) / (count as f64);
            epsilon + normalized * range
        })
        .collect()
}

/// Compute bend points for orthogonal routing between two ports.
fn compute_bend_points(source: Point, target: Point, direction: LayoutDirection) -> Vec<Point> {
    match direction {
        LayoutDirection::TB => {
            // For TB: create horizontal then vertical segments
            let mid_y = (source.y + target.y) / 2.0;
            if (source.x - target.x).abs() < 1e-9 {
                // Same x-coordinate, no horizontal bend needed
                vec![]
            } else {
                // Need horizontal then vertical routing
                vec![
                    Point::new(source.x, mid_y),
                    Point::new(target.x, mid_y),
                ]
            }
        }
        LayoutDirection::LR => {
            // For LR: create vertical then horizontal segments
            let mid_x = (source.x + target.x) / 2.0;
            if (source.y - target.y).abs() < 1e-9 {
                // Same y-coordinate, no vertical bend needed
                vec![]
            } else {
                vec![
                    Point::new(mid_x, source.y),
                    Point::new(mid_x, target.y),
                ]
            }
        }
        LayoutDirection::BT => {
            // Bottom to top - similar to TB but reversed
            let mid_y = (source.y + target.y) / 2.0;
            if (source.x - target.x).abs() < 1e-9 {
                vec![]
            } else {
                vec![
                    Point::new(source.x, mid_y),
                    Point::new(target.x, mid_y),
                ]
            }
        }
        LayoutDirection::RL => {
            // Right to left - similar to LR but reversed
            let mid_x = (source.x + target.x) / 2.0;
            if (source.y - target.y).abs() < 1e-9 {
                vec![]
            } else {
                vec![
                    Point::new(mid_x, source.y),
                    Point::new(mid_x, target.y),
                ]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::types::{LayoutConfig, LayoutedEdge};

    fn create_test_node(id: &str, x: f64, y: f64, w: f64, h: f64) -> LayoutedNode {
        LayoutedNode {
            id: id.into(),
            label: id.into(),
            position: Point::new(x, y),
            size: (w, h),
            ports: vec![],
            style_class: "default".into(),
            children: vec![],
            parent: None,
            kind: "system".into(),
            technology: None,
            description: None,
            z_index: 0,
        }
    }

    fn create_test_diagram(
        nodes: Vec<LayoutedNode>,
        edges: Vec<LayoutedEdge>,
        direction: LayoutDirection,
    ) -> LayoutedDiagram {
        LayoutedDiagram {
            nodes,
            edges,
            bounds: (0.0, 0.0, 0.0, 0.0),
            config: LayoutConfig {
                direction,
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_port_position_south() {
        let node = create_test_node("n1", 100.0, 100.0, 80.0, 40.0);
        let pos = compute_port_position(&node, PortSide::South, 0.5);
        assert!((pos.x - 140.0).abs() < 1e-9); // 100 + 80/2
        assert!((pos.y - 140.0).abs() < 1e-9); // 100 + 40
    }

    #[test]
    fn test_port_position_north() {
        let node = create_test_node("n1", 100.0, 100.0, 80.0, 40.0);
        let pos = compute_port_position(&node, PortSide::North, 0.5);
        assert!((pos.x - 140.0).abs() < 1e-9); // 100 + 80/2
        assert!((pos.y - 100.0).abs() < 1e-9); // 100
    }

    #[test]
    fn test_port_position_east() {
        let node = create_test_node("n1", 100.0, 100.0, 80.0, 40.0);
        let pos = compute_port_position(&node, PortSide::East, 0.5);
        assert!((pos.x - 180.0).abs() < 1e-9); // 100 + 80
        assert!((pos.y - 120.0).abs() < 1e-9); // 100 + 40/2
    }

    #[test]
    fn test_port_position_west() {
        let node = create_test_node("n1", 100.0, 100.0, 80.0, 40.0);
        let pos = compute_port_position(&node, PortSide::West, 0.5);
        assert!((pos.x - 100.0).abs() < 1e-9); // 100
        assert!((pos.y - 120.0).abs() < 1e-9); // 100 + 40/2
    }

    #[test]
    fn test_distribute_offsets_single() {
        let offsets = distribute_offsets(1);
        assert_eq!(offsets.len(), 1);
        assert!((offsets[0] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_distribute_offsets_two() {
        let offsets = distribute_offsets(2);
        assert_eq!(offsets.len(), 2);
        // Should be roughly 0.25 and 0.75 (with epsilon padding)
        assert!(offsets[0] > 0.2 && offsets[0] < 0.3);
        assert!(offsets[1] > 0.7 && offsets[1] < 0.8);
    }

    #[test]
    fn test_distribute_offsets_three() {
        let offsets = distribute_offsets(3);
        assert_eq!(offsets.len(), 3);
        // Center one should be near 0.5
        assert!((offsets[1] - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_distribute_offsets_no_extremes() {
        // Ensure no offset is at 0.0 or 1.0
        for count in 1..=10 {
            let offsets = distribute_offsets(count);
            for &offset in &offsets {
                assert!(offset > 0.0 && offset < 1.0);
            }
        }
    }

    #[test]
    fn test_assign_ports_tb_simple() {
        // Two nodes: source above target, one edge
        let source = create_test_node("source", 100.0, 50.0, 80.0, 40.0);
        let target = create_test_node("target", 100.0, 150.0, 80.0, 40.0);

        let edge = LayoutedEdge {
            id: "e1".into(),
            source_id: "source".into(),
            target_id: "target".into(),
            source_port: Point::new(0.0, 0.0),
            target_port: Point::new(0.0, 0.0),
            bend_points: vec![],
            label: None,
            kind: "uses".into(),
            style_class: "default".into(),
            z_index: 0,
        };

        let mut diagram = create_test_diagram(vec![source, target], vec![edge], LayoutDirection::TB);
        assign_ports(&mut diagram);

        // Source port should be on South side (bottom)
        assert!((diagram.edges[0].source_port.x - 140.0).abs() < 1e-9); // center x
        assert!((diagram.edges[0].source_port.y - 90.0).abs() < 1e-9); // 50 + 40

        // Target port should be on North side (top)
        assert!((diagram.edges[0].target_port.x - 140.0).abs() < 1e-9); // center x
        assert!((diagram.edges[0].target_port.y - 150.0).abs() < 1e-9); // top y
    }

    #[test]
    fn test_assign_ports_lr_simple() {
        // Two nodes: source left of target, one edge
        let source = create_test_node("source", 50.0, 100.0, 80.0, 40.0);
        let target = create_test_node("target", 200.0, 100.0, 80.0, 40.0);

        let edge = LayoutedEdge {
            id: "e1".into(),
            source_id: "source".into(),
            target_id: "target".into(),
            source_port: Point::new(0.0, 0.0),
            target_port: Point::new(0.0, 0.0),
            bend_points: vec![],
            label: None,
            kind: "uses".into(),
            style_class: "default".into(),
            z_index: 0,
        };

        let mut diagram = create_test_diagram(vec![source, target], vec![edge], LayoutDirection::LR);
        assign_ports(&mut diagram);

        // Source port should be on East side (right)
        assert!((diagram.edges[0].source_port.x - 130.0).abs() < 1e-9); // 50 + 80
        assert!((diagram.edges[0].source_port.y - 120.0).abs() < 1e-9); // center y

        // Target port should be on West side (left)
        assert!((diagram.edges[0].target_port.x - 200.0).abs() < 1e-9); // left x
        assert!((diagram.edges[0].target_port.y - 120.0).abs() < 1e-9); // center y
    }

    #[test]
    fn test_multiple_edges_same_side_distributed() {
        // One node with 3 edges going to 3 different targets below it
        let source = create_test_node("source", 100.0, 50.0, 80.0, 40.0);
        let target1 = create_test_node("target1", 50.0, 150.0, 60.0, 30.0);
        let target2 = create_test_node("target2", 120.0, 150.0, 60.0, 30.0);
        let target3 = create_test_node("target3", 190.0, 150.0, 60.0, 30.0);

        let edges = vec![
            LayoutedEdge {
                id: "e1".into(),
                source_id: "source".into(),
                target_id: "target1".into(),
                source_port: Point::new(0.0, 0.0),
                target_port: Point::new(0.0, 0.0),
                bend_points: vec![],
                label: None,
                kind: "uses".into(),
                style_class: "default".into(),
                z_index: 0,
            },
            LayoutedEdge {
                id: "e2".into(),
                source_id: "source".into(),
                target_id: "target2".into(),
                source_port: Point::new(0.0, 0.0),
                target_port: Point::new(0.0, 0.0),
                bend_points: vec![],
                label: None,
                kind: "uses".into(),
                style_class: "default".into(),
                z_index: 0,
            },
            LayoutedEdge {
                id: "e3".into(),
                source_id: "source".into(),
                target_id: "target3".into(),
                source_port: Point::new(0.0, 0.0),
                target_port: Point::new(0.0, 0.0),
                bend_points: vec![],
                label: None,
                kind: "uses".into(),
                style_class: "default".into(),
                z_index: 0,
            },
        ];

        let mut diagram = create_test_diagram(
            vec![source, target1, target2, target3],
            edges,
            LayoutDirection::TB,
        );
        assign_ports(&mut diagram);

        // All source ports should have different x positions (on South side)
        let x1 = diagram.edges[0].source_port.x;
        let x2 = diagram.edges[1].source_port.x;
        let x3 = diagram.edges[2].source_port.x;

        // All should be different (distributed)
        assert!(x1 != x2 && x2 != x3 && x1 != x3);

        // All should be within the node's width
        assert!(x1 >= 100.0 && x1 <= 180.0);
        assert!(x2 >= 100.0 && x2 <= 180.0);
        assert!(x3 >= 100.0 && x3 <= 180.0);
    }

    #[test]
    fn test_bend_points_for_horizontal_crossing() {
        // Source and target at same vertical level
        let source = create_test_node("source", 50.0, 100.0, 80.0, 40.0);
        let target = create_test_node("target", 200.0, 100.0, 80.0, 40.0);

        let edge = LayoutedEdge {
            id: "e1".into(),
            source_id: "source".into(),
            target_id: "target".into(),
            source_port: Point::new(0.0, 0.0),
            target_port: Point::new(0.0, 0.0),
            bend_points: vec![],
            label: None,
            kind: "uses".into(),
            style_class: "default".into(),
            z_index: 0,
        };

        let mut diagram = create_test_diagram(vec![source, target], vec![edge], LayoutDirection::LR);
        assign_ports(&mut diagram);

        // For LR at same y-level, no bend points needed
        assert!(diagram.edges[0].bend_points.is_empty());
    }

    #[test]
    fn test_bend_points_for_vertical_routing() {
        // Source and target at different x positions (needs horizontal routing)
        let source = create_test_node("source", 50.0, 50.0, 80.0, 40.0);
        let target = create_test_node("target", 200.0, 150.0, 80.0, 40.0);

        let edge = LayoutedEdge {
            id: "e1".into(),
            source_id: "source".into(),
            target_id: "target".into(),
            source_port: Point::new(0.0, 0.0),
            target_port: Point::new(0.0, 0.0),
            bend_points: vec![],
            label: None,
            kind: "uses".into(),
            style_class: "default".into(),
            z_index: 0,
        };

        let mut diagram = create_test_diagram(vec![source, target], vec![edge], LayoutDirection::TB);
        assign_ports(&mut diagram);

        // For TB, should have bend points for horizontal routing
        assert!(!diagram.edges[0].bend_points.is_empty());
    }

    #[test]
    fn test_port_assignment_updates_nodes() {
        let source = create_test_node("source", 100.0, 50.0, 80.0, 40.0);
        let target = create_test_node("target", 100.0, 150.0, 80.0, 40.0);

        let edge = LayoutedEdge {
            id: "e1".into(),
            source_id: "source".into(),
            target_id: "target".into(),
            source_port: Point::new(0.0, 0.0),
            target_port: Point::new(0.0, 0.0),
            bend_points: vec![],
            label: None,
            kind: "uses".into(),
            style_class: "default".into(),
            z_index: 0,
        };

        let mut diagram = create_test_diagram(vec![source, target], vec![edge], LayoutDirection::TB);
        assign_ports(&mut diagram);

        // Verify node.ports is populated
        let source_node = diagram.nodes.iter().find(|n| n.id == "source").unwrap();
        let target_node = diagram.nodes.iter().find(|n| n.id == "target").unwrap();

        // Source should have a port on South side
        assert!(!source_node.ports.is_empty());
        assert_eq!(source_node.ports[0].side, PortSide::South);

        // Target should have a port on North side
        assert!(!target_node.ports.is_empty());
        assert_eq!(target_node.ports[0].side, PortSide::North);
    }

    #[test]
    fn test_compute_ports_and_path() {
        let source = create_test_node("source", 100.0, 50.0, 80.0, 40.0);
        let target = create_test_node("target", 100.0, 150.0, 80.0, 40.0);

        let (source_port, target_port, bend_points) =
            compute_ports_and_path(&source, &target, LayoutDirection::TB);

        assert_eq!(source_port.side, PortSide::South);
        assert_eq!(target_port.side, PortSide::North);
        assert!((source_port.position.x - 140.0).abs() < 1e-9);
        assert!((source_port.position.y - 90.0).abs() < 1e-9);
        assert!((target_port.position.x - 140.0).abs() < 1e-9);
        assert!((target_port.position.y - 150.0).abs() < 1e-9);

        // Bend points should exist since x is the same
        assert!(bend_points.is_empty());
    }

    #[test]
    fn test_layout_direction_bt() {
        // Bottom to top: source on North, target on South
        let source = create_test_node("source", 100.0, 150.0, 80.0, 40.0);
        let target = create_test_node("target", 100.0, 50.0, 80.0, 40.0);

        let edge = LayoutedEdge {
            id: "e1".into(),
            source_id: "source".into(),
            target_id: "target".into(),
            source_port: Point::new(0.0, 0.0),
            target_port: Point::new(0.0, 0.0),
            bend_points: vec![],
            label: None,
            kind: "uses".into(),
            style_class: "default".into(),
            z_index: 0,
        };

        let mut diagram = create_test_diagram(vec![source, target], vec![edge], LayoutDirection::BT);
        assign_ports(&mut diagram);

        // Source port should be on North side
        assert!((diagram.edges[0].source_port.y - 150.0).abs() < 1e-9); // y position

        // Target port should be on South side
        assert!((diagram.edges[0].target_port.y - 90.0).abs() < 1e-9); // y + h
    }

    #[test]
    fn test_layout_direction_rl() {
        // Right to left: source on West, target on East
        let source = create_test_node("source", 200.0, 100.0, 80.0, 40.0);
        let target = create_test_node("target", 50.0, 100.0, 80.0, 40.0);

        let edge = LayoutedEdge {
            id: "e1".into(),
            source_id: "source".into(),
            target_id: "target".into(),
            source_port: Point::new(0.0, 0.0),
            target_port: Point::new(0.0, 0.0),
            bend_points: vec![],
            label: None,
            kind: "uses".into(),
            style_class: "default".into(),
            z_index: 0,
        };

        let mut diagram = create_test_diagram(vec![source, target], vec![edge], LayoutDirection::RL);
        assign_ports(&mut diagram);

        // Source port should be on West side
        assert!((diagram.edges[0].source_port.x - 200.0).abs() < 1e-9); // x position

        // Target port should be on East side
        assert!((diagram.edges[0].target_port.x - 130.0).abs() < 1e-9); // x + w
    }
}
