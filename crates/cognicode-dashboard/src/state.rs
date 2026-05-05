//! Application State - types only
//!
//! These types will be used once we integrate with the actual analysis.

use serde::{Deserialize, Serialize};

/// Issue severity levels (SonarQube-style)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Blocker,
    Critical,
    Major,
    Minor,
    Info,
}

impl Severity {
    pub fn color_class(&self) -> &'static str {
        match self {
            Severity::Blocker => "severity-blocker",
            Severity::Critical => "severity-critical",
            Severity::Major => "severity-major",
            Severity::Minor => "severity-minor",
            Severity::Info => "severity-info",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Severity::Blocker => "BLOCKER",
            Severity::Critical => "CRITICAL",
            Severity::Major => "MAJOR",
            Severity::Minor => "MINOR",
            Severity::Info => "INFO",
        }
    }
}

/// Issue category
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Reliability,
    Security,
    Maintainability,
    Coverage,
    Duplicate,
    Complexity,
}

impl Category {
    pub fn label(&self) -> &'static str {
        match self {
            Category::Reliability => "Reliability",
            Category::Security => "Security",
            Category::Maintainability => "Maintainability",
            Category::Coverage => "Coverage",
            Category::Duplicate => "Duplicates",
            Category::Complexity => "Complexity",
        }
    }
}

/// A single issue found during analysis
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueResult {
    pub rule_id: String,
    pub message: String,
    pub severity: Severity,
    pub category: Category,
    pub file: String,
    pub line: usize,
    pub column: Option<usize>,
    pub end_line: Option<usize>,
    pub remediation_hint: Option<String>,
}

/// Project ratings (A-E scale)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectRatings {
    pub reliability: char,
    pub security: char,
    pub maintainability: char,
    pub coverage: char,
}

/// Technical debt summary
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TechnicalDebt {
    pub total_minutes: u64,
    pub rating: char,
    pub label: String,
}

/// Quality gate condition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateCondition {
    pub id: String,
    pub name: String,
    pub metric: String,
    pub operator: String,
    pub threshold: f64,
    pub passed: bool,
}

/// Quality gate overall status
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityGateResult {
    pub name: String,
    pub status: String, // "PASSED" or "FAILED"
    pub conditions: Vec<GateCondition>,
}
