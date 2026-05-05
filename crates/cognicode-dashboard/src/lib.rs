//! CogniCode Dashboard
//!
//! Web UI for code quality analysis, built with Leptos 0.7
//! Integrates with cognicode-quality for in-process analysis.

pub mod state;
pub mod components;
pub mod pages;

// Re-export for convenience
pub use state::{
    Severity, Category, IssueResult, ProjectRatings,
    TechnicalDebt, GateCondition, QualityGateResult,
};
pub use components::*;
pub use pages::*;
