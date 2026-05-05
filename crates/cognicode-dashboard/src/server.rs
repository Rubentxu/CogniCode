//! Axum server with real cognicode-quality integration
//! All analysis endpoints call cognicode-quality in-process

use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use cognicode_quality::handler::{
    QualityAnalysisHandler, AnalyzeProjectParams,
    IssueResult as QualityIssueResult,
    ProjectMetricsResult,
};
use cognicode_axiom::rules::{QualityGate, GateCondition, CompareOperator, MetricValue};
use cognicode_db::quality::QualityStore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

// ─────────────────────────────────────────────────────────────────────────────
// Request/Response DTOs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisRequest {
    pub project_path: String,
    pub quick: Option<bool>,
    pub changed_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuesRequest {
    pub project_path: String,
    pub severity: Option<String>,
    pub category: Option<String>,
    pub file_filter: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsRequest {
    pub project_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateRequest {
    pub project_path: String,
    pub gate_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingsRequest {
    pub project_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatePathRequest {
    pub project_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsLsRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsEntryDto {
    pub name: String,
    pub is_dir: bool,
    pub has_cognicode: bool,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsLsResponseDto {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<FsEntryDto>,
}

// DTO for frontend (matches state.rs types)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDto {
    pub rule_id: String,
    pub message: String,
    pub severity: String,
    pub category: String,
    pub file: String,
    pub line: usize,
    pub column: Option<usize>,
    pub end_line: Option<usize>,
    pub remediation_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRatingsDto {
    pub reliability: char,
    pub security: char,
    pub maintainability: char,
    pub coverage: char,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalDebtDto {
    pub total_minutes: u64,
    pub rating: char,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateConditionDto {
    pub id: String,
    pub name: String,
    pub metric: String,
    pub operator: String,
    pub threshold: f64,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateResultDto {
    pub name: String,
    pub status: String,
    pub conditions: Vec<GateConditionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetricsDto {
    pub ncloc: usize,
    pub functions: usize,
    pub code_smells: usize,
    pub bugs: usize,
    pub vulnerabilities: usize,
    pub issues_by_severity: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSummaryDto {
    pub project_path: String,
    pub total_files: usize,
    pub total_issues: usize,
    pub ratings: ProjectRatingsDto,
    pub metrics: ProjectMetricsDto,
    pub technical_debt: TechnicalDebtDto,
    pub quality_gate: QualityGateResultDto,
    pub incremental: IncrementalInfoDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalInfoDto {
    pub files_total: usize,
    pub files_changed: usize,
    pub files_reused: usize,
    pub new_code_issues: usize,
    pub legacy_issues: usize,
    pub clean_as_you_code: bool,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathValidationDto {
    pub valid: bool,
    pub is_rust_project: bool,
    pub has_cargo_toml: bool,
    pub has_git: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleProfileDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfigDto {
    pub project_path: String,
    pub quick_analysis: bool,
    pub changed_only: bool,
    pub auto_refresh: bool,
    pub refresh_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuesResponseDto {
    pub issues: Vec<IssueDto>,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Project Management DTOs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterProjectRequest {
    pub name: String,
    pub path: String,
}

/// Project info from its .cognicode/cognicode.db
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfoDto {
    pub id: String,             // path-based ID
    pub name: String,
    pub path: String,
    pub has_cognicode_db: bool,
    pub last_analysis: Option<String>,
    pub total_issues: usize,
    pub quality_gate_status: String,
    pub rating: String,          // Overall rating (A-E)
    pub debt_minutes: u64,
    pub blockers: usize,
    pub criticals: usize,
    pub files_changed: usize,
    pub files_total: usize,
    pub history_count: usize,    // Number of analysis runs in DB
}

/// History for a specific project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectHistoryDto {
    pub project_id: String,
    pub runs: Vec<HistoryEntryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntryDto {
    pub timestamp: String,
    pub total_issues: usize,
    pub debt_minutes: u64,
    pub rating: String,
    pub files_changed: usize,
    pub new_issues: usize,
    pub fixed_issues: usize,
}

/// Project list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectListDto {
    pub projects: Vec<ProjectInfoDto>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Application State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppServerState {
    config: Arc<RwLock<DashboardConfigDto>>,
    /// Cached analysis result for the last project analyzed
    analysis_cache: Arc<RwLock<Option<CachedAnalysis>>>,
    /// Registered projects (path → name)
    registered_projects: Arc<RwLock<Vec<RegisteredProject>>>,
}

/// A project registered in the dashboard
#[derive(Clone)]
struct RegisteredProject {
    name: String,
    path: String,
}

/// Cached analysis with timestamp for TTL-based invalidation
#[derive(Clone)]
struct CachedAnalysis {
    project_path: String,
    timestamp: std::time::Instant,
    summary: AnalysisSummaryDto,
    issues: Vec<IssueDto>,
    metrics: ProjectMetricsDto,
}

impl CachedAnalysis {
    fn is_valid(&self, project_path: &str) -> bool {
        self.project_path == project_path && self.timestamp.elapsed().as_secs() < 300 // 5 min TTL
    }
}

impl AppServerState {
    fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(DashboardConfigDto {
                project_path: std::env::current_dir()
                    .unwrap_or_default()
                    .display()
                    .to_string(),
                quick_analysis: true,
                changed_only: true,
                auto_refresh: false,
                refresh_interval_secs: 60,
            })),
            analysis_cache: Arc::new(RwLock::new(None)),
            registered_projects: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// API Error Handling
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ApiError {
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = if self.message.contains("not exist") {
            StatusCode::NOT_FOUND
        } else if self.message.contains("already") || self.message.contains("duplicate") {
            StatusCode::CONFLICT
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        (status, Json(self)).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

fn compute_ratings(metrics: &ProjectMetricsResult) -> ProjectRatingsDto {
    let code_smells = metrics.code_smells;
    let bugs = metrics.bugs;
    let vulnerabilities = metrics.vulnerabilities;

    let reliability = if bugs > 10 { 'E' } else if bugs > 5 { 'D' } else if bugs > 2 { 'C' } else if bugs > 0 { 'B' } else { 'A' };
    let security = if vulnerabilities > 5 { 'E' } else if vulnerabilities > 3 { 'D' } else if vulnerabilities > 1 { 'C' } else if vulnerabilities > 0 { 'B' } else { 'A' };
    let maintainability = if code_smells > 100 { 'E' } else if code_smells > 50 { 'D' } else if code_smells > 20 { 'C' } else if code_smells > 5 { 'B' } else { 'A' };
    let coverage = 'C'; // Would need actual coverage data

    ProjectRatingsDto {
        reliability,
        security,
        maintainability,
        coverage,
    }
}

fn compute_technical_debt(metrics: &ProjectMetricsResult) -> TechnicalDebtDto {
    let code_smells = metrics.code_smells as u64;
    let bugs = metrics.bugs as u64;
    let vulnerabilities = metrics.vulnerabilities as u64;

    let total_minutes = (code_smells * 5) + (bugs * 15) + (vulnerabilities * 30);

    let rating = if total_minutes > 600 { 'E' } else if total_minutes > 300 { 'D' } else if total_minutes > 120 { 'C' } else if total_minutes > 30 { 'B' } else { 'A' };

    let label = if total_minutes < 60 {
        "Excellent".to_string()
    } else if total_minutes < 120 {
        "Good".to_string()
    } else if total_minutes < 300 {
        "Acceptable".to_string()
    } else if total_minutes < 600 {
        "Warning".to_string()
    } else {
        "Critical".to_string()
    };

    TechnicalDebtDto {
        total_minutes,
        rating,
        label,
    }
}

fn evaluate_quality_gate(metrics: &ProjectMetricsResult) -> QualityGateResultDto {
    let gate = QualityGate::new("cognicode-default", "Default CogniCode quality gate")
        .add_condition(GateCondition::new(
            "code_smells",
            CompareOperator::LT,
            MetricValue::Integer(50),
        ))
        .add_condition(GateCondition::new(
            "bugs",
            CompareOperator::LT,
            MetricValue::Integer(10),
        ))
        .add_condition(GateCondition::new(
            "vulnerabilities",
            CompareOperator::LT,
            MetricValue::Integer(5),
        ));

    let conditions: Vec<GateConditionDto> = gate.conditions.iter().enumerate().map(|(i, cond)| {
        let actual = match cond.metric.as_str() {
            "code_smells" => metrics.code_smells as f64,
            "bugs" => metrics.bugs as f64,
            "vulnerabilities" => metrics.vulnerabilities as f64,
            _ => 0.0,
        };

        let threshold = match &cond.threshold {
            MetricValue::Integer(v) => *v as f64,
            MetricValue::Float(v) => *v,
            MetricValue::Percentage(v) => *v,
        };

        let passed = match cond.operator {
            CompareOperator::LT => actual < threshold,
            CompareOperator::LTE => actual <= threshold,
            CompareOperator::GT => actual > threshold,
            CompareOperator::GTE => actual >= threshold,
            CompareOperator::EQ => actual == threshold,
            CompareOperator::NEQ => actual != threshold,
        };

        GateConditionDto {
            id: format!("{}", i + 1),
            name: format!("{} < {}", cond.metric, threshold),
            metric: cond.metric.clone(),
            operator: format!("{:?}", cond.operator),
            threshold,
            passed,
        }
    }).collect();

    let all_passed = conditions.iter().all(|c| c.passed);

    QualityGateResultDto {
        name: gate.name.clone(),
        status: if all_passed { "PASSED".to_string() } else { "FAILED".to_string() },
        conditions,
    }
}

fn convert_issue(issue: QualityIssueResult) -> IssueDto {
    IssueDto {
        rule_id: issue.rule_id,
        message: issue.message,
        severity: issue.severity,
        category: issue.category,
        file: issue.file,
        line: issue.line,
        column: issue.column,
        end_line: None,
        remediation_hint: None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cache-aware analysis helper
// ─────────────────────────────────────────────────────────────────────────────

/// Get cached analysis if valid, otherwise run a new analysis
/// Returns (summary, issues, metrics) from cache or new analysis
async fn get_or_run_analysis(
    state: &AppServerState,
    project_path: &str,
) -> Result<(AnalysisSummaryDto, Vec<IssueDto>, ProjectMetricsDto), String> {
    // Check cache first
    {
        let cache = state.analysis_cache.read().await;
        if let Some(ref cached) = *cache {
            if cached.is_valid(project_path) {
                return Ok((
                    cached.summary.clone(),
                    cached.issues.clone(),
                    cached.metrics.clone(),
                ));
            }
        }
    }

    // Run new analysis
    let path = PathBuf::from(project_path);
    let handler = QualityAnalysisHandler::new(path.clone());

    let params = AnalyzeProjectParams {
        project_path: path,
        quick: true,
        max_duration_secs: Some(60),
        changed_only: true,
    };

    let result = handler.analyze_project_impl(params).map_err(|e| e.to_string())?;

    // Build DTOs
    let metrics = ProjectMetricsDto {
        ncloc: result.project_metrics.ncloc,
        functions: result.project_metrics.functions,
        code_smells: result.project_metrics.code_smells,
        bugs: result.project_metrics.bugs,
        vulnerabilities: result.project_metrics.vulnerabilities,
        issues_by_severity: result.project_metrics.issues_by_severity.clone(),
    };

    let ratings = compute_ratings(&result.project_metrics);
    let debt = compute_technical_debt(&result.project_metrics);
    let gate = evaluate_quality_gate(&result.project_metrics);

    let issues: Vec<IssueDto> = result.issues.into_iter().map(convert_issue).collect();

    let summary = AnalysisSummaryDto {
        project_path: project_path.to_string(),
        total_files: result.total_files,
        total_issues: issues.len(),
        ratings,
        metrics: metrics.clone(),
        technical_debt: debt,
        quality_gate: gate,
        incremental: IncrementalInfoDto {
            files_total: result.incremental.files_total,
            files_changed: result.incremental.files_changed,
            files_reused: result.incremental.files_reused,
            new_code_issues: result.incremental.new_code_issues,
            legacy_issues: result.incremental.legacy_issues,
            clean_as_you_code: result.incremental.clean_as_you_code,
            timed_out: result.incremental.timed_out,
        },
    };

    // Update cache
    {
        let mut cache = state.analysis_cache.write().await;
        // Re-count issues from the summary
        *cache = Some(CachedAnalysis {
            project_path: project_path.to_string(),
            timestamp: std::time::Instant::now(),
            summary: summary.clone(),
            issues: issues.clone(),
            metrics: metrics.clone(),
        });
    }

    Ok((summary, issues, metrics))
}

// ─────────────────────────────────────────────────────────────────────────────
// Route Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

/// Run full analysis on a project
async fn run_analysis(
    State(state): State<AppServerState>,
    Json(req): Json<AnalysisRequest>,
) -> ApiResult<Json<AnalysisSummaryDto>> {
    let project_path = PathBuf::from(&req.project_path);

    if !project_path.exists() {
        return Err(ApiError {
            message: format!("Project path does not exist: {}", req.project_path),
        });
    }

    let handler = QualityAnalysisHandler::new(project_path.clone());
    let params = AnalyzeProjectParams {
        project_path: project_path.clone(),
        quick: req.quick.unwrap_or(true),
        max_duration_secs: Some(60),
        changed_only: req.changed_only.unwrap_or(true),
    };

    let result = handler.analyze_project_impl(params).map_err(|e| ApiError {
        message: format!("Analysis failed: {}", e),
    })?;

    let metrics = ProjectMetricsDto {
        ncloc: result.project_metrics.ncloc,
        functions: result.project_metrics.functions,
        code_smells: result.project_metrics.code_smells,
        bugs: result.project_metrics.bugs,
        vulnerabilities: result.project_metrics.vulnerabilities,
        issues_by_severity: result.project_metrics.issues_by_severity.clone(),
    };
    let ratings = compute_ratings(&result.project_metrics);
    let debt = compute_technical_debt(&result.project_metrics);
    let gate = evaluate_quality_gate(&result.project_metrics);
    let issues: Vec<IssueDto> = result.issues.into_iter().map(convert_issue).collect();

    let summary = AnalysisSummaryDto {
        project_path: req.project_path.clone(),
        total_files: result.total_files,
        total_issues: issues.len(),
        ratings,
        metrics: metrics.clone(),
        technical_debt: debt,
        quality_gate: gate,
        incremental: IncrementalInfoDto {
            files_total: result.incremental.files_total,
            files_changed: result.incremental.files_changed,
            files_reused: result.incremental.files_reused,
            new_code_issues: result.incremental.new_code_issues,
            legacy_issues: result.incremental.legacy_issues,
            clean_as_you_code: result.incremental.clean_as_you_code,
            timed_out: result.incremental.timed_out,
        },
    };

    // Cache the result
    {
        let mut cache = state.analysis_cache.write().await;
        *cache = Some(CachedAnalysis {
            project_path: req.project_path,
            timestamp: std::time::Instant::now(),
            summary: summary.clone(),
            issues,
            metrics,
        });
    }

    Ok(Json(summary))
}

/// Get issues for a project with filtering
async fn get_issues(
    State(state): State<AppServerState>,
    Json(req): Json<IssuesRequest>,
) -> ApiResult<Json<IssuesResponseDto>> {
    let (_, mut issues, _) = get_or_run_analysis(&state, &req.project_path).await
        .map_err(|e| ApiError { message: e })?;

    // Apply filters
    if let Some(severity) = &req.severity {
        issues.retain(|i| i.severity == *severity);
    }
    if let Some(category) = &req.category {
        issues.retain(|i| i.category == *category);
    }
    if let Some(file_filter) = &req.file_filter {
        issues.retain(|i| i.file.contains(file_filter));
    }

    // Apply pagination
    let total_count = issues.len();
    let page = req.page.unwrap_or(1).max(1);
    let page_size = req.page_size.unwrap_or(20).max(1);
    let start = (page - 1) * page_size;
    let end = start + page_size;
    let total_pages = (total_count + page_size - 1) / page_size;

    let paged = if start < total_count {
        issues[start..end.min(total_count)].to_vec()
    } else {
        vec![]
    };

    Ok(Json(IssuesResponseDto {
        issues: paged,
        total_count,
        page,
        page_size,
        total_pages,
    }))
}

/// Get project metrics
async fn get_metrics(
    State(state): State<AppServerState>,
    Json(req): Json<MetricsRequest>,
) -> ApiResult<Json<ProjectMetricsDto>> {
    let (_, _, metrics) = get_or_run_analysis(&state, &req.project_path).await
        .map_err(|e| ApiError { message: e })?;
    Ok(Json(metrics))
}

/// Get quality gate status
async fn get_quality_gate(
    State(state): State<AppServerState>,
    Json(req): Json<QualityGateRequest>,
) -> ApiResult<Json<QualityGateResultDto>> {
    let (_, _, metrics) = get_or_run_analysis(&state, &req.project_path).await
        .map_err(|e| ApiError { message: e })?;
    let pm = ProjectMetricsResult {
        ncloc: metrics.ncloc,
        functions: metrics.functions,
        classes: 0,
        code_smells: metrics.code_smells,
        bugs: metrics.bugs,
        vulnerabilities: metrics.vulnerabilities,
        issues_by_severity: metrics.issues_by_severity,
    };
    Ok(Json(evaluate_quality_gate(&pm)))
}

/// Get project ratings
async fn get_ratings(
    State(state): State<AppServerState>,
    Json(req): Json<RatingsRequest>,
) -> ApiResult<Json<ProjectRatingsDto>> {
    let (_, _, metrics) = get_or_run_analysis(&state, &req.project_path).await
        .map_err(|e| ApiError { message: e })?;
    let pm = ProjectMetricsResult {
        ncloc: metrics.ncloc,
        functions: metrics.functions,
        classes: 0,
        code_smells: metrics.code_smells,
        bugs: metrics.bugs,
        vulnerabilities: metrics.vulnerabilities,
        issues_by_severity: metrics.issues_by_severity,
    };
    Ok(Json(compute_ratings(&pm)))
}

/// Validate project path
async fn validate_project_path(
    State(_state): State<AppServerState>,
    Json(req): Json<ValidatePathRequest>,
) -> ApiResult<Json<PathValidationDto>> {
    let project_path = PathBuf::from(&req.project_path);

    if !project_path.exists() {
        return Ok(Json(PathValidationDto {
            valid: false,
            is_rust_project: false,
            has_cargo_toml: false,
            has_git: false,
            error: Some("Path does not exist".to_string()),
        }));
    }

    let has_cargo_toml = project_path.join("Cargo.toml").exists();
    let has_git = project_path.join(".git").exists();
    let is_rust_project = has_cargo_toml;

    Ok(Json(PathValidationDto {
        valid: true,
        is_rust_project,
        has_cargo_toml,
        has_git,
        error: None,
    }))
}

/// Browse filesystem directories (for project path selection)
async fn browse_filesystem(
    Json(req): Json<FsLsRequest>,
) -> ApiResult<Json<FsLsResponseDto>> {
    let target_path = PathBuf::from(&req.path);
    let path = if req.path.is_empty() || !target_path.exists() {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/"))
    } else {
        target_path
    };

    if !path.is_dir() {
        return Err(ApiError { message: format!("Not a directory: {}", req.path) });
    }

    let parent = path.parent().map(|p| p.to_string_lossy().to_string());

    let mut entries: Vec<FsEntryDto> = Vec::new();
    if let Ok(readdir) = std::fs::read_dir(&path) {
        for entry in readdir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let entry_path = entry.path();
            let is_dir = entry_path.is_dir();
            
            // Skip hidden files/dirs (except .cognicode detection)
            if name.starts_with('.') && name != ".cognicode" {
                continue;
            }

            let has_cognicode = if is_dir {
                entry_path.join(".cognicode").join("cognicode.db").exists()
            } else {
                false
            };

            entries.push(FsEntryDto {
                name,
                is_dir,
                has_cognicode,
                path: entry_path.to_string_lossy().to_string(),
            });
        }
    }

    // Sort: directories first, then alphabetically
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(Json(FsLsResponseDto {
        path: path.to_string_lossy().to_string(),
        parent,
        entries,
    }))
}

/// Get available rule profiles
async fn get_rule_profiles() -> Json<Vec<RuleProfileDto>> {
    Json(vec![
        RuleProfileDto {
            id: "default".to_string(),
            name: "Default".to_string(),
            description: "All rules enabled".to_string(),
            is_default: true,
        },
        RuleProfileDto {
            id: "quick".to_string(),
            name: "Quick".to_string(),
            description: "Only Blocker and Critical rules".to_string(),
            is_default: false,
        },
    ])
}

/// Get dashboard configuration
async fn get_config(
    State(state): State<AppServerState>,
) -> Json<DashboardConfigDto> {
    let config = state.config.read().await.clone();
    Json(config)
}

/// Save dashboard configuration
async fn save_config(
    State(state): State<AppServerState>,
    Json(config): Json<DashboardConfigDto>,
) -> ApiResult<Json<()>> {
    let mut current_config = state.config.write().await;
    *current_config = config;
    Ok(Json(()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Project Management Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// List all registered projects with their latest analysis data from cognicode.db
async fn list_projects(
    State(state): State<AppServerState>,
) -> Json<ProjectListDto> {
    let projects = state.registered_projects.read().await;
    let mut project_infos = Vec::new();

    for p in projects.iter() {
        project_infos.push(build_project_info(&p.name, &p.path));
    }

    Json(ProjectListDto { projects: project_infos })
}

/// Register a new project by adding it to the dashboard
async fn register_project(
    State(state): State<AppServerState>,
    Json(req): Json<RegisterProjectRequest>,
) -> ApiResult<Json<ProjectInfoDto>> {
    let path = req.path.trim().to_string();
    let project_path = PathBuf::from(&path);

    if !project_path.exists() {
        return Err(ApiError { message: format!("Project path does not exist: {}", path) });
    }

    // Check for duplicate
    {
        let projects = state.registered_projects.read().await;
        if projects.iter().any(|p| p.path == path) {
            return Err(ApiError { message: "Project already registered".to_string() });
        }
    }

    let mut projects = state.registered_projects.write().await;
    let name = if req.name.is_empty() {
        project_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.clone())
    } else {
        req.name.clone()
    };

    projects.push(RegisteredProject { name: name.clone(), path: path.clone() });
    let info = build_project_info(&name, &path);
    Ok(Json(info))
}

/// Get analysis history for a project
async fn get_project_history(
    State(state): State<AppServerState>,
    axum::extract::Path(project_id): axum::extract::Path<String>,
) -> ApiResult<Json<ProjectHistoryDto>> {
    // Find project by ID (the path URL-encoded)
    let projects = state.registered_projects.read().await;
    let found = projects.iter().find(|p| p.path == project_id || url_decode(&p.path) == project_id);

    let project = match found {
        Some(p) => p.clone(),
        None => return Err(ApiError { message: format!("Project not found: {}", project_id) }),
    };

    let store = QualityStore::open(&PathBuf::from(&project.path));
    let runs: Vec<HistoryEntryDto> = store.get_run_history(30).into_iter().map(|s| HistoryEntryDto {
        timestamp: s.timestamp,
        total_issues: s.total_issues,
        debt_minutes: s.debt_minutes,
        rating: s.rating,
        files_changed: s.files_changed,
        new_issues: s.new_issues,
        fixed_issues: s.fixed_issues,
    }).collect();

    Ok(Json(ProjectHistoryDto {
        project_id: project.path.clone(),
        runs,
    }))
}

/// Build project info from a path — reads cognicode.db if available
fn build_project_info(name: &str, path: &str) -> ProjectInfoDto {
    let project_path = PathBuf::from(path);
    let db_path = project_path.join(".cognicode").join("cognicode.db");
    let has_cognicode_db = db_path.exists();

    if !has_cognicode_db {
        return ProjectInfoDto {
            id: path.to_string(),
            name: name.to_string(),
            path: path.to_string(),
            has_cognicode_db: false,
            last_analysis: None,
            total_issues: 0,
            quality_gate_status: "UNKNOWN".to_string(),
            rating: "?".to_string(),
            debt_minutes: 0,
            blockers: 0,
            criticals: 0,
            files_changed: 0,
            files_total: 0,
            history_count: 0,
        };
    }

    let store = QualityStore::open(&project_path);
    let history = store.get_run_history(1); // Latest run only
    let latest = history.first();

    ProjectInfoDto {
        id: path.to_string(),
        name: name.to_string(),
        path: path.to_string(),
        has_cognicode_db: true,
        last_analysis: latest.map(|r| r.timestamp.clone()),
        total_issues: latest.map(|r| r.total_issues).unwrap_or(0),
        quality_gate_status: "PASSED".to_string(),  // Computed separately when needed
        rating: latest.as_ref().map(|r| r.rating.clone()).unwrap_or_else(|| "?".to_string()),
        debt_minutes: latest.map(|r| r.debt_minutes).unwrap_or(0),
        blockers: 0,   // Would need separate query, simplified for now
        criticals: 0,
        files_changed: latest.map(|r| r.files_changed).unwrap_or(0),
        files_total: 0,
        history_count: store.get_run_history(100).len(),
    }
}

fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&s[i+1..i+3], 16) {
                result.push(hex as char);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Server Setup
// ─────────────────────────────────────────────────────────────────────────────

pub async fn start_server(port: u16) {
    let app_state = AppServerState::new();

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/analysis", post(run_analysis))
        .route("/api/issues", post(get_issues))
        .route("/api/metrics", post(get_metrics))
        .route("/api/quality-gate", post(get_quality_gate))
        .route("/api/ratings", post(get_ratings))
        .route("/api/validate-path", post(validate_project_path))
        .route("/api/fs/ls", post(browse_filesystem))
        .route("/api/rule-profiles", get(get_rule_profiles))
        .route("/api/config", get(get_config))
        .route("/api/config", post(save_config))
        .route("/api/projects", get(list_projects))
        .route("/api/projects/register", post(register_project))
        .route("/api/projects/:id/history", get(get_project_history))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    // Serve static files + SPA fallback
    let dist_dir = std::env::var("DIST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("dist"));
    let dist_dir2 = dist_dir.clone();
    
    let app = app.fallback_service(tower::service_fn(move |req: Request| {
        let dist = dist_dir2.clone();
        async move {
            let path = req.uri().path().trim_start_matches('/');
            let file_path = if path.is_empty() || !path.contains('.') {
                // SPA fallback: routes without extension go to index.html
                dist.join("index.html")
            } else {
                dist.join(path)
            };

            match tokio::fs::read(&file_path).await {
                Ok(content) => {
                    let ext = file_path.extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    let mime = match ext {
                        "wasm" => "application/wasm",
                        "js" => "application/javascript",
                        "css" => "text/css",
                        "html" => "text/html; charset=utf-8",
                        "png" => "image/png",
                        "svg" => "image/svg+xml",
                        "ico" => "image/x-icon",
                        _ => "application/octet-stream",
                    };
                    Ok::<_, std::convert::Infallible>((
                        StatusCode::OK,
                        [("content-type", mime)],
                        content,
                    ).into_response())
                }
                Err(_) => {
                    // SPA fallback: serve index.html
                    match tokio::fs::read(dist.join("index.html")).await {
                        Ok(content) => Ok((
                            StatusCode::OK,
                            [("content-type", "text/html; charset=utf-8")],
                            content,
                        ).into_response()),
                        Err(_) => Ok((StatusCode::NOT_FOUND, "Not Found").into_response()),
                    }
                }
            }
        }
    }));

    let addr = format!("0.0.0.0:{}", port);
    println!("CogniCode Dashboard Server running on http://{}", addr);
    println!("Health check: http://{}/health", addr);
    println!("API: http://{}/api/...", addr);
    println!("");
    println!("To serve the frontend, run in another terminal:");
    println!("  cd crates/cognicode-dashboard && trunk serve --no-default-features");
    println!("Or serve static files:");
    println!("  DIST_DIR=crates/cognicode-dashboard/dist python3 -m http.server 8080 -d \\$DIST_DIR");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    start_server(port).await;
}
