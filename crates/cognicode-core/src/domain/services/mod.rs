//! Services module
//!
//! Domain services that encapsulate business logic.

mod call_graph_analyzer;
mod complexity;
mod confidence_rules;
#[cfg(feature = "multimodal")]
mod corroboration;
mod cycle_detector;
mod edge_attachment;
mod impact_analyzer;
mod target_resolution;

pub use call_graph_analyzer::{
    CallGraphAnalyzer, CallGraphComplexityReport, EntryPointAnalysis, HotPath, LeafFunctionAnalysis,
};
pub use complexity::{
    CFGNode, CFGNodeType, ComplexityCalculator, ComplexityReport, ComplexityRisk, DecisionPoint,
};
pub use confidence_rules::{ConfidenceError, ConfidenceRules, ExtractionContext};
#[cfg(feature = "multimodal")]
pub use corroboration::{edge_score, provenance_weight, score_subgraph, target_score};
pub use cycle_detector::{Cycle, CycleDetectionResult, CycleDetector};
pub use edge_attachment::attach_relations;
pub use impact_analyzer::{ImpactAnalyzer, ImpactLevel, ImpactReport, ImpactThreshold};
pub use target_resolution::resolve_target;
