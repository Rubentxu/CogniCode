//! Decision Trace view — shows the rationale trace for a decision artifact.
//!
//! Produces two blocks:
//! - `graph`: Mermaid flowchart LR from the rationale subgraph
//! - `markdown`: Extracted ADR content (title, status, decision section)
//!
//! Feature-gated behind `multimodal`.

use async_trait::async_trait;
use serde_json::json;

use crate::domain::views::{
    ViewContext, ViewDescriptor, ViewExecutor,
};
use crate::dto::{ContextualView, InspectableObjectType, RendererKind, ViewKind};
use crate::error::ExplorerResult;

/// Decision Trace capability — applies to DecisionArtifact.
/// Shows the rationale trace as a Mermaid graph and the ADR content as markdown.
pub struct DecisionTraceExecutor;

impl ViewDescriptor for DecisionTraceExecutor {
    fn id(&self) -> &'static str {
        "decision-trace"
    }
    fn title(&self) -> &'static str {
        "Decision Trace"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::DecisionArtifact]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::DecisionTrace
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Composite
    }
}

#[async_trait]
impl ViewExecutor for DecisionTraceExecutor {
    async fn build(&self, _ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        #[cfg(feature = "multimodal")]
        {
            // Real implementation deferred — placeholder for now
            Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: "decision-trace".to_string(),
                view_id: "decision-trace".to_string(),
            })
        }
        #[cfg(not(feature = "multimodal"))]
        {
            Err(crate::error::ExplorerError::FeatureDisabled(
                "DecisionTrace requires multimodal feature".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_to_decision_artifact_returns_true() {
        let executor = DecisionTraceExecutor;
        let applies = executor.applies_to();
        assert!(
            applies.contains(&InspectableObjectType::DecisionArtifact),
            "DecisionTraceExecutor should apply to DecisionArtifact"
        );
    }

    #[test]
    fn applies_to_non_decision_artifact_returns_false() {
        let executor = DecisionTraceExecutor;
        let applies = executor.applies_to();
        assert!(
            !applies.contains(&InspectableObjectType::Symbol),
            "DecisionTraceExecutor should not apply to Symbol"
        );
        assert!(
            !applies.contains(&InspectableObjectType::File),
            "DecisionTraceExecutor should not apply to File"
        );
    }
}
