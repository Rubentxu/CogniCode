//! Services module
//!
//! Domain services that encapsulate business logic.

mod call_graph_analyzer;
mod complexity;
mod cycle_detector;
mod impact_analyzer;

pub use call_graph_analyzer::{
    CallGraphAnalyzer, CallGraphComplexityReport, EntryPointAnalysis, HotPath, LeafFunctionAnalysis,
};
pub use complexity::{
    CFGNode, CFGNodeType, ComplexityCalculator, ComplexityReport, ComplexityRisk, DecisionPoint,
};
pub use cycle_detector::{Cycle, CycleDetectionResult, CycleDetector};
pub use impact_analyzer::{ImpactAnalyzer, ImpactLevel, ImpactReport, ImpactThreshold};
