//! Application State - types and reactive state management
//!
//! Contains both types for data structures and AppState for global reactive state.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::api_client::{
    ApiClient, AnalysisSummaryDto, IssueDto, DashboardConfigDto,
};

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

/// Analysis request for the server
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisRequest {
    pub project_path: String,
    pub quick: Option<bool>,
}

/// Application-wide reactive state
/// Uses leptos signals for reactive updates
#[derive(Clone, Debug)]
pub struct AppState {
    /// Current project path being analyzed
    pub project_path: RwSignal<String>,
    /// Whether an analysis is currently running
    pub is_loading: RwSignal<bool>,
    /// Current error message, if any
    pub error: RwSignal<Option<String>>,
    /// Last analysis timestamp
    pub last_analysis: RwSignal<Option<String>>,
    /// Current configuration
    pub config: RwSignal<DashboardConfig>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Create a new AppState with default values
    pub fn new() -> Self {
        Self {
            project_path: RwSignal::new(String::new()),
            is_loading: RwSignal::new(false),
            error: RwSignal::new(None),
            last_analysis: RwSignal::new(None),
            config: RwSignal::new(DashboardConfig::default()),
        }
    }

    /// Create a new AppState with the given project path
    pub fn with_project(project_path: impl Into<String>) -> Self {
        let mut config = DashboardConfig::default();
        config.project_path = project_path.into();
        Self {
            project_path: RwSignal::new(config.project_path.clone()),
            is_loading: RwSignal::new(false),
            error: RwSignal::new(None),
            last_analysis: RwSignal::new(None),
            config: RwSignal::new(config),
        }
    }

    /// Clear any current error
    pub fn clear_error(&self) {
        self.error.set(None);
    }

    /// Set an error message
    pub fn set_error(&self, msg: impl Into<String>) {
        self.error.set(Some(msg.into()));
    }

    /// Set loading state
    pub fn set_loading(&self, loading: bool) {
        self.is_loading.set(loading);
    }

    /// Update the last analysis timestamp
    pub fn update_last_analysis(&self) {
        self.last_analysis.set(Some(chrono::Utc::now().to_rfc3339()));
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
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "blocker" => Severity::Blocker,
            "critical" => Severity::Critical,
            "major" => Severity::Major,
            "minor" => Severity::Minor,
            _ => Severity::Info,
        }
    }

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
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "reliability" => Category::Reliability,
            "security" => Category::Security,
            "maintainability" => Category::Maintainability,
            "coverage" => Category::Coverage,
            "duplicate" => Category::Duplicate,
            _ => Category::Complexity,
        }
    }

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

/// Analysis summary returned by the server
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisSummary {
    pub project_path: String,
    pub timestamp: String,
    pub lines_of_code: usize,
    pub ratings: ProjectRatings,
    pub technical_debt: TechnicalDebt,
    pub total_issues: usize,
    pub blocker_issues: usize,
    pub critical_issues: usize,
    pub major_issues: usize,
    pub minor_issues: usize,
    pub info_issues: usize,
}

/// Full analysis result with all details
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub summary: AnalysisSummary,
    pub issues: Vec<IssueResult>,
    pub quality_gate: QualityGateResult,
}

/// Issue filter parameters
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct IssueFilter {
    pub project_path: String,
    pub severity: Option<String>,
    pub category: Option<String>,
    pub rule_id: Option<String>,
    pub file_path: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

/// Paginated issue list response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueListResponse {
    pub issues: Vec<IssueResult>,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

/// Project metrics DTO
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectMetricsDto {
    pub ncloc: usize,
    pub functions: usize,
    pub classes: usize,
    pub code_smells: usize,
    pub bugs: usize,
    pub vulnerabilities: usize,
    pub issues_by_severity: std::collections::HashMap<String, usize>,
}

/// Rule profile
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rule_count: usize,
}

/// Quality gate definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityGateDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub conditions: Vec<GateConditionTemplate>,
}

/// Template for a gate condition (before being evaluated)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateConditionTemplate {
    pub metric: String,
    pub name: String,
    pub operator: String,
    pub threshold: f64,
}

