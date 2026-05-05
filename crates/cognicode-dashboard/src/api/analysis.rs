//! Analysis API - client-side functions
//!
//! These functions provide analysis data. Currently using mock data.
//! Can be extended to use real HTTP calls when server is available.

use crate::state::{AnalysisRequest, AnalysisResult, AnalysisSummary, Severity, Category, IssueResult, GateCondition, QualityGateResult};

/// Run full analysis on a project
pub async fn run_analysis(_request: AnalysisRequest) -> Result<AnalysisResult, String> {
    // Return mock data for now
    Ok(create_mock_analysis_result())
}

/// Get analysis summary for a project
pub async fn get_analysis_summary(project_path: String) -> Result<AnalysisSummary, String> {
    Ok(AnalysisSummary {
        project_path,
        timestamp: chrono::Utc::now().to_rfc3339(),
        lines_of_code: 0,
        ratings: crate::state::ProjectRatings {
            reliability: '-',
            security: '-',
            maintainability: '-',
            coverage: '-',
        },
        technical_debt: crate::state::TechnicalDebt {
            total_minutes: 0,
            rating: '-',
            label: "N/A".to_string(),
        },
        total_issues: 0,
        blocker_issues: 0,
        critical_issues: 0,
        major_issues: 0,
        minor_issues: 0,
        info_issues: 0,
    })
}

fn create_mock_analysis_result() -> AnalysisResult {
    AnalysisResult {
        summary: AnalysisSummary {
            project_path: "Mock Project".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            lines_of_code: 847,
            ratings: crate::state::ProjectRatings {
                reliability: 'A',
                security: 'B',
                maintainability: 'B',
                coverage: 'C',
            },
            technical_debt: crate::state::TechnicalDebt {
                total_minutes: 245,
                rating: 'C',
                label: "4h 5min".to_string(),
            },
            total_issues: 50,
            blocker_issues: 0,
            critical_issues: 2,
            major_issues: 15,
            minor_issues: 28,
            info_issues: 5,
        },
        issues: vec![
            IssueResult {
                rule_id: "java:S1130".to_string(),
                message: "Replace this generic exception declaration with a more specific one.".to_string(),
                severity: Severity::Minor,
                category: Category::Maintainability,
                file: "src/main/java/com/example/Service.java".to_string(),
                line: 42,
                column: Some(13),
                end_line: Some(42),
                remediation_hint: Some("Consider using IllegalArgumentException or a custom exception".to_string()),
            },
            IssueResult {
                rule_id: "java:S3752".to_string(),
                message: "This URL should be parameterised to prevent SQL injection.".to_string(),
                severity: Severity::Major,
                category: Category::Security,
                file: "src/main/java/com/example/Repository.java".to_string(),
                line: 156,
                column: Some(20),
                end_line: Some(156),
                remediation_hint: Some("Use PreparedStatement or a framework that handles parameterisation".to_string()),
            },
        ],
        quality_gate: QualityGateResult {
            name: "SonarQube Way".to_string(),
            status: "PASSED".to_string(),
            conditions: vec![
                GateCondition {
                    id: "1".to_string(),
                    name: "Reliability Rating".to_string(),
                    metric: "reliability_rating".to_string(),
                    operator: "<=".to_string(),
                    threshold: 1.0,
                    passed: true,
                },
                GateCondition {
                    id: "2".to_string(),
                    name: "Security Rating".to_string(),
                    metric: "security_rating".to_string(),
                    operator: "<=".to_string(),
                    threshold: 2.0,
                    passed: true,
                },
            ],
        },
    }
}