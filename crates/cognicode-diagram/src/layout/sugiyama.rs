//! Sugiyama layout implementation for C4 diagrams
//!
//! Converts C4Model → rust-sugiyama → layout coordinates.
//!
//! The Sugiyama algorithm produces a layered directed graph layout that:
//! 1. Assigns nodes to ranks (layers) based on graph topology
//! 2. Minimizes edge crossings within each rank
//! 3. Computes x/y positions for each node within its rank

use std::collections::HashMap;

use rust_sugiyama::configure::Config;
use rust_sugiyama::from_vertices_and_edges;

use crate::layout::types::{
    LayoutedDiagram, LayoutedEdge, LayoutedNode, LayoutConfig, LayoutDirection, Point, Port,
    PortSide,
};
use crate::model::c4_types::{ContainerType, ElementId, Person, SoftwareSystem};
use crate::model::relationships::C4Relationship;
use crate::model::workspace::C4Workspace;

/// Compute layout for a C4Workspace using Sugiyama algorithm
///
/// Returns a LayoutedDiagram with positions computed for all nodes and edges.
pub fn compute_layout(
    workspace: &C4Workspace,
    config: &LayoutConfig,
) -> anyhow::Result<LayoutedDiagram> {
    // Step 1: Build mappings from ElementId to vertex index (u32)
    let mut element_to_vertex: HashMap<String, u32> = HashMap::new();
    let mut nodes_data: HashMap<u32, NodeInfo> = HashMap::new();
    let mut vertex_id = 0u32;

    // Step 2: Collect vertices with their sizes
    let mut vertices: Vec<(u32, (f64, f64))> = Vec::new();

    // Add people as nodes
    for person in &workspace.model.people {
        let id_str = person.id.as_str().to_string();
        let size = estimate_node_size(&person.name, "person", config);
        vertices.push((vertex_id, size));
        element_to_vertex.insert(id_str.clone(), vertex_id);
        nodes_data.insert(
            vertex_id,
            NodeInfo {
                id: id_str,
                label: person.name.clone(),
                kind: "person".to_string(),
                technology: None,
                description: Some(person.description.clone()),
                parent: None,
            },
        );
        vertex_id += 1;
    }

    // Add software systems and their containers/components
    for system in &workspace.model.systems {
        let system_id_str = system.id.as_str().to_string();
        let size = estimate_node_size(&system.name, "system", config);
        vertices.push((vertex_id, size));
        element_to_vertex.insert(system_id_str.clone(), vertex_id);
        nodes_data.insert(
            vertex_id,
            NodeInfo {
                id: system_id_str,
                label: system.name.clone(),
                kind: "system".to_string(),
                technology: None,
                description: Some(system.description.clone()),
                parent: None,
            },
        );
        vertex_id += 1;

        // Add containers
        for container in &system.containers {
            let container_id_str = container.id.as_str().to_string();
            let kind = match container.container_type {
                ContainerType::DataStore => "datastore",
                ContainerType::Library
                | ContainerType::Service
                | ContainerType::Executable
                | ContainerType::Queue => "container",
            };
            let size = estimate_node_size(&container.name, kind, config);
            vertices.push((vertex_id, size));
            element_to_vertex.insert(container_id_str.clone(), vertex_id);
            nodes_data.insert(
                vertex_id,
                NodeInfo {
                    id: container_id_str,
                    label: container.name.clone(),
                    kind: kind.to_string(),
                    technology: Some(container.technology.clone()),
                    description: Some(container.description.clone()),
                    parent: Some(system.id.as_str().to_string()),
                },
            );
            vertex_id += 1;

            // Add components within each container
            for component in &container.components {
                let component_id_str = component.id.as_str().to_string();
                let size = estimate_node_size(&component.name, "component", config);
                vertices.push((vertex_id, size));
                element_to_vertex.insert(component_id_str.clone(), vertex_id);
                nodes_data.insert(
                    vertex_id,
                    NodeInfo {
                        id: component_id_str,
                        label: component.name.clone(),
                        kind: "component".to_string(),
                        technology: Some(component.technology.clone()),
                        description: Some(component.description.clone()),
                        parent: Some(container.id.as_str().to_string()),
                    },
                );
                vertex_id += 1;
            }
        }
    }

    // Step 3: Build edges from relationships
    let mut edges: Vec<(u32, u32)> = Vec::new();
    for relationship in &workspace.model.relationships {
        let source_id = relationship.source_id.as_str().to_string();
        let target_id = relationship.target_id.as_str().to_string();

        if let (Some(&source_v), Some(&target_v)) =
            (element_to_vertex.get(&source_id), element_to_vertex.get(&target_id))
        {
            edges.push((source_v, target_v));
        }
    }

    // Step 4: Run Sugiyama layout
    let sugiyama_config = Config {
        minimum_length: 1,
        vertex_spacing: config.node_separation,
        dummy_vertices: false,
        dummy_size: 1.0,
        ranking_type: rust_sugiyama::configure::RankingType::MinimizeEdgeLength,
        c_minimization: rust_sugiyama::configure::CrossingMinimization::Barycenter,
        transpose: true,
    };

    let layout_result = from_vertices_and_edges(&vertices, &edges, &sugiyama_config);

    // Flatten all subgraphs into a single map of vertex -> (x, y)
    let mut positions: HashMap<u32, (f64, f64)> = HashMap::new();

    for (subgraph_layout, _width, _height) in &layout_result {
        for &(v, (x, y)) in subgraph_layout {
            positions.insert(v as u32, (x, y));
        }
    }

    // Handle case where graph is empty or layout failed
    if positions.is_empty() && !vertices.is_empty() {
        // Fallback: place nodes in a grid
        let total = vertices.len();
        let cols = ((total as f64).sqrt().ceil() as usize).max(1);
        for &(v, _) in &vertices {
            let idx = v as usize;
            let row = idx / cols;
            let col = idx % cols;
            let x = col as f64 * (config.max_node_width + config.node_separation);
            let y = row as f64 * (config.max_node_height + config.rank_separation);
            positions.insert(v, (x, y));
        }
    } else if !positions.is_empty() {
        // Post-process: ensure no overlapping nodes at the same rank
        // Group nodes by their y-coordinate (rank)
        let mut rank_groups: HashMap<i64, Vec<u32>> = HashMap::new();
        for (v, (_x, y)) in &positions {
            let rank = (*y as i64) / 100; // Group by approximate rank (100px per rank)
            rank_groups.entry(rank).or_default().push(*v);
        }

        // For each rank, sort by x and spread nodes apart if needed
        let min_spacing = config.node_separation.max(50.0); // Ensure minimum spacing
        for (_rank, node_ids) in rank_groups.iter_mut() {
            if node_ids.len() > 1 {
                // Sort by current x position
                node_ids.sort_by(|a, b| {
                    let pos_a = positions.get(a).map(|p| p.0).unwrap_or(0.0);
                    let pos_b = positions.get(b).map(|p| p.0).unwrap_or(0.0);
                    pos_a.partial_cmp(&pos_b).unwrap_or(std::cmp::Ordering::Equal)
                });

                // Track the maximum x extent used and spread if overlapping
                let mut max_extent = f64::MIN;
                for &v in node_ids.iter() {
                    if let Some((x, y)) = positions.get(&v).copied() {
                        let node_width = vertices.iter()
                            .find(|(vid, _)| *vid == v)
                            .map(|(_, (w, _))| *w)
                            .unwrap_or(120.0);

                        let min_x = max_extent + min_spacing;
                        if x < min_x {
                            // Node overlaps with previous, shift it
                            positions.insert(v, (min_x, y));
                        }
                        max_extent = positions.get(&v).unwrap().0 + node_width;
                    }
                }
            }
        }
    }

    // Step 5: Map positions back to LayoutedNode
    let mut layouted_nodes: Vec<LayoutedNode> = Vec::new();

    for &(v, (width, height)) in &vertices {
        let info = nodes_data.get(&v).expect("Node info must exist");

        let (x, y) = positions.get(&v).copied().unwrap_or((0.0, 0.0));

        // Apply direction transformation
        let (x, y) = match config.direction {
            LayoutDirection::LR => (y, x),
            LayoutDirection::RL => (y, -x),
            LayoutDirection::BT => (-x, -y),
            LayoutDirection::TB => (x, y),
        };

        // Create default ports (will be refined in T4.3)
        let ports = create_default_ports(x, y, width, height, &config.direction);

        layouted_nodes.push(LayoutedNode {
            id: info.id.clone(),
            label: info.label.clone(),
            position: Point::new(x, y),
            size: (width, height),
            ports,
            style_class: "default".to_string(),
            children: vec![],
            parent: info.parent.clone(),
            kind: info.kind.clone(),
            technology: info.technology.clone(),
            description: info.description.clone(),
            z_index: 0,
        });
    }

    // Step 6: Create LayoutedEdge for each relationship
    let node_pos_map: HashMap<&str, &LayoutedNode> = layouted_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    let mut layouted_edges: Vec<LayoutedEdge> = Vec::new();
    let mut edge_id_counter = 0usize;

    for relationship in &workspace.model.relationships {
        let source_id = relationship.source_id.as_str();
        let target_id = relationship.target_id.as_str();

        let (source_port, target_port) =
            if let (Some(source_node), Some(target_node)) =
                (node_pos_map.get(source_id), node_pos_map.get(target_id))
            {
                let sp = get_default_source_port(source_node, target_node, &config.direction);
                let tp = get_default_target_port(source_node, target_node, &config.direction);
                (sp, tp)
            } else {
                // Fallback to center points
                let sp = node_pos_map
                    .get(source_id)
                    .map(|s| s.center())
                    .unwrap_or(Point::new(0.0, 0.0));
                let tp = node_pos_map
                    .get(target_id)
                    .map(|t| t.center())
                    .unwrap_or(Point::new(0.0, 0.0));
                (sp, tp)
            };

        let edge_label = relationship
            .label
            .clone()
            .or_else(|| Some(relationship.kind.to_string()));

        layouted_edges.push(LayoutedEdge {
            id: format!("edge_{}", edge_id_counter),
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
            source_port,
            target_port,
            bend_points: vec![],
            label: edge_label,
            kind: relationship.kind.to_string(),
            style_class: "default".to_string(),
            z_index: 0,
        });
        edge_id_counter += 1;
    }

    // Step 7: Build LayoutedDiagram
    let mut diagram = LayoutedDiagram {
        nodes: layouted_nodes,
        edges: layouted_edges,
        bounds: (0.0, 0.0, 0.0, 0.0),
        config: config.clone(),
    };
    diagram.compute_bounds();

    Ok(diagram)
}

