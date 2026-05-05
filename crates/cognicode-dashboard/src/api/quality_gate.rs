//! Quality Gate API - functions for quality gate management

use crate::state::{GateCondition, QualityGateResult};
use serde::{Deserialize, Serialize};

/// Available quality gate definitions
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
        QualityGateDefinition {
            id: "security-defaults".to_string(),
            name: "Security Defaults".to_string(),
            description: "Security-focused quality gate".to_string(),
            conditions: vec![
                GateConditionTemplate {
                    metric: "security_rating".to_string(),
                    name: "Security Rating".to_string(),
                    operator: "<=".to_string(),
                    threshold: 1.0,
                },
                GateConditionTemplate {
                    metric: "vulnerabilities".to_string(),
                    name: "Vulnerabilities".to_string(),
                    operator: "=".to_string(),
                    threshold: 0.0,
                },
                GateConditionTemplate {
                    metric: "security_hotspots".to_string(),
                    name: "Security Hotspots".to_string(),
                    operator: "<=".to_string(),
                    threshold: 10.0,
                },
            ],
        },
        QualityGateDefinition {
            id: "production-prevents".to_string(),
            name: "Production Prevents".to_string(),
            description: "Strict gate to prevent production issues".to_string(),
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
                    metric: "coverage".to_string(),
                    name: "Coverage".to_string(),
                    operator: ">=".to_string(),
                    threshold: 80.0,
                },
                GateConditionTemplate {
                    metric: "duplicates".to_string(),
                    name: "Duplicates".to_string(),
                    operator: "<=".to_string(),
                    threshold: 3.0,
                },
            ],
        },
    ])
}

/// Evaluate a quality gate for a project
pub async fn evaluate_quality_gate(gate_id: String, _project_path: String) -> Result<QualityGateResult, String> {
    // TODO: Integrate with cognicode-quality for actual evaluation
    // For now, return mock result

    let conditions = match gate_id.as_str() {
        "sonarqube-way" => vec![
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
        _ => vec![
            GateCondition {
                id: "1".to_string(),
                name: "Gate Check".to_string(),
                metric: "overall".to_string(),
                operator: "=".to_string(),
                threshold: 0.0,
                passed: true,
            },
        ],
    };

    let all_passed = conditions.iter().all(|c| c.passed);

    Ok(QualityGateResult {
        name: gate_id,
        status: if all_passed { "PASSED".to_string() } else { "FAILED".to_string() },
        conditions,
    })
}
