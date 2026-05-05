//! Axum server with real cognicode-quality integration
//! All analysis endpoints call cognicode-quality in-process

use axum::{
    extract::State,
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
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

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

// ─────────────────────────────────────────────────────────────────────────────
// Application State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppServerState {
    config: Arc<RwLock<DashboardConfigDto>>,
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
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
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
// API Handlers
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
        project_path,
        quick: req.quick.unwrap_or(true),
        max_duration_secs: Some(120),
        changed_only: req.changed_only.unwrap_or(true),
    };

    let result = handler.analyze_project_impl(params).map_err(|e| ApiError {
        message: format!("Analysis failed: {}", e),
    })?;

    let ratings = compute_ratings(&result.project_metrics);
    let debt = compute_technical_debt(&result.project_metrics);
    let gate = evaluate_quality_gate(&result.project_metrics);

    let summary = AnalysisSummaryDto {
        project_path: result.project_path,
        total_files: result.total_files,
        total_issues: result.total_issues,
        ratings,
        metrics: ProjectMetricsDto {
            ncloc: result.project_metrics.ncloc,
            functions: result.project_metrics.functions,
            code_smells: result.project_metrics.code_smells,
            bugs: result.project_metrics.bugs,
            vulnerabilities: result.project_metrics.vulnerabilities,
            issues_by_severity: result.project_metrics.issues_by_severity,
        },
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

    Ok(Json(summary))
}

/// Get issues for a project with filtering
async fn get_issues(
    State(_state): State<AppServerState>,
    Json(req): Json<IssuesRequest>,
) -> ApiResult<Json<Vec<IssueDto>>> {
    let project_path = PathBuf::from(&req.project_path);

    if !project_path.exists() {
        return Err(ApiError {
            message: format!("Project path does not exist: {}", req.project_path),
        });
    }

    let handler = QualityAnalysisHandler::new(project_path.clone());

    let params = AnalyzeProjectParams {
        project_path,
        quick: false,
        max_duration_secs: Some(120),
        changed_only: false,
    };

    let result = handler.analyze_project_impl(params).map_err(|e| ApiError {
        message: format!("Analysis failed: {}", e),
    })?;

    let mut issues: Vec<IssueDto> = result.issues.into_iter().map(convert_issue).collect();

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
    let page = req.page.unwrap_or(1);
    let page_size = req.page_size.unwrap_or(50);
    let start = (page - 1) * page_size;
    let end = start + page_size;

    if start < issues.len() {
        issues = issues[start..end.min(issues.len())].to_vec();
    } else {
        issues = vec![];
    }

    Ok(Json(issues))
}

/// Get project metrics
async fn get_metrics(
    State(_state): State<AppServerState>,
    Json(req): Json<MetricsRequest>,
) -> ApiResult<Json<ProjectMetricsDto>> {
    let project_path = PathBuf::from(&req.project_path);

    if !project_path.exists() {
        return Err(ApiError {
            message: format!("Project path does not exist: {}", req.project_path),
        });
    }

    let handler = QualityAnalysisHandler::new(project_path.clone());

    let params = AnalyzeProjectParams {
        project_path,
        quick: true,
        max_duration_secs: Some(60),
        changed_only: true,
    };

    let result = handler.analyze_project_impl(params).map_err(|e| ApiError {
        message: format!("Analysis failed: {}", e),
    })?;

    Ok(Json(ProjectMetricsDto {
        ncloc: result.project_metrics.ncloc,
        functions: result.project_metrics.functions,
        code_smells: result.project_metrics.code_smells,
        bugs: result.project_metrics.bugs,
        vulnerabilities: result.project_metrics.vulnerabilities,
        issues_by_severity: result.project_metrics.issues_by_severity,
    }))
}

/// Get quality gate status
async fn get_quality_gate(
    State(_state): State<AppServerState>,
    Json(req): Json<QualityGateRequest>,
) -> ApiResult<Json<QualityGateResultDto>> {
    let project_path = PathBuf::from(&req.project_path);

    if !project_path.exists() {
        return Err(ApiError {
            message: format!("Project path does not exist: {}", req.project_path),
        });
    }

    let handler = QualityAnalysisHandler::new(project_path.clone());

    let params = AnalyzeProjectParams {
        project_path,
        quick: true,
        max_duration_secs: Some(60),
        changed_only: true,
    };

    let result = handler.analyze_project_impl(params).map_err(|e| ApiError {
        message: format!("Analysis failed: {}", e),
    })?;

    Ok(Json(evaluate_quality_gate(&result.project_metrics)))
}

/// Get project ratings
async fn get_ratings(
    State(_state): State<AppServerState>,
    Json(req): Json<RatingsRequest>,
) -> ApiResult<Json<ProjectRatingsDto>> {
    let project_path = PathBuf::from(&req.project_path);

    if !project_path.exists() {
        return Err(ApiError {
            message: format!("Project path does not exist: {}", req.project_path),
        });
    }

    let handler = QualityAnalysisHandler::new(project_path.clone());

    let params = AnalyzeProjectParams {
        project_path,
        quick: true,
        max_duration_secs: Some(60),
        changed_only: true,
    };

    let result = handler.analyze_project_impl(params).map_err(|e| ApiError {
        message: format!("Analysis failed: {}", e),
    })?;

    Ok(Json(compute_ratings(&result.project_metrics)))
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
        .route("/api/rule-profiles", get(get_rule_profiles))
        .route("/api/config", get(get_config))
        .route("/api/config", post(save_config))
        .with_state(app_state);

    let addr = format!("0.0.0.0:{}", port);
    println!("CogniCode Dashboard Server running on http://{}", addr);
    println!("Health check: http://{}/health", addr);

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