/// Internal node information for layout computation
#[derive(Debug, Clone)]
struct NodeInfo {
    id: String,
    label: String,
    kind: String,
    technology: Option<String>,
    description: Option<String>,
    parent: Option<String>,
}

/// Estimate node size based on label, kind, and config
fn estimate_node_size(label: &str, kind: &str, config: &LayoutConfig) -> (f64, f64) {
    let base_width = (label.len() as f64 * 8.0 + 40.0).max(config.min_node_width);
    let width = base_width.min(config.max_node_width);

    // Height varies by kind
    let base_height: f64 = match kind {
        "person" => 60.0,
        "system" => 80.0,
        "container" | "datastore" => 70.0,
        "component" => 60.0,
        _ => 60.0,
    };

    let height = base_height.max(config.min_node_height).min(config.max_node_height);

    (width, height)
}

/// Create default ports for a node based on layout direction
fn create_default_ports(x: f64, y: f64, width: f64, height: f64, direction: &LayoutDirection) -> Vec<Port> {
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;

    match direction {
        LayoutDirection::TB => vec![
            Port {
                side: PortSide::South,
                offset: 0.5,
                position: Point::new(center_x, y + height),
                connected_edge: None,
            },
            Port {
                side: PortSide::North,
                offset: 0.5,
                position: Point::new(center_x, y),
                connected_edge: None,
            },
        ],
        LayoutDirection::LR => vec![
            Port {
                side: PortSide::East,
                offset: 0.5,
                position: Point::new(x + width, center_y),
                connected_edge: None,
            },
            Port {
                side: PortSide::West,
                offset: 0.5,
                position: Point::new(x, center_y),
                connected_edge: None,
            },
        ],
        LayoutDirection::BT => vec![
            Port {
                side: PortSide::North,
                offset: 0.5,
                position: Point::new(center_x, y),
                connected_edge: None,
            },
            Port {
                side: PortSide::South,
                offset: 0.5,
                position: Point::new(center_x, y + height),
                connected_edge: None,
            },
        ],
        LayoutDirection::RL => vec![
            Port {
                side: PortSide::West,
                offset: 0.5,
                position: Point::new(x, center_y),
                connected_edge: None,
            },
            Port {
                side: PortSide::East,
                offset: 0.5,
                position: Point::new(x + width, center_y),
                connected_edge: None,
            },
        ],
    }
}

