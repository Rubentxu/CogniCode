//! CogniCode Quality Analysis Library
//!
//! This library exposes the QualityAnalysisHandler and related types for testing.

pub mod config;
pub mod handler;

pub use handler::{
    QualityAnalysisHandler, AnalyzeFileParams, AnalyzeProjectParams,
    FileAnalysisResult, ProjectAnalysisResult, IssueResult,
    FileMetricsResult, ProjectMetricsResult,
};
