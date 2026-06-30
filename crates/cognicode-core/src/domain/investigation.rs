//! Investigation domain entity — ADR-005 Phase INV-1.
//!
//! An Investigation is a focused exploration session that tracks evidence,
//! artifacts, and narrative as the user investigates a code intelligence question.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Snapshot of a single pane's state at save time (ADR-040 Wave 3).
/// `pane_id` is the frontend-generated id; `object_id` and `view_id`
/// are the resolved identifiers. `scroll_y` and `viewport` carry the
/// UI state for restore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneSnapshot {
    pub pane_id: String,
    pub object_id: String,
    pub view_id: String,
    pub scroll_y: f32,
    pub viewport: Option<ViewportState>,
}

/// Viewport state for a pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportState {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

/// Status of an investigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Draft,
    Active,
    Completed,
    Archived,
}

impl Status {
    /// Parse a status from its string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "draft" => Some(Status::Draft),
            "active" => Some(Status::Active),
            "completed" => Some(Status::Completed),
            "archived" => Some(Status::Archived),
            _ => None,
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Draft => write!(f, "draft"),
            Status::Active => write!(f, "active"),
            Status::Completed => write!(f, "completed"),
            Status::Archived => write!(f, "archived"),
        }
    }
}

/// Evidence item pinned to an investigation.
///
/// An evidence item is a reference to a code object (symbol, file, etc.)
/// that the user has marked as relevant to the investigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Evidence {
    /// Unique identifier for this evidence item.
    pub id: String,
    /// The object id this evidence references (e.g. `symbol:path:name:line`).
    pub object_id: String,
    /// Optional view id when the evidence was captured from a specific view.
    pub view_id: Option<String>,
    /// User-authored note explaining why this evidence is relevant.
    pub note: String,
    /// When this evidence was pinned.
    pub pinned_at: OffsetDateTime,
}

/// Artifact attached to an investigation.
///
/// An artifact is generated content: Mermaid diagrams, draw.io exports,
/// SVG snapshots, or markdown notes produced during the investigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Artifact {
    /// Unique identifier for this artifact.
    pub id: String,
    /// Kind of artifact (e.g. "mermaid", "svg", "markdown", "drawio").
    pub kind: String,
    /// Human-readable title for this artifact.
    pub title: String,
    /// The generated content.
    pub content: String,
    /// Optional reference to the object/view that generated this artifact.
    pub generated_from: Option<String>,
}

/// Investigation aggregate root.
///
/// A focused exploration session that tracks evidence, artifacts, and narrative
/// as the user investigates a code intelligence question.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Investigation {
    /// Unique identifier for this investigation.
    pub id: String,
    /// Workspace this investigation belongs to.
    pub workspace_id: String,
    /// Human-readable title.
    pub title: String,
    /// The goal or question this investigation aims to answer.
    pub goal: String,
    /// Current lifecycle status.
    pub status: Status,
    /// Optional entry point that initiated this investigation.
    pub entry_point: Option<String>,
    /// Pane snapshots for restore (ADR-040 Wave 3).
    pub panes: Vec<PaneSnapshot>,
    /// Evidence items pinned during this investigation.
    pub evidence: Vec<Evidence>,
    /// Artifacts generated during this investigation.
    pub artifacts: Vec<Artifact>,
    /// User-authored narrative documenting the investigation process.
    pub narrative: String,
    /// ADR identifiers related to this investigation.
    pub related_adrs: Vec<String>,
    /// When this investigation was created.
    pub created_at: OffsetDateTime,
    /// When this investigation was last updated.
    pub updated_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_from_str() {
        assert_eq!(Status::from_str("draft"), Some(Status::Draft));
        assert_eq!(Status::from_str("active"), Some(Status::Active));
        assert_eq!(Status::from_str("completed"), Some(Status::Completed));
        assert_eq!(Status::from_str("archived"), Some(Status::Archived));
        assert_eq!(Status::from_str("DRAFT"), Some(Status::Draft));
        assert_eq!(Status::from_str("unknown"), None);
    }

    #[test]
    fn test_status_to_string() {
        assert_eq!(Status::Draft.to_string(), "draft");
        assert_eq!(Status::Active.to_string(), "active");
        assert_eq!(Status::Completed.to_string(), "completed");
        assert_eq!(Status::Archived.to_string(), "archived");
    }

    #[test]
    fn test_investigation_serde() {
        let investigation = Investigation {
            id: "inv-001".to_string(),
            workspace_id: "ws-001".to_string(),
            title: "Test Investigation".to_string(),
            goal: "What is this code doing?".to_string(),
            status: Status::Active,
            entry_point: Some("symbol:main.rs:main:1".to_string()),
            panes: vec![],
            evidence: vec![],
            artifacts: vec![],
            narrative: "Initial hypothesis...".to_string(),
            related_adrs: vec!["ADR-005".to_string()],
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };

        let json = serde_json::to_string(&investigation).unwrap();
        let deser: Investigation = serde_json::from_str(&json).unwrap();

        assert_eq!(deser.id, investigation.id);
        assert_eq!(deser.status, investigation.status);
        assert_eq!(deser.title, investigation.title);
    }
}
