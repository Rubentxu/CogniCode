//! API Client Module
//!
//! HTTP client for calling the CogniCode server endpoints.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// DTOs matching server responses
// ============================================================================

/// Analysis summary returned by the server
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

/// Project ratings (A-E scale)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRatingsDto {
    pub reliability: char,
    pub security: char,
    pub maintainability: char,
    pub coverage: char,
}

/// Project metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetricsDto {
    pub ncloc: usize,
    pub functions: usize,
    pub code_smells: usize,
    pub bugs: usize,
    pub vulnerabilities: usize,
    #[serde(default)]
    pub issues_by_severity: HashMap<String, usize>,
}

/// Technical debt summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalDebtDto {
    pub total_minutes: u64,
    pub rating: char,
    pub label: String,
}

/// Quality gate condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateConditionDto {
    pub id: String,
    pub name: String,
    pub metric: String,
    pub operator: String,
    pub threshold: f64,
    pub passed: bool,
}

/// Quality gate overall status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateResultDto {
    pub name: String,
    pub status: String,
    pub conditions: Vec<GateConditionDto>,
}

/// Incremental analysis info
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

/// A single issue found during analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDto {
    pub rule_id: String,
    pub message: String,
    pub severity: String,
    pub category: String,
    pub file: String,
    pub line: usize,
    #[serde(default)]
    pub column: Option<usize>,
    #[serde(default)]
    pub end_line: Option<usize>,
    #[serde(default)]
    pub remediation_hint: Option<String>,
    #[serde(default)]
    pub effort_minutes: Option<u32>,
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub code_snippet: Option<String>,
    #[serde(default)]
    pub variable_name: Option<String>,
}

/// Path validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathValidationDto {
    pub valid: bool,
    pub is_rust_project: bool,
    pub has_cargo_toml: bool,
    pub has_git: bool,
    #[serde(default)]
    pub error: Option<String>,
}

/// Dashboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfigDto {
    pub project_path: String,
    #[serde(default = "default_quick_analysis")]
    pub quick_analysis: bool,
    #[serde(default)]
    pub changed_only: bool,
    #[serde(default = "default_true")]
    pub auto_refresh: bool,
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,
}

fn default_quick_analysis() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_refresh_interval() -> u64 {
    60
}

impl Default for DashboardConfigDto {
    fn default() -> Self {
        Self {
            project_path: String::new(),
            quick_analysis: true,
            changed_only: false,
            auto_refresh: true,
            refresh_interval_secs: 60,
        }
    }
}

/// Issues request body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuesRequestDto {
    pub project_path: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub file_filter: Option<String>,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_page() -> usize {
    1
}

fn default_page_size() -> usize {
    50
}

/// Issues response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuesResponseDto {
    pub issues: Vec<IssueDto>,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

/// A single drift detection event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftEventDto {
    pub id: i64,
    pub timestamp: String,
    pub file_path: String,
    pub function_name: String,
    pub drift_score: f64,
    pub intent: Option<String>,
    pub severity: String,
}

/// Paginated drift events response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftResponseDto {
    pub events: Vec<DriftEventDto>,
    pub total_count: usize,
    pub offset: usize,
    pub limit: usize,
}

/// A single contract from the AVC analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDto {
    pub id: i64,
    pub source_file: String,
    pub function_name: String,
    pub compliance_score: f64,
    pub generated_at: String,
}

/// Result status breakdown counts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultStatusBreakdown {
    pub success: usize,
    pub error: usize,
    pub other: usize,
}

/// Agent tool usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatDto {
    pub tool_name: String,
    pub count: usize,
    pub avg_duration_ms: f64,
    pub result_status_breakdown: ResultStatusBreakdown,
}

/// Analysis request body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisRequestDto {
    pub project_path: String,
    #[serde(default = "default_quick_analysis")]
    pub quick: bool,
    #[serde(default)]
    pub changed_only: bool,
}



// ============================================================================
// Trends and Agent Tasks DTOs
// ============================================================================

