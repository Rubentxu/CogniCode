//! CogniCode Dashboard
//!
//! Web UI for code quality analysis, built with Leptos 0.8
//! Integrates with cognicode-quality for in-process analysis.

#![recursion_limit = "256"]

pub mod state;
pub mod components;
pub mod pages;
pub mod api;
pub mod api_client;
pub mod app;

// Re-export cognicode-quality for server use
#[cfg(feature = "server")]
pub use cognicode_quality;

// Re-export state types
pub use state::{
    Severity, Category, IssueResult, ProjectRatings,
    TechnicalDebt, GateCondition, QualityGateResult,
    AppState, DashboardConfig,
    AnalysisSummary, AnalysisResult, IssueFilter, IssueListResponse,
    RuleProfile, QualityGateDefinition, GateConditionTemplate,
    AnalysisRequest,
    ReactiveAppState,
};

// Re-export API client types
pub use api_client::{
    ApiClient, AnalysisSummaryDto, IssueDto, DashboardConfigDto,
    ProjectRatingsDto, TechnicalDebtDto,
    GateConditionDto, QualityGateResultDto, IncrementalInfoDto,
    PathValidationDto,
};

// Re-export components
pub use components::*;

// Re-export pages
pub use pages::*;