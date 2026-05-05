//! Analysis API - functions for project analysis

use crate::state::{ProjectRatings, TechnicalDebt};
use serde::{Deserialize, Serialize};

/// Analysis request parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisRequest {
    pub project_path: String,
    pub rule_profile: Option<String>,
    pub include_test_files: bool,
    pub analyze_dependencies: bool,
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
    pub issues: Vec<crate::state::IssueResult>,
    pub quality_gate: crate::state::QualityGateResult,
}

/// Get analysis summary for a project
pub async fn get_analysis_summary(project_path: String) -> Result<AnalysisSummary, String> {
    // TODO: Integrate with cognicode-quality for actual analysis
    // For now, return mock data

    Ok(AnalysisSummary {
        project_path,
        timestamp: chrono::Utc::now().to_rfc3339(),
        lines_of_code: 847,
        ratings: ProjectRatings {
            reliability: 'A',
            security: 'B',
            maintainability: 'B',
            coverage: 'C',
        },
        technical_debt: TechnicalDebt {
            total_minutes: 245,
            rating: 'C',
            label: "2h 45min".to_string(),
        },
        total_issues: 50,
        blocker_issues: 0,
        critical_issues: 2,
        major_issues: 15,
        minor_issues: 28,
        info_issues: 5,
    })
}

/// Run full analysis on a project
pub async fn run_analysis(request: AnalysisRequest) -> Result<AnalysisResult, String> {
    // TODO: Integrate with cognicode-quality for actual analysis
    // For now, return mock data

    use crate::state::{Category, GateCondition, IssueResult, QualityGateResult, Severity};

    let summary = AnalysisSummary {
        project_path: request.project_path,
        timestamp: chrono::Utc::now().to_rfc3339(),
        lines_of_code: 847,
        ratings: ProjectRatings {
            reliability: 'A',
            security: 'B',
            maintainability: 'B',
            coverage: 'C',
        },
        technical_debt: TechnicalDebt {
            total_minutes: 245,
            rating: 'C',
            label: "2h 45min".to_string(),
        },
        total_issues: 50,
        blocker_issues: 0,
        critical_issues: 2,
        major_issues: 15,
        minor_issues: 28,
        info_issues: 5,
    };

    let issues = vec![
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
            rule_id: "java:S1135".to_string(),
            message: "Complete the task implementation to avoid code smell.".to_string(),
            severity: Severity::Info,
            category: Category::Maintainability,
            file: "src/main/java/com/example/Controller.java".to_string(),
            line: 78,
            column: Some(5),
            end_line: Some(78),
            remediation_hint: None,
        },
    ];

    let quality_gate = QualityGateResult {
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
    };

    Ok(AnalysisResult {
        summary,
        issues,
        quality_gate,
    })
}
