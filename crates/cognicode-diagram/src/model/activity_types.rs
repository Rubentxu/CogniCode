//! Activity diagram model types
//!
//! Represents activity diagrams (control flow) as an intermediate format agnostic to output.
//! Used for inference (from functions) and rendering (to Mermaid, PlantUML).

use serde::{Deserialize, Serialize};

/// An activity diagram showing control flow through a function or process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityModel {
    /// Diagram title
    pub title: String,
    /// Entry point symbol/function name
    pub entry_point: String,
    /// All nodes in the activity
    pub nodes: Vec<ActivityNode>,
    /// All edges/flows in the activity
    pub edges: Vec<ActivityEdge>,
    /// Metadata
    pub metadata: ActivityMetadata,
}

/// Metadata about the activity diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityMetadata {
    /// Number of action nodes
    pub action_count: usize,
    /// Number of decision/merge nodes
    pub decision_count: usize,
    /// Number of fork/join nodes
    pub parallel_count: usize,
    /// Maximum nesting depth of loops
    pub loop_depth: usize,
    /// Whether the diagram has parallel regions
    pub has_parallel: bool,
}

/// A node in the activity diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityNode {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Node type
    pub node_type: ActivityNodeType,
    /// Guard condition (for decision nodes)
    pub guard: Option<String>,
    /// Loop variable (for loop nodes)
    pub loop_variable: Option<String>,
    /// Source location if available
    pub location: Option<String>,
}

/// Type of activity node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActivityNodeType {
    /// Initial node (filled circle)
    Initial,
    /// Final node (double circle)
    Final,
    /// Action node (rounded rectangle)
    Action,
    /// Decision/choice node (diamond)
    Decision,
    /// Merge node (diamond, joins branches)
    Merge,
    /// Fork node (horizontal line, parallel start)
    Fork,
    /// Join node (horizontal line, parallel end)
    Join,
    /// Loop node (represents for/while)
    Loop,
    /// Call to another activity
    Call,
}

impl Default for ActivityNodeType {
    fn default() -> Self {
        ActivityNodeType::Action
    }
}

impl ActivityNodeType {
    /// Infer node type from name
    pub fn from_name(name: &str) -> Self {
        let name_lower = name.to_lowercase();
        if name_lower.contains("start") || name_lower.contains("begin") || name_lower == "entry" {
            ActivityNodeType::Initial
        } else if name_lower.contains("end") || name_lower.contains("finish") || name_lower.contains("exit") || name_lower == "return" {
            ActivityNodeType::Final
        } else if name_lower.contains("loop") || name_lower.contains("for") || name_lower.contains("while") || name_lower.contains("iterate") {
            ActivityNodeType::Loop
        } else if name_lower.contains("fork") || name_lower.contains("parallel") || name_lower.contains("async") {
            ActivityNodeType::Fork
        } else if name_lower.contains("join") || name_lower.contains("sync") {
            ActivityNodeType::Join
        } else if name_lower.contains("if") || name_lower.contains("else") || name_lower.contains("match") || name_lower.contains("switch") || name_lower.contains("?") {
            ActivityNodeType::Decision
        } else if name_lower.contains("merge") || name_lower.contains("alt") || name_lower.contains("opt") {
            ActivityNodeType::Merge
        } else {
            ActivityNodeType::Action
        }
    }
}

/// An edge/flow between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEdge {
    /// Unique identifier
    pub id: String,
    /// Source node ID
    pub from: String,
    /// Target node ID
    pub to: String,
    /// Label (condition for decision edges)
    pub label: Option<String>,
    /// Guard condition
    pub guard: Option<String>,
}

