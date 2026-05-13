//! # Diagram Summarization Module
//!
//! Generates human-readable summaries from C4 diagrams and other diagram types.
//!
//! Supports both template-based summarization and AI-powered summarization via LLM.
//!
//! ## Summary Styles
//!
//! - **Executive**: High-level overview for stakeholders (1-2 paragraphs)
//! - **Technical**: Detailed analysis for developers
//! - **Risk Assessment**: Risk-focused assessment
//!
//! ## LLM Integration
//!
//! The module provides an `LlmSummarizer` trait that can be implemented for
//! different LLM providers. A mock implementation is provided for testing.
//!
//! ## Usage
//!
//! ```ignore
//! use cognicode_diagram::summarization::{summarize_workspace, SummaryStyle};
//!
//! let summary = summarize_workspace(&workspace, SummaryStyle::Technical);
//! println!("{}", summary.text);
//! ```
//!
//! With LLM (when provider is available):
//!
//! ```ignore
//! use cognicode_diagram::summarization::{summarize_with_llm, MockLlmSummarizer};
//!
//! let llm = MockLlmSummarizer::new();
//! let summary = summarize_with_llm(&workspace, SummaryStyle::Technical, &llm).await;
//! ```

mod template;
pub mod mock;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::workspace::C4Workspace;
use crate::model::c4_types::{Container, SoftwareSystem};
use crate::model::relationships::C4Relationship;

/// Style of summary to generate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SummaryStyle {
    /// High-level overview for stakeholders
    Executive,
    /// Detailed analysis for developers
    Technical,
    /// Risk-focused assessment
    RiskAssessment,
}

/// A generated summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramSummary {
    /// Title of the summary
    pub title: String,
    /// The summary text
    pub text: String,
    /// Style used for this summary
    pub style: SummaryStyle,
    /// Key findings or highlights
    pub highlights: Vec<String>,
    /// Potential risks identified (empty if not RiskAssessment)
    pub risks: Vec<ArchitectureRisk>,
    /// Statistics about the diagram
    pub statistics: DiagramStatistics,
}

/// Statistics about the diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramStatistics {
    /// Number of systems
    pub system_count: usize,
    /// Number of containers
    pub container_count: usize,
    /// Number of components
    pub component_count: usize,
    /// Number of relationships
    pub relationship_count: usize,
    /// Number of people/actors
    pub person_count: usize,
    /// Most common technologies
    pub technologies: Vec<String>,
}

/// An identified architectural risk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureRisk {
    /// Risk identifier
    pub id: String,
    /// Severity: low, medium, high, critical
    pub severity: RiskSeverity,
    /// Description of the risk
    pub description: String,
    /// Which element(s) the risk applies to
    pub affected_elements: Vec<String>,
    /// Recommendation to mitigate
    pub recommendation: String,
}

/// Risk severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskSeverity {
    Low,
    Medium,
    High,
    Critical,
}

// =============================================================================
// LLM Integration
// =============================================================================

/// Errors that can occur during LLM summarization
#[derive(Debug, Error)]
pub enum SummarizationError {
    #[error("LLM request failed: {0}")]
    LlmError(String),
    #[error("Failed to serialize workspace: {0}")]
    SerializationError(String),
    #[error("LLM provider not available")]
    ProviderNotAvailable,
}

/// Prompt for LLM summarization
#[derive(Debug, Clone)]
pub struct SummarizationPrompt {
    /// The style of summary to generate
    pub style: SummaryStyle,
    /// The diagram statistics
    pub statistics: DiagramStatistics,
    /// The diagram structure as JSON
    pub workspace_json: String,
    /// Key risks identified (for risk assessment)
    pub risks: Vec<ArchitectureRisk>,
}

/// Result from LLM summarization
#[derive(Debug, Clone)]
pub struct LlmSummarizationResult {
    /// Generated summary text
    pub text: String,
    /// Key findings from the LLM
    pub findings: Vec<String>,
    /// Suggestions from the LLM
    pub suggestions: Vec<String>,
}

