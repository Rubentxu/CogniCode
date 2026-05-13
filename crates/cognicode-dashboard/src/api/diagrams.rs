//! Diagram API — client-side functions
//!
//! HTTP client for calling diagram generation endpoints.

use serde::{Deserialize, Serialize};

/// Request to generate a diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateDiagramRequest {
    pub project_path: String,
    /// "c4", "sequence", "state_machine", "activity", "multi_lang"
    pub diagram_type: String,
    /// "context", "container", "component", "code"
    pub level: Option<String>,
    /// Entry symbol for sequence/activity/state_machine
    pub entry_symbol: Option<String>,
    /// Output format: "mermaid" or "json"
    pub format: Option<String>,
}

/// Response from diagram generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateDiagramResponse {
    pub diagram_type: String,
    pub mermaid_code: String,
    pub workspace_json: Option<String>,
    pub element_count: usize,
    pub relationship_count: usize,
    pub cached: bool,
}

/// Cached diagram info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedDiagramDto {
    pub key: String,
    pub project_path: String,
    pub diagram_type: String,
    pub element_count: usize,
    pub age_secs: u64,
}

/// List diagrams response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDiagramsResponse {
    pub diagrams: Vec<CachedDiagramDto>,
}

/// Generate a diagram via the server API
pub async fn generate_diagram(request: GenerateDiagramRequest) -> Result<GenerateDiagramResponse, String> {
    let resp = gloo_net::http::Request::post("/api/diagrams/generate")
        .json(&request)
        .map_err(|e| format!("Failed to serialize request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Failed to call diagram API: {}", e))?;

    if !resp.ok() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Diagram API error ({}): {}", status, body));
    }

    resp.json::<GenerateDiagramResponse>()
        .await
        .map_err(|e| format!("Failed to parse diagram response: {}", e))
}

/// List cached diagrams
pub async fn list_diagrams() -> Result<ListDiagramsResponse, String> {
    let resp = gloo_net::http::Request::get("/api/diagrams")
        .send()
        .await
        .map_err(|e| format!("Failed to list diagrams: {}", e))?;

    if !resp.ok() {
        return Err(format!("List diagrams error: {}", resp.status()));
    }

    resp.json::<ListDiagramsResponse>()
        .await
        .map_err(|e| format!("Failed to parse diagrams list: {}", e))
}

/// Diagram type options for the selector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagramType {
    C4,
    Sequence,
    StateMachine,
    Activity,
    MultiLang,
}

impl DiagramType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiagramType::C4 => "c4",
            DiagramType::Sequence => "sequence",
            DiagramType::StateMachine => "state_machine",
            DiagramType::Activity => "activity",
            DiagramType::MultiLang => "multi_lang",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DiagramType::C4 => "C4 Architecture",
            DiagramType::Sequence => "Sequence Diagram",
            DiagramType::StateMachine => "State Machine",
            DiagramType::Activity => "Activity Diagram",
            DiagramType::MultiLang => "Multi-Language Workspace",
        }
    }

    pub fn all() -> &'static [DiagramType] {
        &[
            DiagramType::C4,
            DiagramType::Sequence,
            DiagramType::StateMachine,
            DiagramType::Activity,
            DiagramType::MultiLang,
        ]
    }
}

/// C4 level options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum C4Level {
    Context,
    Container,
    Component,
    Code,
}

impl C4Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            C4Level::Context => "context",
            C4Level::Container => "container",
            C4Level::Component => "component",
            C4Level::Code => "code",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            C4Level::Context => "System Context (L1)",
            C4Level::Container => "Container (L2)",
            C4Level::Component => "Component (L3)",
            C4Level::Code => "Code (L4)",
        }
    }

    pub fn all() -> &'static [C4Level] {
        &[C4Level::Context, C4Level::Container, C4Level::Component, C4Level::Code]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Diff types — match the server's DiffDiagramsOutput structure
// ─────────────────────────────────────────────────────────────────────────────

/// Request to diff two diagram workspaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffDiagramRequest {
    /// Serialized C4Workspace JSON for diagram A (get via generate_diagram with format="json")
    pub workspace_a_json: String,
    /// Serialized C4Workspace JSON for diagram B (get via generate_diagram with format="json")
    pub workspace_b_json: String,
    /// Output format: "mermaid" (default) or "json"
    pub format: Option<String>,
}

/// Summary counts from a diagram diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummaryDto {
    pub systems_added: usize,
    pub systems_removed: usize,
    pub containers_added: usize,
    pub containers_removed: usize,
    pub containers_modified: usize,
    pub relationships_added: usize,
    pub relationships_removed: usize,
    pub total_changes: usize,
}

/// A container that was added or removed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSummaryDto {
    pub id: String,
    pub name: String,
    pub technology: String,
    pub description: String,
}

/// A container that was modified (shows before/after)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerDiffDto {
    pub id: String,
    pub name: String,
    pub before_technology: Option<String>,
    pub after_technology: Option<String>,
    pub before_description: Option<String>,
    pub after_description: Option<String>,
}

/// Response from the diff endpoint — matches server's DiffDiagramsOutput
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffDiagramResponse {
    /// The diff output (Mermaid diagram code or JSON)
    pub diff_output: String,
    /// Format used ("mermaid" or "json")
    pub format: String,
    /// Summary of all changes
    pub summary: DiffSummaryDto,
    /// Containers that were added
    pub containers_added: Vec<ContainerSummaryDto>,
    /// Containers that were removed
    pub containers_removed: Vec<ContainerSummaryDto>,
    /// Containers that were modified
    pub containers_modified: Vec<ContainerDiffDto>,
    /// Count of relationships added
    pub relationships_added_count: usize,
    /// Count of relationships removed
    pub relationships_removed_count: usize,
}

/// Compare two diagram workspaces via the server API
pub async fn diff_diagrams(request: DiffDiagramRequest) -> Result<DiffDiagramResponse, String> {
    let resp = gloo_net::http::Request::post("/api/diagrams/diff")
        .json(&request)
        .map_err(|e| format!("Failed to serialize request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Failed to call diff API: {}", e))?;

    if !resp.ok() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Diff API error ({}): {}", status, body));
    }

    resp.json::<DiffDiagramResponse>()
        .await
        .map_err(|e| format!("Failed to parse diff response: {}", e))
}
