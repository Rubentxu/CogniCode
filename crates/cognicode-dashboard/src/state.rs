//! Application State - types and reactive state management
//!
//! Contains both types for data structures and AppState for global reactive state.

use serde::{Deserialize, Serialize};

/// Dashboard configuration settings
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardConfig {
    pub project_path: String,
    pub project_name: String,
    pub rule_profile: String,
    pub include_test_files: bool,
    pub analyze_dependencies: bool,
    pub quality_gate: String,
    pub fail_build_on_gate_failure: bool,
    pub block_deployment_on_gate_failure: bool,
    pub notify_on_analysis_complete: bool,
    pub alert_on_gate_failure: bool,
    pub weekly_summary_report: bool,
    pub coverage_threshold: f64,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            project_path: String::new(),
            project_name: String::from("My Project"),
            rule_profile: String::from("sonarqube"),
            include_test_files: true,
            analyze_dependencies: true,
            quality_gate: String::from("sonarqube-way"),
            fail_build_on_gate_failure: true,
            block_deployment_on_gate_failure: false,
            notify_on_analysis_complete: true,
            alert_on_gate_failure: false,
            weekly_summary_report: true,
            coverage_threshold: 70.0,
        }
    }
}

/// Application-wide reactive state
/// Uses leptos signals for reactive updates
#[derive(Clone, Debug)]
pub struct AppState {
    /// Current project path being analyzed
    pub project_path: String,
    /// Whether an analysis is currently running
    pub is_loading: bool,
    /// Current error message, if any
    pub error: Option<String>,
    /// Last analysis timestamp
    pub last_analysis: Option<String>,
    /// Current configuration
    pub config: DashboardConfig,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            project_path: String::new(),
            is_loading: false,
            error: None,
            last_analysis: None,
            config: DashboardConfig::default(),
        }
    }
}

impl AppState {
    /// Create a new AppState with the given initial values
    pub fn new(project_path: impl Into<String>) -> Self {
        Self {
            project_path: project_path.into(),
            is_loading: false,
            error: None,
            last_analysis: None,
            config: DashboardConfig::default(),
        }
    }

    /// Clear any current error
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Set an error message
    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some(msg.into());
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    /// Update the last analysis timestamp
    pub fn update_last_analysis(&mut self) {
        self.last_analysis = Some(chrono::Utc::now().to_rfc3339());
    }
}

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
