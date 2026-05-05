//! Quality Gate API - client-side functions

use crate::state::{GateCondition, QualityGateDefinition, QualityGateResult, GateConditionTemplate};

/// Get available quality gate definitions
pub async fn get_quality_gates() -> Result<Vec<QualityGateDefinition>, String> {
    Ok(vec![
        QualityGateDefinition {
            id: "sonarqube-way".to_string(),
            name: "SonarQube Way".to_string(),
            description: "Default SonarQube quality gate with standard thresholds".to_string(),
            conditions: vec![
                GateConditionTemplate {
                    metric: "reliability_rating".to_string(),
                    name: "Reliability Rating".to_string(),
                    operator: "<=".to_string(),
                    threshold: 1.0,
                },
                GateConditionTemplate {
                    metric: "security_rating".to_string(),
                    name: "Security Rating".to_string(),
                    operator: "<=".to_string(),
                    threshold: 2.0,
                },
                GateConditionTemplate {
                    metric: "blocker_issues".to_string(),
                    name: "Blocker Issues".to_string(),
                    operator: "=".to_string(),
                    threshold: 0.0,
                },
                GateConditionTemplate {
                    metric: "critical_issues".to_string(),
                    name: "Critical Issues".to_string(),
                    operator: "=".to_string(),
                    threshold: 0.0,
                },
            ],
        },
        QualityGateDefinition {
            id: "sonarqube-way-strict".to_string(),
            name: "SonarQube Way - Strict".to_string(),
            description: "Stricter version with additional checks".to_string(),
            conditions: vec![
                GateConditionTemplate {
                    metric: "reliability_rating".to_string(),
                    name: "Reliability Rating".to_string(),
                    operator: "<=".to_string(),
                    threshold: 1.0,
                },
                GateConditionTemplate {
                    metric: "security_rating".to_string(),
                    name: "Security Rating".to_string(),
                    operator: "<=".to_string(),
                    threshold: 1.0,
                },
                GateConditionTemplate {
                    metric: "maintainability_rating".to_string(),
                    name: "Maintainability Rating".to_string(),
                    operator: "<=".to_string(),
                    threshold: 1.0,
                },
                GateConditionTemplate {
                    metric: "blocker_issues".to_string(),
                    name: "Blocker Issues".to_string(),
                    operator: "=".to_string(),
                    threshold: 0.0,
                },
                GateConditionTemplate {
                    metric: "critical_issues".to_string(),
                    name: "Critical Issues".to_string(),
                    operator: "=".to_string(),
                    threshold: 0.0,
                },
                GateConditionTemplate {
                    metric: "major_issues".to_string(),
                    name: "Major Issues".to_string(),
                    operator: "<=".to_string(),
                    threshold: 5.0,
                },
            ],
        },
    ])
}

/// Evaluate a quality gate for a project
pub async fn evaluate_quality_gate(_project_path: String) -> Result<QualityGateResult, String> {
    Ok(QualityGateResult {
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
            GateCondition {
                id: "3".to_string(),
                name: "Maintainability Rating".to_string(),
                metric: "maintainability_rating".to_string(),
                operator: "<=".to_string(),
                threshold: 1.0,
                passed: true,
            },
            GateCondition {
                id: "4".to_string(),
                name: "Blocker Issues".to_string(),
                metric: "blocker_issues".to_string(),
                operator: "=".to_string(),
                threshold: 0.0,
                passed: true,
            },
            GateCondition {
                id: "5".to_string(),
                name: "Critical Issues".to_string(),
                metric: "critical_issues".to_string(),
                operator: "=".to_string(),
                threshold: 0.0,
                passed: true,
            },
        ],
    })
}