impl ActivityModel {
    /// Create a new empty activity model
    pub fn new(title: impl Into<String>, entry_point: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            entry_point: entry_point.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
            metadata: ActivityMetadata {
                action_count: 0,
                decision_count: 0,
                parallel_count: 0,
                loop_depth: 0,
                has_parallel: false,
            },
        }
    }

    /// Add a node
    pub fn add_node(&mut self, node: ActivityNode) {
        if !self.nodes.iter().any(|n| n.id == node.id) {
            self.nodes.push(node);
        }
    }

    /// Add an edge
    pub fn add_edge(&mut self, edge: ActivityEdge) {
        self.edges.push(edge);
    }

    /// Get node by ID
    pub fn get_node(&self, id: &str) -> Option<&ActivityNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Finalize the model (compute metadata)
    pub fn finalize(&mut self) {
        let action_count = self.nodes.iter()
            .filter(|n| n.node_type == ActivityNodeType::Action || n.node_type == ActivityNodeType::Call)
            .count();
        let decision_count = self.nodes.iter()
            .filter(|n| n.node_type == ActivityNodeType::Decision || n.node_type == ActivityNodeType::Merge)
            .count();
        let parallel_count = self.nodes.iter()
            .filter(|n| n.node_type == ActivityNodeType::Fork || n.node_type == ActivityNodeType::Join)
            .count();
        let has_parallel = parallel_count > 0;
        let loop_depth = self.compute_loop_depth();

        self.metadata = ActivityMetadata {
            action_count,
            decision_count,
            parallel_count,
            loop_depth,
            has_parallel,
        };
    }

    /// Compute maximum loop nesting depth
    fn compute_loop_depth(&self) -> usize {
        let mut max_depth = 0;
        for node in &self.nodes {
            if node.node_type == ActivityNodeType::Loop {
                // For simplicity, loops contribute 1 to depth
                // In a real implementation, we'd track actual nesting
                max_depth = max_depth.max(1);
            }
        }
        max_depth
    }

    /// Get initial node
    pub fn initial_node(&self) -> Option<&ActivityNode> {
        self.nodes.iter().find(|n| n.node_type == ActivityNodeType::Initial)
    }

    /// Get final nodes
    pub fn final_nodes(&self) -> Vec<&ActivityNode> {
        self.nodes.iter().filter(|n| n.node_type == ActivityNodeType::Final).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_model_new() {
        let model = ActivityModel::new("Test Process", "process_user");
        assert_eq!(model.title, "Test Process");
        assert_eq!(model.entry_point, "process_user");
        assert!(model.nodes.is_empty());
        assert!(model.edges.is_empty());
    }

    #[test]
    fn test_add_node() {
        let mut model = ActivityModel::new("Test", "main");
        model.add_node(ActivityNode {
            id: "start".to_string(),
            name: "Start".to_string(),
            node_type: ActivityNodeType::Initial,
            guard: None,
            loop_variable: None,
            location: None,
        });
        assert_eq!(model.nodes.len(), 1);
    }

    #[test]
    fn test_add_duplicate_node() {
        let mut model = ActivityModel::new("Test", "main");
        model.add_node(ActivityNode {
            id: "start".to_string(),
            name: "Start".to_string(),
            node_type: ActivityNodeType::Initial,
            guard: None,
            loop_variable: None,
            location: None,
        });
        model.add_node(ActivityNode {
            id: "start".to_string(),
            name: "Start2".to_string(),
            node_type: ActivityNodeType::Action,
            guard: None,
            loop_variable: None,
            location: None,
        });
        assert_eq!(model.nodes.len(), 1);
    }

    #[test]
    fn test_finalize() {
        let mut model = ActivityModel::new("Test", "main");
        model.add_node(ActivityNode {
            id: "start".to_string(),
            name: "Start".to_string(),
            node_type: ActivityNodeType::Initial,
            guard: None,
            loop_variable: None,
            location: None,
        });
        model.add_node(ActivityNode {
            id: "action1".to_string(),
            name: "Do Something".to_string(),
            node_type: ActivityNodeType::Action,
            guard: None,
            loop_variable: None,
            location: None,
        });
        model.add_node(ActivityNode {
            id: "end".to_string(),
            name: "End".to_string(),
            node_type: ActivityNodeType::Final,
            guard: None,
            loop_variable: None,
            location: None,
        });
        model.finalize();

        assert_eq!(model.metadata.action_count, 1);
        assert_eq!(model.metadata.decision_count, 0);
    }

    #[test]
    fn test_activity_node_type_inference() {
        assert_eq!(ActivityNodeType::from_name("start"), ActivityNodeType::Initial);
        assert_eq!(ActivityNodeType::from_name("end_process"), ActivityNodeType::Final);
        assert_eq!(ActivityNodeType::from_name("for_each_item"), ActivityNodeType::Loop);
        assert_eq!(ActivityNodeType::from_name("if_valid"), ActivityNodeType::Decision);
        assert_eq!(ActivityNodeType::from_name("simple_action"), ActivityNodeType::Action);
    }

    #[test]
    fn test_initial_and_final_nodes() {
        let mut model = ActivityModel::new("Test", "main");
        model.add_node(ActivityNode {
            id: "start".to_string(),
            name: "Start".to_string(),
            node_type: ActivityNodeType::Initial,
            guard: None,
            loop_variable: None,
            location: None,
        });
        model.add_node(ActivityNode {
            id: "end".to_string(),
            name: "End".to_string(),
            node_type: ActivityNodeType::Final,
            guard: None,
            loop_variable: None,
            location: None,
        });

        assert!(model.initial_node().is_some());
        assert_eq!(model.final_nodes().len(), 1);
    }
}