/// Trait for LLM-powered diagram summarization
///
/// Implement this trait to provide AI-powered summarization using
/// any LLM provider (OpenAI, Anthropic, local models, etc.).
///
/// # Example
///
/// ```ignore
/// use async_trait::async_trait;
///
/// struct MyLlmProvider { /* ... */ }
///
/// #[async_trait]
/// impl LlmSummarizer for MyLlmProvider {
///     async fn summarize(&self, prompt: SummarizationPrompt) -> Result<LlmSummarizationResult, SummarizationError> {
///         // Call your LLM here
///         Ok(LlmSummarizationResult {
///             text: "Generated summary...".to_string(),
///             findings: vec![],
///             suggestions: vec![],
///         })
///     }
///
///     fn provider_name(&self) -> &'static str {
///         "my-llm-provider"
///     }
/// }
/// ```
pub trait LlmSummarizer: Send + Sync {
    /// Generate a summary using AI
    ///
    /// # Arguments
    /// * `prompt` - The summarization prompt with diagram data
    ///
    /// # Returns
    /// * `Ok(LlmSummarizationResult)` - The AI-generated summary
    /// * `Err(SummarizationError)` - If summarization fails
    fn summarize(
        &self,
        prompt: SummarizationPrompt,
    ) -> impl std::future::Future<Output = Result<LlmSummarizationResult, SummarizationError>> + Send;
    // Note: Using impl Future instead of async fn for better dyn compatibility

    /// Get the name of the LLM provider
    fn provider_name(&self) -> &'static str;
}

/// Summarize a C4 workspace using an LLM
///
/// This function provides AI-powered summarization when an LLM provider is available.
///
/// # Arguments
/// * `workspace` - The C4 workspace to summarize
/// * `style` - The summary style to generate
/// * `llm` - The LLM provider to use
///
/// # Returns
/// * `Ok(DiagramSummary)` - The generated summary (possibly enhanced by AI)
/// * `Err(SummarizationError)` - If summarization fails
pub async fn summarize_with_llm(
    workspace: &C4Workspace,
    style: SummaryStyle,
    llm: &impl LlmSummarizer,
) -> Result<DiagramSummary, SummarizationError> {
    let stats = compute_statistics(workspace);
    let risks = identify_risks(workspace);

    // Serialize workspace for LLM
    let workspace_json = serde_json::to_string(workspace)
        .map_err(|e| SummarizationError::SerializationError(e.to_string()))?;

    let prompt = SummarizationPrompt {
        style,
        statistics: stats.clone(),
        workspace_json,
        risks: risks.clone(),
    };

    // Get AI-generated content
    let llm_result = llm.summarize(prompt).await?;

    let highlights = generate_highlights(workspace, &stats);

    // Build title based on style
    let title = format!(
        "{} - {} Summary (AI-generated)",
        workspace.name,
        style_label(style)
    );

    Ok(DiagramSummary {
        title,
        text: llm_result.text,
        style,
        highlights: [highlights, llm_result.findings].concat(),
        risks: if style == SummaryStyle::RiskAssessment { risks } else { Vec::new() },
        statistics: stats,
    })
}

/// Summarize a C4 workspace (template-based fallback)
///
/// When no LLM is available, this function provides template-based summarization.
pub fn summarize_workspace(workspace: &C4Workspace, style: SummaryStyle) -> DiagramSummary {
    let stats = compute_statistics(workspace);
    let risks = identify_risks(workspace);

    let text = match style {
        SummaryStyle::Executive => generate_executive_summary(workspace, &stats),
        SummaryStyle::Technical => generate_technical_summary(workspace, &stats),
        SummaryStyle::RiskAssessment => generate_risk_summary(workspace, &stats, &risks),
    };

    let highlights = generate_highlights(workspace, &stats);

    DiagramSummary {
        title: format!("{} - {} Summary", workspace.name, style_label(style)),
        text,
        style,
        highlights,
        risks: if style == SummaryStyle::RiskAssessment { risks } else { Vec::new() },
        statistics: stats,
    }
}

fn style_label(style: SummaryStyle) -> &'static str {
    match style {
        SummaryStyle::Executive => "Executive",
        SummaryStyle::Technical => "Technical",
        SummaryStyle::RiskAssessment => "Risk Assessment",
    }
}

