//! API Server Functions module
//!
//! Provides async functions that call cognicode-quality for analysis.
//! These are used by the dashboard to fetch analysis data.

pub mod analysis;
pub mod issues;
pub mod quality_gate;
pub mod configuration;

pub use analysis::{
    AnalysisRequest, AnalysisResult, AnalysisSummary,
    get_analysis_summary, run_analysis,
};
pub use issues::{
    IssueFilter, IssueListResponse,
    get_issues, get_issue, get_issue_counts,
};
pub use quality_gate::{
    QualityGateDefinition, GateConditionTemplate,
    get_quality_gates, evaluate_quality_gate,
};
pub use configuration::{
    RuleProfile,
    get_rule_profiles, get_configuration, save_configuration, validate_project_path,
};
