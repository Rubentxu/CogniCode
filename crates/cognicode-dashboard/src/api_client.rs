//! API Client Module
//!
//! HTTP client for calling the CogniCode server endpoints.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    base_url: String,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
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
}