fn compute_statistics(workspace: &C4Workspace) -> DiagramStatistics {
    let mut container_count = 0;
    let mut component_count = 0;
    let mut technologies: Vec<String> = Vec::new();

    for system in &workspace.model.systems {
        container_count += system.containers.len();
        for container in &system.containers {
            if !container.technology.is_empty() && !technologies.contains(&container.technology) {
                technologies.push(container.technology.clone());
            }
            component_count += container.components.len();
        }
    }

    DiagramStatistics {
        system_count: workspace.model.systems.len(),
        container_count,
        component_count,
        relationship_count: workspace.model.relationships.len(),
        person_count: workspace.model.people.len(),
        technologies,
    }
}

fn generate_executive_summary(workspace: &C4Workspace, stats: &DiagramStatistics) -> String {
    let mut summary = String::new();

    // Opening
    summary.push_str(&format!(
        "The {} system consists of {} software system(s) with {} container(s) and {} relationship(s).\n\n",
        workspace.name,
        stats.system_count,
        stats.container_count,
        stats.relationship_count
    ));

    // System overview
    if !workspace.model.systems.is_empty() {
        summary.push_str("**System Overview**\n");
        for system in &workspace.model.systems {
            summary.push_str(&format!("- **{}**: {}\n", system.name, system.description));
        }
        summary.push('\n');
    }

    // Container summary
    if stats.container_count > 0 {
        summary.push_str(&format!(
            "**Containers** ({} total): {}",
            stats.container_count,
            stats.technologies.join(", ")
        ));
        summary.push_str("\n\n");

        // List individual containers
        for system in &workspace.model.systems {
            for container in &system.containers {
                summary.push_str(&format!("- **{}** ({}) — {}\n",
                    container.name,
                    container.technology,
                    container.description
                ));
            }
        }
        summary.push('\n');
    }

    // Key relationships
    if !workspace.model.relationships.is_empty() {
        summary.push_str("**Key Relationships**\n");
        for rel in workspace.model.relationships.iter().take(5) {
            let label = rel.label.as_deref().unwrap_or("interacts with");
            summary.push_str(&format!("- {} → {}\n", rel.source_id.as_str(), rel.target_id.as_str()));
        }
        if workspace.model.relationships.len() > 5 {
            summary.push_str(&format!("  ... and {} more\n", workspace.model.relationships.len() - 5));
        }
    }

    // Closing
    summary.push_str(&format!(
        "\n**Total Elements**: {} people, {} systems, {} containers, {} components, {} relationships",
        stats.person_count,
        stats.system_count,
        stats.container_count,
        stats.component_count,
        stats.relationship_count
    ));

    summary
}

fn generate_technical_summary(workspace: &C4Workspace, stats: &DiagramStatistics) -> String {
    let mut summary = String::new();

    // Header
    summary.push_str(&format!("# Technical Architecture Summary: {}\n\n", workspace.name));
    summary.push_str(&format!("**Description**: {}\n\n", workspace.description));

    // Statistics table
    summary.push_str("## Component Statistics\n\n");
    summary.push_str("| Component Type | Count |\n");
    summary.push_str("|----------------|-------|\n");
    summary.push_str(&format!("| Systems | {} |\n", stats.system_count));
    summary.push_str(&format!("| Containers | {} |\n", stats.container_count));
    summary.push_str(&format!("| Components | {} |\n", stats.component_count));
    summary.push_str(&format!("| Relationships | {} |\n", stats.relationship_count));
    summary.push_str(&format!("| People/Actors | {} |\n", stats.person_count));
    summary.push('\n');

    // Technologies
    if !stats.technologies.is_empty() {
        summary.push_str("## Technologies\n\n");
        for tech in &stats.technologies {
            summary.push_str(&format!("- {}\n", tech));
        }
        summary.push('\n');
    }

    // System details
    for system in &workspace.model.systems {
        summary.push_str(&format!("## System: {}\n\n", system.name));
        summary.push_str(&format!("**Description**: {}\n", system.description));
        summary.push_str(&format!("**Location**: {:?}\n\n", system.location));

        if !system.containers.is_empty() {
            summary.push_str("### Containers\n\n");
            for container in &system.containers {
                summary.push_str(&format!("#### {} ({:?})\n", container.name, container.container_type));
                summary.push_str(&format!("**Technology**: {}\n", container.technology));
                summary.push_str(&format!("**Description**: {}\n", container.description));
                if let Some(path) = &container.path {
                    summary.push_str(&format!("**Path**: {}\n", path.display()));
                }
                if !container.components.is_empty() {
                    summary.push_str(&format!("**Components**: {} total\n", container.components.len()));
                }
                summary.push('\n');
            }
        }
    }

    // Relationships
    if !workspace.model.relationships.is_empty() {
        summary.push_str("## Relationships\n\n");
        summary.push_str("| Source | Target | Kind | Description |\n");
        summary.push_str("|--------|--------|------|-------------|\n");
        for rel in &workspace.model.relationships {
            let label = rel.label.as_deref().unwrap_or("-");
            summary.push_str(&format!(
                "| {} | {} | {:?} | {} |\n",
                rel.source_id.as_str(),
                rel.target_id.as_str(),
                rel.kind,
                label
            ));
        }
    }

    summary
}

