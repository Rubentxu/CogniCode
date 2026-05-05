//! API Server Functions module
//!
//! Provides async functions that call the dashboard server API.
//! These are used by the dashboard to fetch analysis data.

pub mod analysis;
pub mod issues;
pub mod quality_gate;
pub mod configuration;

pub use crate::state::{
    AnalysisRequest, IssueFilter, IssueListResponse,
    QualityGateDefinition, GateConditionTemplate, RuleProfile,
};

pub use analysis::{run_analysis, get_analysis_summary};
pub use issues::{get_issues, get_issue, get_issue_counts};
pub use quality_gate::{get_quality_gates, evaluate_quality_gate};
pub use configuration::{get_rule_profiles, get_configuration, save_configuration, validate_project_path};