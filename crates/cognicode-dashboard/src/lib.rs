//! CogniCode Dashboard
//!
//! Web UI for code quality analysis, built with Leptos 0.7
//! Integrates with cognicode-quality for in-process analysis.

pub mod state;
pub mod components;
pub mod pages;
pub mod api;
pub mod app;

// Re-export cognicode-quality for server use
pub use cognicode_quality;

// Re-export state types
pub use state::{
    Severity, Category, IssueResult, ProjectRatings,
    TechnicalDebt, GateCondition, QualityGateResult,
    AppState, DashboardConfig,
    AnalysisSummary, AnalysisResult, IssueFilter, IssueListResponse,
    ProjectMetricsDto, RuleProfile, QualityGateDefinition, GateConditionTemplate,
    AnalysisRequest,
};

// Re-export components
pub use components::*;

// Re-export pages
pub use pages::*;