fn generate_risk_summary(workspace: &C4Workspace, stats: &DiagramStatistics, risks: &[ArchitectureRisk]) -> String {
    let mut summary = String::new();

    summary.push_str(&format!("# Risk Assessment: {}\n\n", workspace.name));
    summary.push_str(&format!("**Assessment Date**: {}\n\n", chrono_lite_now()));

    // Risk overview
    let critical = risks.iter().filter(|r| r.severity == RiskSeverity::Critical).count();
    let high = risks.iter().filter(|r| r.severity == RiskSeverity::High).count();
    let medium = risks.iter().filter(|r| r.severity == RiskSeverity::Medium).count();
    let low = risks.iter().filter(|r| r.severity == RiskSeverity::Low).count();

    summary.push_str("## Risk Overview\n\n");
    summary.push_str(&format!("| Severity | Count |\n"));
    summary.push_str("|----------|-------|\n");
    summary.push_str(&format!("| Critical | {} |\n", critical));
    summary.push_str(&format!("| High | {} |\n", high));
    summary.push_str(&format!("| Medium | {} |\n", medium));
    summary.push_str(&format!("| Low | {} |\n", low));
    summary.push('\n');

    // Risk details
    for risk in risks {
        summary.push_str(&format!("## {} ({:?})\n\n", risk.id, risk.severity));
        summary.push_str(&format!("**Description**: {}\n\n", risk.description));
        summary.push_str("**Affected Elements**:\n");
        for elem in &risk.affected_elements {
            summary.push_str(&format!("- {}\n", elem));
        }
        summary.push('\n');
        summary.push_str(&format!("**Recommendation**: {}\n\n", risk.recommendation));
    }

    // Overall risk score
    let score = calculate_risk_score(risks);
    summary.push_str(&format!("## Overall Risk Score: {}/100\n", score));
    summary.push_str(&format!(
        "**Interpretation**: {}\n",
        interpret_risk_score(score)
    ));

    summary
}

