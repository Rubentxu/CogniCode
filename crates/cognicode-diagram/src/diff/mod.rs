//! # Diagram Diff Module
//!
//! Computes and renders differences between two C4 workspaces.
//!
//! ## Usage
//!
//! ```ignore
//! use cognicode_diagram::diff::{diff_workspaces, render_diff_mermaid};
//!
//! let diff = diff_workspaces(&workspace_a, &workspace_b);
//! let mermaid = render_diff_mermaid(&diff, "mermaid");
//! ```

pub mod render;

use serde::{Deserialize, Serialize};

use crate::model::c4_types::{Container, SoftwareSystem};
use crate::model::relationships::C4Relationship;
use crate::model::workspace::C4Workspace;

/// Represents a difference between two diagram elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElementDiff<T> {
    /// Element exists in both (maybe with changes)
    Unchanged(T),
    /// Element exists only in the first workspace (removed)
    Removed(T),
    /// Element exists only in the second workspace (added)
    Added(T),
    /// Element exists in both but with changes
    Modified { before: T, after: T },
}

impl<T> ElementDiff<T> {
    pub fn is_added(&self) -> bool {
        matches!(self, ElementDiff::Added(_))
    }

    pub fn is_removed(&self) -> bool {
        matches!(self, ElementDiff::Removed(_))
    }

    pub fn is_modified(&self) -> bool {
        matches!(self, ElementDiff::Modified { .. })
    }

    pub fn is_unchanged(&self) -> bool {
        matches!(self, ElementDiff::Unchanged(_))
    }

    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> ElementDiff<U> {
        match self {
            ElementDiff::Unchanged(v) => ElementDiff::Unchanged(f(v)),
            ElementDiff::Removed(v) => ElementDiff::Removed(f(v)),
            ElementDiff::Added(v) => ElementDiff::Added(f(v)),
            ElementDiff::Modified { before, after } => ElementDiff::Modified {
                before: f(before),
                after: f(after),
            },
        }
    }
}

/// A container diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerDiff {
    pub id: String,
    pub name_diff: ElementDiff<String>,
    pub technology_diff: ElementDiff<String>,
    pub description_diff: ElementDiff<String>,
    pub container_type_diff: ElementDiff<String>,
    pub component_count_before: usize,
    pub component_count_after: usize,
}

/// A relationship diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipDiff {
    pub source_id: String,
    pub target_id: String,
    pub kind_diff: ElementDiff<String>,
    pub label_diff: ElementDiff<Option<String>>,
    pub technology_diff: ElementDiff<Option<String>>,
}

/// The complete diff between two workspaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDiff {
    /// Systems that were added
    pub systems_added: Vec<SoftwareSystem>,
    /// Systems that were removed
    pub systems_removed: Vec<SoftwareSystem>,
    /// Containers that were added
    pub containers_added: Vec<Container>,
    /// Containers that were removed
    pub containers_removed: Vec<Container>,
    /// Containers that were modified
    pub containers_modified: Vec<ContainerDiff>,
    /// Relationships that were added
    pub relationships_added: Vec<C4Relationship>,
    /// Relationships that were removed
    pub relationships_removed: Vec<C4Relationship>,
    /// Summary counts
    pub summary: DiffSummary,
}

/// Summary of changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub systems_added: usize,
    pub systems_removed: usize,
    pub containers_added: usize,
    pub containers_removed: usize,
    pub containers_modified: usize,
    pub relationships_added: usize,
    pub relationships_removed: usize,
    pub total_changes: usize,
}

impl DiffSummary {
    pub fn new() -> Self {
        Self {
            systems_added: 0,
            systems_removed: 0,
            containers_added: 0,
            containers_removed: 0,
            containers_modified: 0,
            relationships_added: 0,
            relationships_removed: 0,
            total_changes: 0,
        }
    }

    pub fn compute_total(&mut self) {
        self.total_changes = self.systems_added
            + self.systems_removed
            + self.containers_added
            + self.containers_removed
            + self.containers_modified
            + self.relationships_added
            + self.relationships_removed;
    }
}

