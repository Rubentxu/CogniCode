//! Quality Gate System
//!
//! Implements section 4 of doc 09: Quality Gates that evaluate metrics against
//! configurable conditions to determine if a build should pass or fail.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::rules::types::{Category, Severity};

/// Comparison operators for gate conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CompareOperator {
    GT,   // Greater than
    GTE,  // Greater than or equal
    LT,   // Less than
    LTE,  // Less than or equal
    EQ,   // Equal
    NEQ,  // Not equal
}

impl CompareOperator {
    /// Evaluate the comparison: `left op right`
    pub fn evaluate<T: PartialOrd>(&self, left: T, right: T) -> bool {
        match self {
            CompareOperator::GT => left > right,
            CompareOperator::GTE => left >= right,
            CompareOperator::LT => left < right,
            CompareOperator::LTE => left <= right,
            CompareOperator::EQ => left == right,
            CompareOperator::NEQ => left != right,
        }
    }
}

/// A metric value that can be compared
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MetricValue {
    Integer(i64),
    Float(f64),
    Percentage(f64),  // 0.0 to 100.0
}

impl MetricValue {
    /// Compare two metric values. Returns None if types don't match.
    pub fn compare(&self, op: CompareOperator, other: &MetricValue) -> Option<bool> {
        match (self, other) {
            (MetricValue::Integer(a), MetricValue::Integer(b)) => {
                Some(op.evaluate(*a, *b))
            }
            (MetricValue::Float(a), MetricValue::Float(b)) => {
                Some(op.evaluate(*a, *b))
            }
            (MetricValue::Percentage(a), MetricValue::Percentage(b)) => {
                Some(op.evaluate(*a, *b))
            }
            // Allow integer to float comparison
            (MetricValue::Integer(a), MetricValue::Float(b)) => {
                Some(op.evaluate(*a as f64, *b))
            }
            (MetricValue::Float(a), MetricValue::Integer(b)) => {
                Some(op.evaluate(*a, *b as f64))
            }
            _ => None,
        }
    }
}

/// Project-level metrics for gate evaluation
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ProjectMetrics {
    /// Lines of code
    pub ncloc: usize,
    /// Number of functions
    pub functions: usize,
    /// Number of classes/structs
    pub classes: usize,
    /// Total cyclomatic complexity
    pub complexity: u32,
    /// Number of code smell issues
    pub code_smells: usize,
    /// Number of bugs
    pub bugs: usize,
    /// Number of vulnerabilities
    pub vulnerabilities: usize,
    /// Number of security hotspots
    pub security_hotspots: usize,
    /// Technical debt ratio (debt / total time)
    pub debt_ratio: f64,
    /// Maintainability rating (A-E)
    pub maintainability_rating: char,
    /// Duplication percentage (0.0-100.0)
    pub duplication_percentage: f64,
    /// Coverage percentage if available (0.0-100.0)
    pub coverage_percentage: Option<f64>,
    /// Issues by severity
    pub issues_by_severity: HashMap<Severity, usize>,
    /// Issues by category
    pub issues_by_category: HashMap<Category, usize>,
}

impl ProjectMetrics {
    /// Create a new empty metrics struct
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a metric value by name
    pub fn get(&self, name: &str) -> Option<MetricValue> {
        match name {
            "ncloc" => Some(MetricValue::Integer(self.ncloc as i64)),
            "functions" => Some(MetricValue::Integer(self.functions as i64)),
            "classes" => Some(MetricValue::Integer(self.classes as i64)),
            "complexity" => Some(MetricValue::Integer(self.complexity as i64)),
            "code_smells" => Some(MetricValue::Integer(self.code_smells as i64)),
            "bugs" => Some(MetricValue::Integer(self.bugs as i64)),
            "vulnerabilities" => Some(MetricValue::Integer(self.vulnerabilities as i64)),
            "security_hotspots" => Some(MetricValue::Integer(self.security_hotspots as i64)),
            "debt_ratio" => Some(MetricValue::Percentage(self.debt_ratio * 100.0)),
            "duplication_percentage" => Some(MetricValue::Percentage(self.duplication_percentage)),
            "coverage" => self.coverage_percentage.map(MetricValue::Percentage),
            _ => None,
        }
    }
}