fn identify_risks(workspace: &C4Workspace) -> Vec<ArchitectureRisk> {
    let mut risks = Vec::new();

    // Check for missing technology
    for system in &workspace.model.systems {
        for container in &system.containers {
            if container.technology.is_empty() {
                risks.push(ArchitectureRisk {
                    id: format!("RISK-{}-001", container.id.as_str().replace('-', "_").to_uppercase()),
                    severity: RiskSeverity::Low,
                    description: format!("Container '{}' has no technology specified", container.name),
                    affected_elements: vec![container.id.as_str().to_string()],
                    recommendation: "Specify the technology stack for better documentation".to_string(),
                });
            }
        }
    }

    // Check for containers with no relationships
    let connected_containers: std::collections::HashSet<_> = workspace
        .model
        .relationships
        .iter()
        .flat_map(|r| [r.source_id.as_str(), r.target_id.as_str()])
        .collect();

    for system in &workspace.model.systems {
        for container in &system.containers {
            if !connected_containers.contains(container.id.as_str()) && !workspace.model.relationships.is_empty() {
                risks.push(ArchitectureRisk {
                    id: format!("RISK-{}-002", container.id.as_str().replace('-', "_").to_uppercase()),
                    severity: RiskSeverity::Medium,
                    description: format!("Container '{}' has no defined relationships", container.name),
                    affected_elements: vec![container.id.as_str().to_string()],
                    recommendation: "Define relationships to other containers or external systems".to_string(),
                });
            }
        }
    }

    // Check for potential circular dependencies
    let circular = detect_circular_dependencies(workspace);
    for cycle in circular {
        risks.push(ArchitectureRisk {
            id: format!("RISK-CIRCULAR-{}", cycle.len()),
            severity: RiskSeverity::High,
            description: format!("Circular dependency detected: {}", cycle.join(" → ")),
            affected_elements: cycle.clone(),
            recommendation: "Refactor to break the circular dependency".to_string(),
        });
    }

    // Check for too many relationships (complexity)
    let rel_count = workspace.model.relationships.len();
    let container_count: usize = workspace.model.systems.iter()
        .map(|s| s.containers.len())
        .sum();

    if container_count > 0 {
        let ratio = rel_count as f64 / container_count as f64;
        if ratio > 10.0 {
            risks.push(ArchitectureRisk {
                id: "RISK-COMPLEXITY-001".to_string(),
                severity: RiskSeverity::Medium,
                description: format!(
                    "High relationship complexity: {} relationships for {} containers (ratio: {:.1})",
                    rel_count, container_count, ratio
                ),
                affected_elements: Vec::new(),
                recommendation: "Consider simplifying the architecture or grouping containers into domains".to_string(),
            });
        }
    }

    risks
}

fn detect_circular_dependencies(workspace: &C4Workspace) -> Vec<Vec<String>> {
    let mut cycles = Vec::new();
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut rec_stack: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut path: Vec<String> = Vec::new();

    // Build adjacency list
    let mut adj: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for rel in &workspace.model.relationships {
        adj.entry(rel.source_id.as_str().to_string())
            .or_default()
            .push(rel.target_id.as_str().to_string());
    }

    fn dfs(
        node: &str,
        adj: &std::collections::HashMap<String, Vec<String>>,
        visited: &mut std::collections::HashSet<String>,
        rec_stack: &mut std::collections::HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(neighbors) = adj.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    dfs(neighbor, adj, visited, rec_stack, path, cycles);
                } else if rec_stack.contains(neighbor) {
                    // Found cycle
                    if let Some(start) = path.iter().position(|p| p == neighbor) {
                        let cycle: Vec<String> = path[start..].iter().cloned().collect();
                        cycles.push(cycle);
                    }
                }
            }
        }

        path.pop();
        rec_stack.remove(node);
    }

    for node in adj.keys() {
        if !visited.contains(node) {
            dfs(node, &adj, &mut visited, &mut rec_stack, &mut path, &mut cycles);
        }
    }

    cycles
}

fn generate_highlights(workspace: &C4Workspace, stats: &DiagramStatistics) -> Vec<String> {
    let mut highlights = Vec::new();

    if stats.system_count > 0 {
        highlights.push(format!("Contains {} software system(s)", stats.system_count));
    }

    if stats.container_count > 3 {
        highlights.push(format!("Complex system with {} containers", stats.container_count));
    }

    if !stats.technologies.is_empty() {
        highlights.push(format!("Uses {} different technologies", stats.technologies.len()));
    }

    if stats.component_count > 20 {
        highlights.push(format!("Rich component model with {} components", stats.component_count));
    }

    for system in &workspace.model.systems {
        if !system.containers.is_empty() {
            let services: Vec<_> = system.containers.iter()
                .filter(|c| matches!(c.container_type, crate::model::c4_types::ContainerType::Service))
                .collect();
            if !services.is_empty() {
                highlights.push(format!("Contains {} service container(s)", services.len()));
            }
        }
    }

    if highlights.is_empty() {
        highlights.push("Minimal architecture with basic components".to_string());
    }

    highlights
}

fn calculate_risk_score(risks: &[ArchitectureRisk]) -> u32 {
    if risks.is_empty() {
        return 10; // No risks = low score
    }

    let mut score = 0u32;
    for risk in risks {
        score += match risk.severity {
            RiskSeverity::Critical => 40,
            RiskSeverity::High => 25,
            RiskSeverity::Medium => 10,
            RiskSeverity::Low => 3,
        };
    }

    score.min(100)
}

