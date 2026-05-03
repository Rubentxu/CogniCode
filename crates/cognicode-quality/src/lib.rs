//! CogniCode Quality Analysis Library
//!
//! This library exposes the QualityAnalysisHandler and related types for testing.

pub mod config;
pub mod incremental;
pub mod handler;

pub use handler::{
    QualityAnalysisHandler, AnalyzeFileParams, AnalyzeProjectParams,
    CheckQualityParams,
    FileAnalysisResult, ProjectAnalysisResult, IssueResult,
    FileMetricsResult, ProjectMetricsResult,
};

pub use incremental::{
    AnalysisState, BaselineDiff, FileState, QualityBaseline, QualitySnapshot,
};