/// Get default source port position based on relative positions
fn get_default_source_port(source: &LayoutedNode, target: &LayoutedNode, direction: &LayoutDirection) -> Point {
    let source_center = source.center();
    let target_center = target.center();

    match direction {
        LayoutDirection::TB | LayoutDirection::BT => {
            if target_center.y > source_center.y {
                // Target is below source
                Point::new(source_center.x, source.position.y + source.size.1)
            } else {
                // Target is above source
                Point::new(source_center.x, source.position.y)
            }
        }
        LayoutDirection::LR | LayoutDirection::RL => {
            if target_center.x > source_center.x {
                // Target is to the right
                Point::new(source.position.x + source.size.0, source_center.y)
            } else {
                // Target is to the left
                Point::new(source.position.x, source_center.y)
            }
        }
    }
}

/// Get default target port position based on relative positions
fn get_default_target_port(source: &LayoutedNode, target: &LayoutedNode, direction: &LayoutDirection) -> Point {
    let source_center = source.center();
    let target_center = target.center();

    match direction {
        LayoutDirection::TB | LayoutDirection::BT => {
            if source_center.y > target_center.y {
                // Source is below target
                Point::new(target_center.x, target.position.y + target.size.1)
            } else {
                // Source is above target
                Point::new(target_center.x, target.position.y)
            }
        }
        LayoutDirection::LR | LayoutDirection::RL => {
            if source_center.x > target_center.x {
                // Source is to the right of target
                Point::new(target.position.x + target.size.0, target_center.y)
            } else {
                // Source is to the left of target
                Point::new(target.position.x, target_center.y)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_workspace() -> C4Workspace {
        let mut workspace = C4Workspace::new("TestSystem");

        // Add a person
        workspace.model.people.push(Person {
            id: ElementId::new("person-1"),
            name: "Test User".to_string(),
            description: "A test user".to_string(),
            location: crate::model::c4_types::ElementLocation::Internal,
        });

        // Add relationships
        workspace.model.relationships.push(C4Relationship::new(
            ElementId::new("person-1"),
            ElementId::new("system-1"),
            crate::model::relationships::C4RelationshipKind::Uses,
        ));

        workspace
    }

    #[test]
    fn test_compute_layout_empty_workspace() {
        let workspace = C4Workspace::new("Empty");
        let config = LayoutConfig::default();
        let result = compute_layout(&workspace, &config);

        assert!(result.is_ok());
        let diagram = result.unwrap();
        assert!(diagram.nodes.is_empty());
        assert!(diagram.edges.is_empty());
        assert_eq!(diagram.bounds, (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn test_compute_layout_with_containers() {
        let mut workspace = C4Workspace::new("TestSystem");

        // Add main system
        let system = SoftwareSystem {
            id: ElementId::new("system-1"),
            name: "Test System".to_string(),
            description: "A test system".to_string(),
            location: crate::model::c4_types::ElementLocation::Internal,
            containers: vec![
                crate::model::c4_types::Container {
                    id: ElementId::new("container-1"),
                    name: "Web App".to_string(),
                    container_type: crate::model::c4_types::ContainerType::Service,
                    technology: "React".to_string(),
                    description: "Web application".to_string(),
                    path: None,
                    components: vec![],
                },
                crate::model::c4_types::Container {
                    id: ElementId::new("container-2"),
                    name: "API".to_string(),
                    container_type: crate::model::c4_types::ContainerType::Service,
                    technology: "Rust".to_string(),
                    description: "REST API".to_string(),
                    path: None,
                    components: vec![],
                },
                crate::model::c4_types::Container {
                    id: ElementId::new("container-3"),
                    name: "Database".to_string(),
                    container_type: crate::model::c4_types::ContainerType::DataStore,
                    technology: "PostgreSQL".to_string(),
                    description: "Primary database".to_string(),
                    path: None,
                    components: vec![],
                },
            ],
        };
        workspace.model.systems.push(system);

        // Add relationship from API to Database
        workspace.model.relationships.push(C4Relationship::new(
            ElementId::new("container-2"),
            ElementId::new("container-3"),
            crate::model::relationships::C4RelationshipKind::ReadsFrom,
        ));

        let config = LayoutConfig::default();
        let result = compute_layout(&workspace, &config);

        assert!(result.is_ok());
        let diagram = result.unwrap();

        // Should have 4 nodes: 1 system + 3 containers
        assert_eq!(diagram.nodes.len(), 4);

        // Should have 1 edge
        assert_eq!(diagram.edges.len(), 1);

        // Verify no overlapping positions
        let mut overlapping = false;
        for (i, node1) in diagram.nodes.iter().enumerate() {
            for node2 in diagram.nodes.iter().skip(i + 1) {
                if nodes_overlap(node1, node2) {
                    overlapping = true;
                    break;
                }
            }
            if overlapping {
                break;
            }
        }
        assert!(
            !overlapping,
            "Nodes should not have overlapping bounding boxes"
        );
    }

    fn nodes_overlap(a: &LayoutedNode, b: &LayoutedNode) -> bool {
        let (ax, ay, aw, ah) = a.bounds();
        let (bx, by, bw, bh) = b.bounds();

        // Check if rectangles overlap (not just touch)
        ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
    }

    #[test]
    fn test_compute_layout_tb_direction() {
        let workspace = create_test_workspace();
        let config = LayoutConfig {
            direction: LayoutDirection::TB,
            ..Default::default()
        };

        let result = compute_layout(&workspace, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compute_layout_lr_direction() {
        let workspace = create_test_workspace();
        let config = LayoutConfig {
            direction: LayoutDirection::LR,
            ..Default::default()
        };

        let result = compute_layout(&workspace, &config);
        assert!(result.is_ok());

        let diagram = result.unwrap();
        // In LR direction, x and y should be swapped
        // But we just verify it produces a valid layout
        for node in &diagram.nodes {
            // Positions should be non-negative
            assert!(node.position.x >= 0.0, "Node {} has negative x", node.id);
            assert!(node.position.y >= 0.0, "Node {} has negative y", node.id);
        }
    }

    #[test]
    fn test_node_sizes_respect_config() {
        let mut workspace = C4Workspace::new("TestSystem");
        let system = SoftwareSystem {
            id: ElementId::new("system-1"),
            name: "Test System".to_string(),
            description: "A test system".to_string(),
            location: crate::model::c4_types::ElementLocation::Internal,
            containers: vec![],
        };
        workspace.model.systems.push(system);

        let config = LayoutConfig {
            min_node_width: 200.0,
            min_node_height: 100.0,
            max_node_width: 400.0,
            max_node_height: 300.0,
            ..Default::default()
        };

        let result = compute_layout(&workspace, &config);
        assert!(result.is_ok());

        let diagram = result.unwrap();
        for node in &diagram.nodes {
            let (width, height) = node.size;
            assert!(
                width >= config.min_node_width && width <= config.max_node_width,
                "Node {} width {} not in range [{}, {}]",
                node.id,
                width,
                config.min_node_width,
                config.max_node_width
            );
            assert!(
                height >= config.min_node_height && height <= config.max_node_height,
                "Node {} height {} not in range [{}, {}]",
                node.id,
                height,
                config.min_node_height,
                config.max_node_height
            );
        }
    }

    #[test]
    fn test_layout_no_overlaps() {
        let mut workspace = C4Workspace::new("TestSystem");

        // Add multiple containers to force potential overlap
        let system = SoftwareSystem {
            id: ElementId::new("system-1"),
            name: "Test System".to_string(),
            description: "A test system".to_string(),
            location: crate::model::c4_types::ElementLocation::Internal,
            containers: vec![
                crate::model::c4_types::Container {
                    id: ElementId::new("container-1"),
                    name: "Service A".to_string(),
                    container_type: crate::model::c4_types::ContainerType::Service,
                    technology: "Rust".to_string(),
                    description: "Service A".to_string(),
                    path: None,
                    components: vec![],
                },
                crate::model::c4_types::Container {
                    id: ElementId::new("container-2"),
                    name: "Service B".to_string(),
                    container_type: crate::model::c4_types::ContainerType::Service,
                    technology: "Rust".to_string(),
                    description: "Service B".to_string(),
                    path: None,
                    components: vec![],
                },
                crate::model::c4_types::Container {
                    id: ElementId::new("container-3"),
                    name: "Service C".to_string(),
                    container_type: crate::model::c4_types::ContainerType::Service,
                    technology: "Rust".to_string(),
                    description: "Service C".to_string(),
                    path: None,
                    components: vec![],
                },
                crate::model::c4_types::Container {
                    id: ElementId::new("container-4"),
                    name: "Service D".to_string(),
                    container_type: crate::model::c4_types::ContainerType::Service,
                    technology: "Rust".to_string(),
                    description: "Service D".to_string(),
                    path: None,
                    components: vec![],
                },
            ],
        };
        workspace.model.systems.push(system);

        // Add relationships to create a more complex graph
        workspace.model.relationships.push(C4Relationship::new(
            ElementId::new("container-1"),
            ElementId::new("container-2"),
            crate::model::relationships::C4RelationshipKind::Calls,
        ));
        workspace.model.relationships.push(C4Relationship::new(
            ElementId::new("container-2"),
            ElementId::new("container-3"),
            crate::model::relationships::C4RelationshipKind::Calls,
        ));
        workspace.model.relationships.push(C4Relationship::new(
            ElementId::new("container-3"),
            ElementId::new("container-4"),
            crate::model::relationships::C4RelationshipKind::Calls,
        ));

        let config = LayoutConfig::default();
        let result = compute_layout(&workspace, &config);

        assert!(result.is_ok());
        let diagram = result.unwrap();

        // For N nodes, verify no two nodes have overlapping bounding boxes
        let nodes = &diagram.nodes;
        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                let overlapping = nodes_overlap(&nodes[i], &nodes[j]);
                assert!(
                    !overlapping,
                    "Nodes {} and {} have overlapping bounding boxes",
                    nodes[i].id,
                    nodes[j].id
                );
            }
        }
    }
}