// ============================================================================
// Reactive Application State (using API client)
// ============================================================================

/// Reactive application state using signals for real-time UI updates
#[derive(Clone)]
pub struct ReactiveAppState {
    /// API client for server communication
    pub api: ApiClient,
    /// Current project path being analyzed
    pub project_path: RwSignal<String>,
    /// Name of the selected project (shown in Dashboard header)
    pub selected_project_name: RwSignal<Option<String>>,
    /// Dashboard configuration
    pub config: RwSignal<DashboardConfigDto>,
    /// Last analysis summary from the server
    pub analysis: RwSignal<Option<AnalysisSummaryDto>>,
    /// Current issues list
    pub issues: RwSignal<Vec<IssueDto>>,
    /// Total number of issues matching current filters
    pub total_issues_count: RwSignal<usize>,
    /// Total number of pages available
    pub total_pages: RwSignal<usize>,
    /// Whether an operation is currently in progress
    pub loading: RwSignal<bool>,
    /// Current error message, if any
    pub error: RwSignal<Option<String>>,
}

impl ReactiveAppState {
    /// Create a new ReactiveAppState with default values
    pub fn new() -> Self {
        Self {
            api: ApiClient::new("http://localhost:3000"),
            project_path: RwSignal::new(String::new()),
            selected_project_name: RwSignal::new(None),
            config: RwSignal::new(DashboardConfigDto::default()),
            analysis: RwSignal::new(None),
            issues: RwSignal::new(Vec::new()),
            total_issues_count: RwSignal::new(0),
            total_pages: RwSignal::new(1),
            loading: RwSignal::new(false),
            error: RwSignal::new(None),
        }
    }

    /// Create a new ReactiveAppState with the given project path
    pub fn with_project(project_path: impl Into<String>) -> Self {
        let path = project_path.into();
        Self {
            api: ApiClient::new("http://localhost:3000"),
            project_path: RwSignal::new(path.clone()),
            selected_project_name: RwSignal::new(None),
            config: RwSignal::new(DashboardConfigDto {
                project_path: path,
                ..Default::default()
            }),
            analysis: RwSignal::new(None),
            issues: RwSignal::new(Vec::new()),
            total_issues_count: RwSignal::new(0),
            total_pages: RwSignal::new(1),
            loading: RwSignal::new(false),
            error: RwSignal::new(None),
        }
    }

    /// Run analysis on the current project
    pub async fn run_analysis(&self) {
        self.loading.set(true);
        self.error.set(None);

        let path = self.project_path.get();
        if path.is_empty() {
            self.error.set(Some("Project path is empty".to_string()));
            self.loading.set(false);
            return;
        }

        match self.api.run_analysis(&path, true, true).await {
            Ok(result) => {
                self.analysis.set(Some(result));
            }
            Err(e) => {
                self.error.set(Some(e));
            }
        }

        self.loading.set(false);
    }

    /// Load issues with optional filters and pagination
    pub async fn load_issues(
        &self,
        severity: Option<&str>,
        category: Option<&str>,
        file_filter: Option<&str>,
        page: usize,
    ) {
        self.loading.set(true);
        self.error.set(None);

        let path = self.project_path.get();
        if path.is_empty() {
            self.error.set(Some("Project path is empty".to_string()));
            self.loading.set(false);
            return;
        }

        match self.api.get_issues(&path, severity, category, file_filter, page, 20).await {
            Ok(response) => {
                self.issues.set(response.issues);
                self.total_issues_count.set(response.total_count);
                self.total_pages.set(response.total_pages.max(1));
            }
            Err(e) => {
                self.error.set(Some(e));
            }
        }

        self.loading.set(false);
    }

    /// Clear any current error
    pub fn clear_error(&self) {
        self.error.set(None);
    }

    /// Set loading state
    pub fn set_loading(&self, loading: bool) {
        self.loading.set(loading);
    }
}

impl Default for ReactiveAppState {
    fn default() -> Self {
        Self::new()
    }
}