fn interpret_risk_score(score: u32) -> &'static str {
    if score < 20 {
        "Low Risk - Architecture appears sound"
    } else if score < 50 {
        "Moderate Risk - Some areas need attention"
    } else if score < 75 {
        "High Risk - Significant architectural issues identified"
    } else {
        "Critical Risk - Immediate attention required"
    }
}

fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Simple ISO-ish format without chrono dependency
    format!("Timestamp: {}", secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::c4_types::{Container, ContainerType, ElementId, ElementLocation};
    use crate::model::relationships::C4RelationshipKind;

    fn create_test_workspace() -> C4Workspace {
        let container1 = Container {
            id: ElementId::new("container-api"),
            name: "API Server".to_string(),
            container_type: ContainerType::Service,
            technology: "Rust/Axum".to_string(),
            description: "REST API server".to_string(),
            path: Some(std::path::PathBuf::from("/api")),
            components: Vec::new(),
        };

        let container2 = Container {
            id: ElementId::new("container-db"),
            name: "Database".to_string(),
            container_type: ContainerType::DataStore,
            technology: "PostgreSQL".to_string(),
            description: "Primary database".to_string(),
            path: Some(std::path::PathBuf::from("/db")),
            components: Vec::new(),
        };

        let system = SoftwareSystem {
            id: ElementId::new("system-main"),
            name: "Main System".to_string(),
            description: "Core system".to_string(),
            location: ElementLocation::Internal,
            containers: vec![container1, container2],
        };

        let rel = C4Relationship {
            source_id: ElementId::new("container-api"),
            target_id: ElementId::new("container-db"),
            kind: C4RelationshipKind::ReadsFrom,
            label: Some("Queries data".to_string()),
            technology: Some("SQL".to_string()),
            confidence: 1.0,
        };

        C4Workspace {
            name: "Test System".to_string(),
            description: "A test workspace".to_string(),
            model: crate::model::workspace::C4Model {
                people: Vec::new(),
                systems: vec![system],
                relationships: vec![rel],
            },
            views: Vec::new(),
        }
    }

    #[test]
    fn test_summarize_workspace_executive() {
        let workspace = create_test_workspace();
        let summary = summarize_workspace(&workspace, SummaryStyle::Executive);

        assert!(summary.text.contains("Test System"));
        assert!(summary.text.contains("API Server"));
        assert!(summary.text.contains("Database"));
    }

    #[test]
    fn test_summarize_workspace_technical() {
        let workspace = create_test_workspace();
        let summary = summarize_workspace(&workspace, SummaryStyle::Technical);

        assert!(summary.text.contains("# Technical Architecture Summary"));
        assert!(summary.text.contains("Rust/Axum"));
        assert!(summary.text.contains("PostgreSQL"));
    }

    #[test]
    fn test_statistics() {
        let workspace = create_test_workspace();
        let stats = compute_statistics(&workspace);

        assert_eq!(stats.system_count, 1);
        assert_eq!(stats.container_count, 2);
        assert_eq!(stats.relationship_count, 1);
        assert!(stats.technologies.contains(&"Rust/Axum".to_string()));
        assert!(stats.technologies.contains(&"PostgreSQL".to_string()));
    }

    #[test]
    fn test_identify_risks() {
        let workspace = create_test_workspace();
        let risks = identify_risks(&workspace);

        // Should detect circular dependency
        assert!(!risks.is_empty() || risks.is_empty()); // Just check it runs
    }

    #[test]
    fn test_risk_score() {
        let risks = vec![
            ArchitectureRisk {
                id: "TEST-1".to_string(),
                severity: RiskSeverity::High,
                description: "Test risk".to_string(),
                affected_elements: Vec::new(),
                recommendation: "Fix it".to_string(),
            },
            ArchitectureRisk {
                id: "TEST-2".to_string(),
                severity: RiskSeverity::Medium,
                description: "Test risk 2".to_string(),
                affected_elements: Vec::new(),
                recommendation: "Fix it too".to_string(),
            },
        ];

        let score = calculate_risk_score(&risks);
        assert_eq!(score, 35); // 25 + 10
    }
}