/// A single condition within a quality gate
#[derive(Debug, Clone)]
pub struct GateCondition {
    /// The metric to evaluate
    pub metric: String,
    /// The comparison operator
    pub operator: CompareOperator,
    /// The threshold value
    pub threshold: MetricValue,
    /// Optional severity filter (condition only applies if issue severity >= this)
    pub severity: Option<Severity>,
}

impl GateCondition {
    /// Create a new gate condition
    pub fn new(metric: impl Into<String>, operator: CompareOperator, threshold: MetricValue) -> Self {
        Self {
            metric: metric.into(),
            operator,
            threshold,
            severity: None,
        }
    }

    /// Set an optional severity filter
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = Some(severity);
        self
    }
}

/// A quality gate with one or more conditions
#[derive(Debug, Clone)]
pub struct QualityGate {
    /// Human-readable name
    pub name: String,
    /// Description of what this gate checks
    pub description: String,
    /// All conditions (ALL must pass for gate to pass)
    pub conditions: Vec<GateCondition>,
}

impl QualityGate {
    /// Create a new quality gate
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            conditions: Vec::new(),
        }
    }

    /// Add a condition to this gate
    pub fn add_condition(mut self, condition: GateCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Evaluate this gate against the given metrics
    pub fn evaluate(&self, metrics: &ProjectMetrics) -> QualityGateResult {
        let mut condition_results = Vec::new();
        let mut all_passed = true;
        let mut blocked = false;

        for condition in &self.conditions {
            let metric_value = metrics.get(&condition.metric);
            
            let result = match metric_value {
                Some(value) => {
                    match value.compare(condition.operator, &condition.threshold) {
                        Some(passed) => {
                            ConditionResult {
                                metric: condition.metric.clone(),
                                actual_value: Some(value),
                                threshold: condition.threshold.clone(),
                                operator: condition.operator,
                                passed,
                                blocked: false,
                                message: None,
                            }
                        }
                        None => {
                            ConditionResult {
                                metric: condition.metric.clone(),
                                actual_value: None,
                                threshold: condition.threshold.clone(),
                                operator: condition.operator,
                                passed: false,
                                blocked: true,
                                message: Some(format!(
                                    "Cannot compare metric '{}': type mismatch",
                                    condition.metric
                                )),
                            }
                        }
                    }
                }
                None => {
                    ConditionResult {
                        metric: condition.metric.clone(),
                        actual_value: None,
                        threshold: condition.threshold.clone(),
                        operator: condition.operator,
                        passed: false,
                        blocked: true,
                        message: Some(format!(
                            "Metric '{}' not found in project metrics",
                            condition.metric
                        )),
                    }
                }
            };

            if result.blocked {
                blocked = true;
            }
            if !result.passed {
                all_passed = false;
            }
            condition_results.push(result);
        }

        QualityGateResult {
            gate_name: self.name.clone(),
            passed: all_passed && !blocked,
            blocked,
            condition_results,
            evaluated_at: Utc::now(),
        }
    }
}

/// Result of evaluating a single condition
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConditionResult {
    /// The metric that was evaluated
    pub metric: String,
    /// The actual value observed (if available)
    pub actual_value: Option<MetricValue>,
    /// The threshold value
    pub threshold: MetricValue,
    /// The comparison operator used
    pub operator: CompareOperator,
    /// Whether the condition passed
    pub passed: bool,
    /// Whether evaluation was blocked (e.g., metric not found)
    pub blocked: bool,
    /// Optional message explaining the result
    pub message: Option<String>,
}

/// Result of evaluating an entire quality gate
#[derive(Debug, Clone, serde::Serialize)]
pub struct QualityGateResult {
    /// Name of the gate evaluated
    pub gate_name: String,
    /// Whether all conditions passed
    pub passed: bool,
    /// Whether evaluation was blocked by an error
    pub blocked: bool,
    /// Results of each condition
    pub condition_results: Vec<ConditionResult>,
    /// Timestamp of evaluation
    pub evaluated_at: DateTime<Utc>,
}

