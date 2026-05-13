//! # Diff Rendering Module
//!
//! Renders workspace diffs in various formats: Mermaid, JSON, etc.

use serde::Serialize;

use crate::diff::{
    ContainerDiff, ElementDiff, WorkspaceDiff,
};

/// Output format for diff rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffFormat {
    /// Mermaid state diagram with highlighting
    MermaidState,
    /// Mermaid class diagram with highlighting
    MermaidClass,
    /// Structured JSON changeset
    Json,
}

impl Default for DiffFormat {
    fn default() -> Self {
        Self::MermaidState
    }
}

impl From<&str> for DiffFormat {
    fn from(s: &str) -> Self {
        match s {
            "mermaid" | "mermaid_state" => Self::MermaidState,
            "mermaid_class" => Self::MermaidClass,
            "json" => Self::Json,
            _ => Self::MermaidState,
        }
    }
}

/// JSON representation of a diff for structured output
#[derive(Debug, Clone, Serialize)]
pub struct DiffJsonOutput {
    pub summary: DiffSummaryJson,
    pub systems: SystemsDiffJson,
    pub containers: ContainersDiffJson,
    pub relationships: RelationshipsDiffJson,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffSummaryJson {
    pub systems_added: usize,
    pub systems_removed: usize,
    pub containers_added: usize,
    pub containers_removed: usize,
    pub containers_modified: usize,
    pub relationships_added: usize,
    pub relationships_removed: usize,
    pub total_changes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemsDiffJson {
    pub added: Vec<SystemSummaryJson>,
    pub removed: Vec<SystemSummaryJson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemSummaryJson {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContainersDiffJson {
    pub added: Vec<ContainerSummaryJson>,
    pub removed: Vec<ContainerSummaryJson>,
    pub modified: Vec<ContainerDiffJson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContainerSummaryJson {
    pub id: String,
    pub name: String,
    pub technology: String,
    pub description: String,
    pub container_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContainerDiffJson {
    pub id: String,
    pub changes: Vec<AttributeChangeJson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttributeChangeJson {
    pub attribute: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub change_type: String, // "added", "removed", "modified", "unchanged"
}

#[derive(Debug, Clone, Serialize)]
pub struct RelationshipsDiffJson {
    pub added: Vec<RelationshipSummaryJson>,
    pub removed: Vec<RelationshipSummaryJson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelationshipSummaryJson {
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
    pub label: Option<String>,
    pub technology: Option<String>,
}

impl From<&crate::diff::DiffSummary> for DiffSummaryJson {
    fn from(summary: &crate::diff::DiffSummary) -> Self {
        Self {
            systems_added: summary.systems_added,
            systems_removed: summary.systems_removed,
            containers_added: summary.containers_added,
            containers_removed: summary.containers_removed,
            containers_modified: summary.containers_modified,
            relationships_added: summary.relationships_added,
            relationships_removed: summary.relationships_removed,
            total_changes: summary.total_changes,
        }
    }
}

/// Render a diff to JSON format
pub fn render_diff_json(diff: &WorkspaceDiff) -> DiffJsonOutput {
    let summary = DiffSummaryJson::from(&diff.summary);

    let systems = SystemsDiffJson {
        added: diff.systems_added.iter().map(|s| SystemSummaryJson {
            id: s.id.as_str().to_string(),
            name: s.name.clone(),
            description: s.description.clone(),
        }).collect(),
        removed: diff.systems_removed.iter().map(|s| SystemSummaryJson {
            id: s.id.as_str().to_string(),
            name: s.name.clone(),
            description: s.description.clone(),
        }).collect(),
    };

    let containers = ContainersDiffJson {
        added: diff.containers_added.iter().map(|c| ContainerSummaryJson {
            id: c.id.as_str().to_string(),
            name: c.name.clone(),
            technology: c.technology.clone(),
            description: c.description.clone(),
            container_type: format!("{:?}", c.container_type),
        }).collect(),
        removed: diff.containers_removed.iter().map(|c| ContainerSummaryJson {
            id: c.id.as_str().to_string(),
            name: c.name.clone(),
            technology: c.technology.clone(),
            description: c.description.clone(),
            container_type: format!("{:?}", c.container_type),
        }).collect(),
        modified: diff.containers_modified.iter().map(|c| {
            let changes = build_container_changes(c);
            ContainerDiffJson {
                id: c.id.clone(),
                changes,
            }
        }).collect(),
    };

    let relationships = RelationshipsDiffJson {
        added: diff.relationships_added.iter().map(|r| RelationshipSummaryJson {
            source_id: r.source_id.as_str().to_string(),
            target_id: r.target_id.as_str().to_string(),
            kind: format!("{:?}", r.kind),
            label: r.label.clone(),
            technology: r.technology.clone(),
        }).collect(),
        removed: diff.relationships_removed.iter().map(|r| RelationshipSummaryJson {
            source_id: r.source_id.as_str().to_string(),
            target_id: r.target_id.as_str().to_string(),
            kind: format!("{:?}", r.kind),
            label: r.label.clone(),
            technology: r.technology.clone(),
        }).collect(),
    };

    DiffJsonOutput {
        summary,
        systems,
        containers,
        relationships,
    }
}

fn build_container_changes(container_diff: &ContainerDiff) -> Vec<AttributeChangeJson> {
    let mut changes = Vec::new();

    changes.push(build_change("name", &container_diff.name_diff));
    changes.push(build_change("technology", &container_diff.technology_diff));
    changes.push(build_change("description", &container_diff.description_diff));
    changes.push(build_change("container_type", &container_diff.container_type_diff));

    changes
}

fn build_change(attribute: &str, diff: &ElementDiff<String>) -> AttributeChangeJson {
    match diff {
        ElementDiff::Unchanged(v) => AttributeChangeJson {
            attribute: attribute.to_string(),
            before: Some(v.clone()),
            after: Some(v.clone()),
            change_type: "unchanged".to_string(),
        },
        ElementDiff::Modified { before, after } => AttributeChangeJson {
            attribute: attribute.to_string(),
            before: Some(before.clone()),
            after: Some(after.clone()),
            change_type: "modified".to_string(),
        },
        ElementDiff::Added(v) => AttributeChangeJson {
            attribute: attribute.to_string(),
            before: None,
            after: Some(v.clone()),
            change_type: "added".to_string(),
        },
        ElementDiff::Removed(v) => AttributeChangeJson {
            attribute: attribute.to_string(),
            before: Some(v.clone()),
            after: None,
            change_type: "removed".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff_workspaces;
    use crate::model::c4_types::{Container, ContainerType, ElementId, ElementLocation, SoftwareSystem};
    use crate::model::relationships::{C4Relationship, C4RelationshipKind};
    use crate::model::workspace::C4Workspace;

    fn create_test_workspace_a() -> C4Workspace {
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

    fn create_test_workspace_b() -> C4Workspace {
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
    fn test_render_diff_json() {
        let before = create_test_workspace_a();
        let after = create_test_workspace_b();
        let diff = diff_workspaces(&before, &after);

        let json_output = render_diff_json(&diff);

        assert_eq!(json_output.summary.total_changes, 3);
        assert_eq!(json_output.summary.containers_added, 1);
        assert_eq!(json_output.summary.containers_modified, 1);
        assert_eq!(json_output.summary.relationships_removed, 1);

        // Check container changes
        assert_eq!(json_output.containers.added.len(), 1);
        assert_eq!(json_output.containers.added[0].name, "Web UI");

        assert_eq!(json_output.containers.modified.len(), 1);
        assert_eq!(json_output.containers.modified[0].id, "container-api");

        // Check that technology change is detected
        let tech_change = json_output.containers.modified[0].changes.iter()
            .find(|c| c.attribute == "technology")
            .expect("Should have technology change");
        assert_eq!(tech_change.change_type, "modified");
        assert_eq!(tech_change.before, Some("Rust".to_string()));
        assert_eq!(tech_change.after, Some("Rust/Axum".to_string()));
    }

    #[test]
    fn test_diff_format_parsing() {
        assert_eq!(DiffFormat::from("mermaid"), DiffFormat::MermaidState);
        assert_eq!(DiffFormat::from("mermaid_state"), DiffFormat::MermaidState);
        assert_eq!(DiffFormat::from("mermaid_class"), DiffFormat::MermaidClass);
        assert_eq!(DiffFormat::from("json"), DiffFormat::Json);
        assert_eq!(DiffFormat::from("unknown"), DiffFormat::MermaidState); // default
    }
}