impl Default for DiffSummary {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the diff between two workspaces
pub fn diff_workspaces(before: &C4Workspace, after: &C4Workspace) -> WorkspaceDiff {
    let mut diff = WorkspaceDiff {
        systems_added: Vec::new(),
        systems_removed: Vec::new(),
        containers_added: Vec::new(),
        containers_removed: Vec::new(),
        containers_modified: Vec::new(),
        relationships_added: Vec::new(),
        relationships_removed: Vec::new(),
        summary: DiffSummary::new(),
    };

    // Build sets of before/after elements
    let before_systems: std::collections::HashMap<_, _> = before
        .model
        .systems
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();

    let after_systems: std::collections::HashMap<_, _> = after
        .model
        .systems
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();

    let before_containers: std::collections::HashMap<_, _> = before
        .model
        .systems
        .iter()
        .flat_map(|s| s.containers.iter().map(|c| (c.id.as_str(), c)))
        .collect();

    let after_containers: std::collections::HashMap<_, _> = after
        .model
        .systems
        .iter()
        .flat_map(|s| s.containers.iter().map(|c| (c.id.as_str(), c)))
        .collect();

    let before_rels: std::collections::HashSet<_> = before
        .model
        .relationships
        .iter()
        .map(|r| format!("{}->{}", r.source_id.as_str(), r.target_id.as_str()))
        .collect();

    let after_rels: std::collections::HashSet<_> = after
        .model
        .relationships
        .iter()
        .map(|r| format!("{}->{}", r.source_id.as_str(), r.target_id.as_str()))
        .collect();

    // Find systems removed (in before but not in after)
    for (id, system) in &before_systems {
        if !after_systems.contains_key(id) {
            diff.systems_removed.push((*system).clone());
            diff.summary.systems_removed += 1;
        }
    }

    // Find systems added (in after but not in before)
    for (id, system) in &after_systems {
        if !before_systems.contains_key(id) {
            diff.systems_added.push((*system).clone());
            diff.summary.systems_added += 1;
        }
    }

    // Find containers removed and modified
    for (id, container) in &before_containers {
        if let Some(after_container) = after_containers.get(id) {
            // Check if modified
            if container.name != after_container.name
                || container.technology != after_container.technology
                || container.description != after_container.description
            {
                diff.containers_modified.push(ContainerDiff {
                    id: id.to_string(),
                    name_diff: diff_element(&container.name, &after_container.name),
                    technology_diff: diff_element(&container.technology, &after_container.technology),
                    description_diff: diff_element(&container.description, &after_container.description),
                    container_type_diff: diff_element(
                        &format!("{:?}", container.container_type),
                        &format!("{:?}", after_container.container_type),
                    ),
                    component_count_before: container.components.len(),
                    component_count_after: after_container.components.len(),
                });
                diff.summary.containers_modified += 1;
            }
        } else {
            diff.containers_removed.push((*container).clone());
            diff.summary.containers_removed += 1;
        }
    }

    // Find containers added
    for (id, container) in &after_containers {
        if !before_containers.contains_key(id) {
            diff.containers_added.push((*container).clone());
            diff.summary.containers_added += 1;
        }
    }

    // Find relationships removed
    for rel in &before.model.relationships {
        let key = format!("{}->{}", rel.source_id.as_str(), rel.target_id.as_str());
        if !after_rels.contains(&key) {
            diff.relationships_removed.push(rel.clone());
            diff.summary.relationships_removed += 1;
        }
    }

    // Find relationships added
    for rel in &after.model.relationships {
        let key = format!("{}->{}", rel.source_id.as_str(), rel.target_id.as_str());
        if !before_rels.contains(&key) {
            diff.relationships_added.push(rel.clone());
            diff.summary.relationships_added += 1;
        }
    }

    diff.summary.compute_total();

    diff
}

/// Diff two string elements
fn diff_element<T: Clone + PartialEq>(before: &T, after: &T) -> ElementDiff<T> {
    if before == after {
        ElementDiff::Unchanged(before.clone())
    } else {
        ElementDiff::Modified {
            before: before.clone(),
            after: after.clone(),
        }
    }
}

/// Render diff as Mermaid with highlighting
pub fn render_diff_mermaid(diff: &WorkspaceDiff, view_type: &str) -> String {
    match view_type {
        "mermaid" | "mermaid_state" => render_mermaid_state_diff(diff),
        "mermaid_class" => render_mermaid_class_diff(diff),
        _ => render_mermaid_state_diff(diff),
    }
}

/// Render diff as structured JSON
pub fn render_diff_json(diff: &WorkspaceDiff) -> String {
    let json_output = render::render_diff_json(diff);
    serde_json::to_string_pretty(&json_output)
        .unwrap_or_else(|_| r#"{"error": "Failed to serialize diff"}"#.to_string())
}

fn render_mermaid_state_diff(diff: &WorkspaceDiff) -> String {
    let mut output = String::new();

    output.push_str("%% { \n");
    output.push_str("%%   title: Diagram Diff\n");
    output.push_str(&format!("%%   systems_added: {}\n", diff.summary.systems_added));
    output.push_str(&format!("%%   systems_removed: {}\n", diff.summary.systems_removed));
    output.push_str(&format!("%%   containers_added: {}\n", diff.summary.containers_added));
    output.push_str(&format!("%%   containers_removed: {}\n", diff.summary.containers_removed));
    output.push_str(&format!("%%   containers_modified: {}\n", diff.summary.containers_modified));
    output.push_str(&format!("%%   relationships_added: {}\n", diff.summary.relationships_added));
    output.push_str(&format!("%%   relationships_removed: {}\n", diff.summary.relationships_removed));
    output.push_str("%% }\n\n");

    output.push_str("stateDiagram-v2\n");
    output.push_str("    direction TB\n\n");

    // Add styling
    output.push_str("    classDef added fill:#90EE90,stroke:#228B22,stroke-width:2px\n");
    output.push_str("    classDef removed fill:#FFB6C1,stroke:#DC143C,stroke-width:2px,stroke-dasharray:5 5\n");
    output.push_str("    classDef modified fill:#FFFACD,stroke:#DAA520,stroke-width:2px\n\n");

    // Containers added
    for container in &diff.containers_added {
        output.push_str(&format!(
            "    [\"+ {} ({})\"]:::added\n",
            escape_mermaid(&container.name),
            escape_mermaid(&container.technology)
        ));
    }

    // Containers removed
    for container in &diff.containers_removed {
        output.push_str(&format!(
            "    [\"- {} ({})\"]:::removed\n",
            escape_mermaid(&container.name),
            escape_mermaid(&container.technology)
        ));
    }

    // Containers modified
    for container_diff in &diff.containers_modified {
        output.push_str(&format!(
            "    [\"~ {} ({})\"]:::modified\n",
            escape_mermaid(&get_modified_value(&container_diff.name_diff)),
            escape_mermaid(&get_modified_value(&container_diff.technology_diff))
        ));
    }

    // Relationships added
    for rel in &diff.relationships_added {
        output.push_str(&format!(
            "    {} --> {} : + {} ({})\n",
            rel.source_id.as_str(),
            rel.target_id.as_str(),
            escape_mermaid(rel.label.as_deref().unwrap_or("")),
            escape_mermaid(rel.technology.as_deref().unwrap_or(""))
        ));
    }

    // Relationships removed
    for rel in &diff.relationships_removed {
        output.push_str(&format!(
            "    {} -.- {} : - {} ({})\n",
            rel.source_id.as_str(),
            rel.target_id.as_str(),
            escape_mermaid(rel.label.as_deref().unwrap_or("")),
            escape_mermaid(rel.technology.as_deref().unwrap_or(""))
        ));
    }

    output
}

fn render_mermaid_class_diff(diff: &WorkspaceDiff) -> String {
    let mut output = String::new();

    output.push_str("classDiagram\n\n");

    // Add styling
    output.push_str("    classDef added stroke:#228B22,stroke-width:2px,bgColor:#90EE90\n");
    output.push_str("    classDef removed stroke:#DC143C,stroke-width:2px,stroke-dasharray:5 5,bgColor:#FFB6C1\n");
    output.push_str("    classDef modified stroke:#DAA520,stroke-width:2px,bgColor:#FFFACD\n\n");

    // Containers
    for container in &diff.containers_added {
        output.push_str(&format!(
            "    class {} ~added~\n",
            escape_mermaid(&container.name)
        ));
    }

    for container in &diff.containers_removed {
        output.push_str(&format!(
            "    class {} ~removed~\n",
            escape_mermaid(&container.name)
        ));
    }

    for container_diff in &diff.containers_modified {
        output.push_str(&format!(
            "    class {} ~modified~\n",
            escape_mermaid(&get_modified_value(&container_diff.name_diff))
        ));
    }

    output
}

fn escape_mermaid(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', " ")
}

fn get_modified_value<T: Clone + PartialEq>(diff: &ElementDiff<T>) -> String
where
    T: std::fmt::Display,
{
    match diff {
        ElementDiff::Unchanged(v) => format!("{}", v),
        ElementDiff::Modified { after, .. } => format!("{}", after),
        ElementDiff::Added(v) => format!("+{}", v),
        ElementDiff::Removed(v) => format!("-{}", v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::c4_types::{Container, ContainerType, ElementId, ElementLocation};
    use crate::model::relationships::C4RelationshipKind;

    fn create_workspace_a() -> C4Workspace {
        let container = Container {
            id: ElementId::new("container-api"),
            name: "API Server".to_string(),
            container_type: ContainerType::Service,
            technology: "Rust".to_string(),
            description: "REST API".to_string(),
            path: Some(std::path::PathBuf::from("/api")),
            components: Vec::new(),
        };

        let system = SoftwareSystem {
            id: ElementId::new("system-main"),
            name: "Main System".to_string(),
            description: "Core system".to_string(),
            location: ElementLocation::Internal,
            containers: vec![container],
        };

        let rel = C4Relationship {
            source_id: ElementId::new("container-api"),
            target_id: ElementId::new("container-db"),
            kind: C4RelationshipKind::ReadsFrom,
            label: Some("Queries".to_string()),
            technology: Some("SQL".to_string()),
            confidence: 1.0,
        };

        C4Workspace {
            name: "System A".to_string(),
            description: "Original".to_string(),
            model: crate::model::workspace::C4Model {
                people: Vec::new(),
                systems: vec![system],
                relationships: vec![rel],
            },
            views: Vec::new(),
        }
    }

    fn create_workspace_b() -> C4Workspace {
        // B adds a container and changes the API server technology
        let container_modified = Container {
            id: ElementId::new("container-api"),
            name: "API Server".to_string(),
            container_type: ContainerType::Service,
            technology: "Rust/Axum".to_string(), // Changed
            description: "REST API".to_string(),
            path: Some(std::path::PathBuf::from("/api")),
            components: Vec::new(),
        };

        let container_added = Container {
            id: ElementId::new("container-web"),
            name: "Web UI".to_string(),
            container_type: ContainerType::Service,
            technology: "Leptos".to_string(),
            description: "Web interface".to_string(),
            path: Some(std::path::PathBuf::from("/web")),
            components: Vec::new(),
        };

        let system = SoftwareSystem {
            id: ElementId::new("system-main"),
            name: "Main System".to_string(),
            description: "Core system".to_string(),
            location: ElementLocation::Internal,
            containers: vec![container_modified, container_added],
        };

        C4Workspace {
            name: "System B".to_string(),
            description: "Modified".to_string(),
            model: crate::model::workspace::C4Model {
                people: Vec::new(),
                systems: vec![system],
                relationships: Vec::new(),
            },
            views: Vec::new(),
        }
    }

    #[test]
    fn test_diff_workspaces() {
        let before = create_workspace_a();
        let after = create_workspace_b();

        let diff = diff_workspaces(&before, &after);

        assert_eq!(diff.summary.containers_added, 1);
        assert_eq!(diff.summary.containers_removed, 0);
        assert_eq!(diff.summary.containers_modified, 1);
        assert_eq!(diff.summary.relationships_added, 0);
        assert_eq!(diff.summary.relationships_removed, 1); // The relationship to db
    }

    #[test]
    fn test_render_diff_mermaid() {
        let before = create_workspace_a();
        let after = create_workspace_b();

        let diff = diff_workspaces(&before, &after);
        let mermaid = render_diff_mermaid(&diff, "mermaid");

        assert!(mermaid.contains("Web UI")); // Added container
        assert!(mermaid.contains("container-api")); // Modified container
        assert!(mermaid.contains("added")); // CSS class
        assert!(mermaid.contains("modified")); // CSS class
    }

    #[test]
    fn test_diff_summary() {
        let before = create_workspace_a();
        let after = create_workspace_b();

        let diff = diff_workspaces(&before, &after);

        assert!(diff.summary.total_changes > 0);
        assert_eq!(
            diff.summary.total_changes,
            diff.summary.containers_added
                + diff.summary.containers_removed
                + diff.summary.containers_modified
                + diff.summary.relationships_added
                + diff.summary.relationships_removed
        );
    }
}