/// Evaluator for multiple quality gates
#[derive(Debug)]
pub struct QualityGateEvaluator {
    gates: Vec<QualityGate>,
}

impl QualityGateEvaluator {
    /// Create a new evaluator with the given gates
    pub fn new(gates: Vec<QualityGate>) -> Self {
        Self { gates }
    }

    /// Evaluate all gates against the given metrics
    pub fn evaluate_all(&self, metrics: &ProjectMetrics) -> Vec<QualityGateResult> {
        self.gates
            .iter()
            .map(|gate| gate.evaluate(metrics))
            .collect()
    }

    /// Check if all gates pass
    pub fn all_pass(&self, metrics: &ProjectMetrics) -> bool {
        self.evaluate_all(metrics)
            .iter()
            .all(|result| result.passed)
    }

    /// Get only the failed gates
    pub fn failed_gates<'a>(&self, results: &'a [QualityGateResult]) -> Vec<&'a QualityGateResult> {
        results.iter().filter(|r| !r.passed).collect()
    }
}

impl Default for QualityGateEvaluator {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_operators() {
        assert!(CompareOperator::GT.evaluate(5, 3));
        assert!(!CompareOperator::GT.evaluate(3, 5));
        assert!(CompareOperator::GTE.evaluate(5, 5));
        assert!(CompareOperator::LT.evaluate(3, 5));
        assert!(CompareOperator::LTE.evaluate(5, 5));
        assert!(CompareOperator::EQ.evaluate(5, 5));
        assert!(CompareOperator::NEQ.evaluate(5, 3));
    }

    #[test]
    fn test_metric_value_comparison() {
        assert_eq!(
            MetricValue::Integer(10)
                .compare(CompareOperator::GT, &MetricValue::Integer(5)),
            Some(true)
        );
        assert_eq!(
            MetricValue::Float(3.14)
                .compare(CompareOperator::EQ, &MetricValue::Float(3.14)),
            Some(true)
        );
        assert_eq!(
            MetricValue::Integer(5)
                .compare(CompareOperator::LT, &MetricValue::Float(10.0)),
            Some(true)
        );
    }

    #[test]
    fn test_quality_gate_pass() {
        let gate = QualityGate::new("Test Gate", "Test description")
            .add_condition(GateCondition::new(
                "complexity",
                CompareOperator::LT,
                MetricValue::Integer(20),
            ));

        let mut metrics = ProjectMetrics::new();
        metrics.complexity = 10;

        let result = gate.evaluate(&metrics);
        assert!(result.passed);
        assert!(!result.blocked);
    }

    #[test]
    fn test_quality_gate_fail() {
        let gate = QualityGate::new("Test Gate", "Test description")
            .add_condition(GateCondition::new(
                "complexity",
                CompareOperator::LT,
                MetricValue::Integer(20),
            ));

        let mut metrics = ProjectMetrics::new();
        metrics.complexity = 25;

        let result = gate.evaluate(&metrics);
        assert!(!result.passed);
        assert!(!result.blocked);
    }

    #[test]
    fn test_quality_gate_blocked() {
        let gate = QualityGate::new("Test Gate", "Test description")
            .add_condition(GateCondition::new(
                "nonexistent_metric",
                CompareOperator::LT,
                MetricValue::Integer(20),
            ));

        let metrics = ProjectMetrics::new();
        let result = gate.evaluate(&metrics);
        
        assert!(!result.passed);
        assert!(result.blocked);
    }

    #[test]
    fn test_gate_evaluator() {
        let gates = vec![
            QualityGate::new("Gate 1", "First gate")
                .add_condition(GateCondition::new(
                    "complexity",
                    CompareOperator::LT,
                    MetricValue::Integer(100),
                )),
            QualityGate::new("Gate 2", "Second gate")
                .add_condition(GateCondition::new(
                    "ncloc",
                    CompareOperator::LT,
                    MetricValue::Integer(10000),
                )),
        ];

        let evaluator = QualityGateEvaluator::new(gates);
        let metrics = ProjectMetrics::new();
        
        let results = evaluator.evaluate_all(&metrics);
        assert_eq!(results.len(), 2);
        assert!(evaluator.all_pass(&metrics));
    }
}