/// Trend data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendEntryDto {
    pub date: String,
    pub total_issues: usize,
    pub debt_minutes: u64,
    pub rating: String,
}

/// Baseline comparison data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineComparisonDto {
    pub baseline_timestamp: String,
    pub issues_delta: i64,
    pub debt_delta: i64,
    pub rating_before: String,
    pub rating_after: String,
}

/// Trends response DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendsResponseDto {
    pub trends: Vec<TrendEntryDto>,
    pub baseline: Option<BaselineComparisonDto>,
}

/// Agent task DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskDto {
    pub id: i64,
    pub task_type: String,
    pub priority: i32,
    pub payload_json: String,
    pub status: String,
    pub created_by: String,
    pub created_at: String,
    pub assigned_at: Option<String>,
    pub completed_at: Option<String>,
    pub result_json: Option<String>,
    pub error_message: Option<String>,
}

/// Create task request DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub task_type: String,
    pub priority: Option<i32>,
    pub payload_json: String,
    pub created_by: Option<String>,
}

/// Create task response DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskResponse {
    pub task_id: i64,
    pub status: String,
}

/// Task list response DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskListResponse {
    pub tasks: Vec<AgentTaskDto>,
    pub total: usize,
}
// ============================================================================
// Project Management DTOs
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfoDto {
    pub id: String,
    pub name: String,
    pub path: String,
    pub has_cognicode_db: bool,
    pub last_analysis: Option<String>,
    pub total_issues: usize,
    pub quality_gate_status: String,
    pub rating: String,
    pub debt_minutes: u64,
    pub blockers: usize,
    pub criticals: usize,
    pub files_changed: usize,
    pub files_total: usize,
    pub history_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectListDto {
    pub projects: Vec<ProjectInfoDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterProjectRequestDto {
    pub name: String,
    pub path: String,
}

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

// ============================================================================
// API Client
// ============================================================================

/// API client for calling the CogniCode server
#[derive(Clone)]
pub struct ApiClient {
    base_url: Arc<String>,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: Arc::new(base_url.into()),
        }
    }

    /// Run analysis on a project
    pub async fn run_analysis(
        &self,
        project_path: &str,
        quick: bool,
        changed_only: bool,
    ) -> Result<AnalysisSummaryDto, String> {
        let url = format!("{}/api/analysis", self.base_url);
        let request = AnalysisRequestDto {
            project_path: project_path.to_string(),
            quick,
            changed_only,
        };

        Request::post(&url)
            .json(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get issues with optional filters — returns full paginated response
    pub async fn get_issues(
        &self,
        project_path: &str,
        severity: Option<&str>,
        category: Option<&str>,
        file_filter: Option<&str>,
        page: usize,
        page_size: usize,
    ) -> Result<IssuesResponseDto, String> {
        let url = format!("{}/api/issues", self.base_url);
        let request = IssuesRequestDto {
            project_path: project_path.to_string(),
            severity: severity.map(String::from),
            category: category.map(String::from),
            file_filter: file_filter.map(String::from),
            page,
            page_size,
        };

        let response: IssuesResponseDto = Request::post(&url)
            .json(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(response)
    }

    /// Get project metrics
    pub async fn get_metrics(&self, project_path: &str) -> Result<ProjectMetricsDto, String> {
        let url = format!("{}/api/metrics", self.base_url);

        #[derive(Serialize)]
        struct MetricsRequest<'a> {
            project_path: &'a str,
        }

        Request::post(&url)
            .json(&MetricsRequest { project_path })
            .map_err(|e| format!("Failed to serialize request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get quality gate result
    pub async fn get_quality_gate(
        &self,
        project_path: &str,
    ) -> Result<QualityGateResultDto, String> {
        let url = format!("{}/api/quality-gate", self.base_url);

        #[derive(Serialize)]
        struct QGRequest<'a> {
            project_path: &'a str,
        }

        Request::post(&url)
            .json(&QGRequest { project_path })
            .map_err(|e| format!("Failed to serialize request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get project ratings
    pub async fn get_ratings(&self, project_path: &str) -> Result<ProjectRatingsDto, String> {
        let url = format!("{}/api/ratings", self.base_url);

        #[derive(Serialize)]
        struct RatingsRequest<'a> {
            project_path: &'a str,
        }

        Request::post(&url)
            .json(&RatingsRequest { project_path })
            .map_err(|e| format!("Failed to serialize request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Validate a project path
    pub async fn validate_path(&self, project_path: &str) -> Result<PathValidationDto, String> {
        let url = format!("{}/api/validate-path", self.base_url);

        #[derive(Serialize)]
        struct ValidateRequest<'a> {
            project_path: &'a str,
        }

        Request::post(&url)
            .json(&ValidateRequest { project_path })
            .map_err(|e| format!("Failed to serialize request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Open native OS directory picker and return selected path
    pub async fn pick_directory(&self) -> Result<String, String> {
        let url = format!("{}/api/fs/pick-directory", self.base_url);
        let resp = Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.ok() {
            return Err(format!("Request failed with status: {}", resp.status()));
        }

        #[derive(Deserialize)]
        struct PickDirectoryResponse {
            path: String,
        }

        resp.json::<PickDirectoryResponse>()
            .await
            .map(|r| r.path)
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get dashboard configuration
    pub async fn get_config(&self) -> Result<DashboardConfigDto, String> {
        let url = format!("{}/api/config", self.base_url);

        Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Save dashboard configuration
    pub async fn save_config(&self, config: &DashboardConfigDto) -> Result<(), String> {
        let url = format!("{}/api/config", self.base_url);

        Request::post(&url)
            .json(config)
            .map_err(|e| format!("Failed to serialize request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        Ok(())
    }

    // === Project Management ===

    /// Register a new project
    pub async fn register_project(&self, name: &str, path: &str) -> Result<ProjectInfoDto, String> {
        let url = format!("{}/api/projects/register", self.base_url);
        #[derive(Serialize)]
        struct RegisterReq<'a> {
            name: &'a str,
            path: &'a str,
        }

        Request::post(&url)
            .json(&RegisterReq { name, path })
            .map_err(|e| format!("Failed to serialize request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// List all registered projects
    pub async fn list_projects(&self) -> Result<ProjectListDto, String> {
        let url = format!("{}/api/projects", self.base_url);
        Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get project analysis history
    pub async fn get_project_history(&self, project_id: &str) -> Result<ProjectHistoryDto, String> {
        let url = format!("{}/api/projects/{}/history", self.base_url, project_id);
        Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get drift events with optional filters — GET /api/drift
    pub async fn get_drift_events(
        &self,
        project_path: &str,
        file: Option<&str>,
        function: Option<&str>,
        severity: Option<&str>,
        min_score: Option<f64>,
        offset: usize,
        limit: usize,
    ) -> Result<DriftResponseDto, String> {
        let mut url = format!("{}/api/drift", self.base_url);

        // Build query params
        let mut params: Vec<String> = vec![
            format!("project_path={}", urlencoding::encode(project_path)),
            format!("offset={}", offset),
            format!("limit={}", limit),
        ];

        if let Some(f) = file {
            if !f.is_empty() {
                params.push(format!("file={}", urlencoding::encode(f)));
            }
        }

        if let Some(fn_) = function {
            if !fn_.is_empty() {
                params.push(format!("function={}", urlencoding::encode(fn_)));
            }
        }

        if let Some(sev) = severity {
            if !sev.is_empty() {
                params.push(format!("severity={}", urlencoding::encode(sev)));
            }
        }

        if let Some(score) = min_score {
            params.push(format!("min_score={}", score));
        }

        url = format!("{}?{}", url, params.join("&"));

        Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get contracts — GET /api/contracts
    pub async fn get_contracts(
        &self,
        project_path: &str,
        limit: usize,
    ) -> Result<Vec<ContractDto>, String> {
        let mut url = format!("{}/api/contracts", self.base_url);

        let params = format!(
            "project_path={}&limit={}",
            urlencoding::encode(project_path),
            limit
        );
        url = format!("{}?{}", url, params);

        Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get agent stats — GET /api/agent-stats
    pub async fn get_agent_stats(
        &self,
        project_path: &str,
        since: Option<&str>,
    ) -> Result<Vec<AgentStatDto>, String> {
        let mut url = format!("{}/api/agent-stats", self.base_url);

        let params = format!("project_path={}", urlencoding::encode(project_path));
        if let Some(since) = since {
            url = format!("{}?{}&since={}", url, params, urlencoding::encode(since));
        } else {
            url = format!("{}?{}", url, params);
        }

        Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get trends — GET /api/trends
    pub async fn get_trends(&self, project_path: &str, limit: Option<usize>) -> Result<TrendsResponseDto, String> {
        let mut url = format!("{}/api/trends", self.base_url);

        let params = format!("project_path={}", urlencoding::encode(project_path));
        if let Some(l) = limit {
            url = format!("{}?{}&limit={}", url, params, l);
        } else {
            url = format!("{}?{}", url, params);
        }

        Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Create an agent task — POST /api/tasks
    pub async fn create_task(&self, request: &CreateTaskRequest) -> Result<CreateTaskResponse, String> {
        let url = format!("{}/api/tasks", self.base_url);

        Request::post(&url)
            .json(request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// List agent tasks — GET /api/tasks
    pub async fn list_tasks(
        &self,
        status: Option<&str>,
        task_type: Option<&str>,
    ) -> Result<TaskListResponse, String> {
        let mut url = format!("{}/api/tasks", self.base_url);
        let mut params: Vec<String> = Vec::new();

        if let Some(s) = status {
            if !s.is_empty() {
                params.push(format!("status={}", urlencoding::encode(s)));
            }
        }
        if let Some(t) = task_type {
            if !t.is_empty() {
                params.push(format!("task_type={}", urlencoding::encode(t)));
            }
        }

        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }

        Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_summary_dto_deserialize() {
        let json = r#"{
            "project_path": "/test/project",
            "total_files": 100,
            "total_issues": 50,
            "ratings": {
                "reliability": "A",
                "security": "B",
                "maintainability": "B",
                "coverage": "C"
            },
            "metrics": {
                "ncloc": 5000,
                "functions": 200,
                "code_smells": 30,
                "bugs": 5,
                "vulnerabilities": 2,
                "issues_by_severity": {
                    "blocker": 0,
                    "critical": 2
                }
            },
            "technical_debt": {
                "total_minutes": 120,
                "rating": "B",
                "label": "2h"
            },
            "quality_gate": {
                "name": "SonarQube Way",
                "status": "PASSED",
                "conditions": []
            },
            "incremental": {
                "files_total": 100,
                "files_changed": 10,
                "files_reused": 90,
                "new_code_issues": 3,
                "legacy_issues": 47,
                "clean_as_you_code": true,
                "timed_out": false
            }
        }"#;

        let dto: AnalysisSummaryDto = serde_json::from_str(json).unwrap();
        assert_eq!(dto.project_path, "/test/project");
        assert_eq!(dto.total_issues, 50);
        assert_eq!(dto.ratings.reliability, 'A');
    }

    #[test]
    fn test_drift_event_dto_deserialize() {
        let json = r#"{
            "id": 42,
            "timestamp": "2026-05-07T10:30:00Z",
            "file_path": "src/main.rs",
            "function_name": "process_data",
            "drift_score": 0.75,
            "intent": "Refactoring needed",
            "severity": "major"
        }"#;

        let dto: DriftEventDto = serde_json::from_str(json).unwrap();
        assert_eq!(dto.id, 42);
        assert_eq!(dto.timestamp, "2026-05-07T10:30:00Z");
        assert_eq!(dto.file_path, "src/main.rs");
        assert_eq!(dto.function_name, "process_data");
        assert_eq!(dto.drift_score, 0.75);
        assert_eq!(dto.intent, Some("Refactoring needed".to_string()));
        assert_eq!(dto.severity, "major");
    }

    #[test]
    fn test_drift_event_dto_without_intent() {
        let json = r#"{
            "id": 99,
            "timestamp": "2026-05-07T12:00:00Z",
            "file_path": "lib.rs",
            "function_name": "helper",
            "drift_score": 0.25,
            "intent": null,
            "severity": "minor"
        }"#;

        let dto: DriftEventDto = serde_json::from_str(json).unwrap();
        assert_eq!(dto.id, 99);
        assert_eq!(dto.intent, None);
        assert_eq!(dto.drift_score, 0.25);
    }

    #[test]
    fn test_drift_response_dto_deserialize() {
        let json = r#"{
            "events": [
                {
                    "id": 1,
                    "timestamp": "2026-05-07T10:00:00Z",
                    "file_path": "a.rs",
                    "function_name": "func_a",
                    "drift_score": 0.8,
                    "intent": "Test",
                    "severity": "critical"
                },
                {
                    "id": 2,
                    "timestamp": "2026-05-07T11:00:00Z",
                    "file_path": "b.rs",
                    "function_name": "func_b",
                    "drift_score": 0.5,
                    "intent": null,
                    "severity": "major"
                }
            ],
            "total_count": 10,
            "offset": 0,
            "limit": 2
        }"#;

        let dto: DriftResponseDto = serde_json::from_str(json).unwrap();
        assert_eq!(dto.events.len(), 2);
        assert_eq!(dto.total_count, 10);
        assert_eq!(dto.offset, 0);
        assert_eq!(dto.limit, 2);
        assert_eq!(dto.events[0].id, 1);
        assert_eq!(dto.events[1].id, 2);
    }

    #[test]
    fn test_get_drift_events_url_construction() {
        // Verify URL construction with query params using a local helper
        // that mirrors the get_drift_events logic without making HTTP calls
        let base_url = "http://localhost:3000";
        let project_path = "/test/project";
        let file = Some("src/main.rs");
        let function = Some("process");
        let severity = Some("major");
        let min_score = Some(0.5);
        let offset = 10;
        let limit = 20;

        // Reconstruct URL building logic from get_drift_events
        let mut params: Vec<String> = vec![
            format!("project_path={}", urlencoding::encode(project_path)),
            format!("offset={}", offset),
            format!("limit={}", limit),
        ];
        if let Some(f) = file {
            if !f.is_empty() {
                params.push(format!("file={}", urlencoding::encode(f)));
            }
        }
        if let Some(fn_) = function {
            if !fn_.is_empty() {
                params.push(format!("function={}", urlencoding::encode(fn_)));
            }
        }
        if let Some(sev) = severity {
            if !sev.is_empty() {
                params.push(format!("severity={}", urlencoding::encode(sev)));
            }
        }
        if let Some(score) = min_score {
            params.push(format!("min_score={}", score));
        }
        let url = format!("{}/api/drift?{}", base_url, params.join("&"));

        assert_eq!(url, "http://localhost:3000/api/drift?project_path=%2Ftest%2Fproject&offset=10&limit=20&file=src%2Fmain.rs&function=process&severity=major&min_score=0.5");
        // Verify param count
        assert_eq!(params.len(), 7);
    }

    #[test]
    fn test_contract_dto_deserialize() {
        let json = r#"{
            "id": 42,
            "source_file": "src/auth/validator.rs",
            "function_name": "validate_token",
            "compliance_score": 0.85,
            "generated_at": "2026-05-07T10:30:00Z"
        }"#;

        let dto: ContractDto = serde_json::from_str(json).unwrap();
        assert_eq!(dto.id, 42);
        assert_eq!(dto.source_file, "src/auth/validator.rs");
        assert_eq!(dto.function_name, "validate_token");
        assert_eq!(dto.compliance_score, 0.85);
        assert_eq!(dto.generated_at, "2026-05-07T10:30:00Z");
    }

    #[test]
    fn test_contract_dto_multiple_contracts() {
        let json = r#"[
            {
                "id": 1,
                "source_file": "a.rs",
                "function_name": "func_a",
                "compliance_score": 0.9,
                "generated_at": "2026-05-07T10:00:00Z"
            },
            {
                "id": 2,
                "source_file": "b.rs",
                "function_name": "func_b",
                "compliance_score": 0.75,
                "generated_at": "2026-05-07T11:00:00Z"
            }
        ]"#;

        let dtos: Vec<ContractDto> = serde_json::from_str(json).unwrap();
        assert_eq!(dtos.len(), 2);
        assert_eq!(dtos[0].id, 1);
        assert_eq!(dtos[0].compliance_score, 0.9);
        assert_eq!(dtos[1].id, 2);
        assert_eq!(dtos[1].compliance_score, 0.75);
    }

    #[test]
    fn test_get_contracts_url_construction() {
        let base_url = "http://localhost:3000";
        let project_path = "/test/project";
        let limit = 50;

        let url = format!(
            "{}/api/contracts?project_path={}&limit={}",
            base_url,
            urlencoding::encode(project_path),
            limit
        );

        assert_eq!(url, "http://localhost:3000/api/contracts?project_path=%2Ftest%2Fproject&limit=50");
    }

    #[test]
    fn test_agent_stat_dto_deserialize() {
        let json = r#"{
            "tool_name": "grep",
            "count": 42,
            "avg_duration_ms": 15.3,
            "result_status_breakdown": {
                "success": 30,
                "error": 8,
                "other": 4
            }
        }"#;

        let dto: AgentStatDto = serde_json::from_str(json).unwrap();
        assert_eq!(dto.tool_name, "grep");
        assert_eq!(dto.count, 42);
        assert_eq!(dto.avg_duration_ms, 15.3);
        assert_eq!(dto.result_status_breakdown.success, 30);
        assert_eq!(dto.result_status_breakdown.error, 8);
        assert_eq!(dto.result_status_breakdown.other, 4);
    }

    #[test]
    fn test_agent_stat_dto_empty_response() {
        let json = r#"[]"#;
        let dtos: Vec<AgentStatDto> = serde_json::from_str(json).unwrap();
        assert_eq!(dtos.len(), 0);
    }

    #[test]
    fn test_agent_stat_dto_multiple_stats() {
        let json = r#"[
            {
                "tool_name": "grep",
                "count": 100,
                "avg_duration_ms": 10.5,
                "result_status_breakdown": {
                    "success": 80,
                    "error": 15,
                    "other": 5
                }
            },
            {
                "tool_name": "file_search",
                "count": 50,
                "avg_duration_ms": 25.0,
                "result_status_breakdown": {
                    "success": 45,
                    "error": 3,
                    "other": 2
                }
            }
        ]"#;

        let dtos: Vec<AgentStatDto> = serde_json::from_str(json).unwrap();
        assert_eq!(dtos.len(), 2);
        assert_eq!(dtos[0].tool_name, "grep");
        assert_eq!(dtos[0].count, 100);
        assert_eq!(dtos[1].tool_name, "file_search");
        assert_eq!(dtos[1].count, 50);
    }

    #[test]
    fn test_get_agent_stats_url_construction_without_since() {
        let base_url = "http://localhost:3000";
        let project_path = "/test/project";

        let url = format!(
            "{}/api/agent-stats?project_path={}",
            base_url,
            urlencoding::encode(project_path)
        );

        assert_eq!(url, "http://localhost:3000/api/agent-stats?project_path=%2Ftest%2Fproject");
    }

    #[test]
    fn test_get_agent_stats_url_construction_with_since() {
        let base_url = "http://localhost:3000";
        let project_path = "/test/project";
        let since = "2026-05-01T00:00:00Z";

        let url = format!(
            "{}/api/agent-stats?project_path={}&since={}",
            base_url,
            urlencoding::encode(project_path),
            urlencoding::encode(since)
        );

        assert_eq!(url, "http://localhost:3000/api/agent-stats?project_path=%2Ftest%2Fproject&since=2026-05-01T00%3A00%3A00Z");
    }
}