//! Quality Snapshot and Delta Comparison
//!
//! Captures quality metrics at a point in time and compares snapshots to detect regressions.

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::services::ComplexityCalculator;

use crate::quality::connascence::ConnascenceAnalyzer;
use crate::quality::lcom::LcomCalculator;
use crate::quality::solid::SolidChecker;

/// Snapshot of quality metrics at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualitySnapshot {
    /// Timestamp of the snapshot
    pub timestamp: DateTime<Utc>,
    /// LCOM scores by type name
    pub lcom_scores: HashMap<String, f64>,
    /// SOLID scores
    pub solid_scores: SolidScoresSnapshot,
    /// Connascence report
    pub connascence_report: ConnascenceReportSnapshot,
    /// Complexity summary
    pub complexity_summary: ComplexitySummary,
    /// Total types analyzed
    pub total_types: usize,
    /// Total modules analyzed
    pub total_modules: usize,
}

impl QualitySnapshot {
    /// Create a snapshot from a call graph
    pub fn from_graph(graph: &CallGraph) -> Self {
        let timestamp = Utc::now();

        // Calculate LCOM
        let lcom_calc = LcomCalculator::new();
        let lcom_results = lcom_calc.calculate_all(graph);
        let lcom_scores: HashMap<String, f64> = lcom_results
            .into_iter()
            .map(|(name, result)| (name, result.lcom_score))
            .collect();

        // Calculate SOLID
        let solid_checker = SolidChecker::new();
        let solid_report = solid_checker.check_all(graph);

        let solid_scores = SolidScoresSnapshot {
            srp_score: solid_report.scores.srp_score,
            ocp_score: solid_report.scores.ocp_score,
            lsp_score: solid_report.scores.lsp_score,
            isp_score: solid_report.scores.isp_score,
            dip_score: solid_report.scores.dip_score,
            overall: solid_report.scores.overall,
            total_violations: solid_report.violations.len(),
        };

        // Calculate Connascence
        let conn_analyzer = ConnascenceAnalyzer::new();
        let conn_report = conn_analyzer.analyze(graph);

        let connascence_report = ConnascenceReportSnapshot {
            coupling_score: conn_report.coupling_score,
            total_violations: conn_report.violations.len(),
            by_type: conn_report
                .by_type
                .iter()
                .map(|(k, v)| (format!("{:?}", k), *v))
                .collect(),
        };

        // Calculate complexity summary (using ComplexityCalculator for future enhancements)
        let _complexity_calc = ComplexityCalculator::new();
        let mut total_cyclomatic = 0u64;
        let mut max_cyclomatic = 0u32;
        let mut func_count = 0usize;

        for symbol in graph.symbols() {
            if symbol.is_callable() {
                func_count += 1;
                // Placeholder complexity - actual would need CFG
                // Using fan-out as a proxy for complexity
                let complexity = graph.fan_out(&cognicode_core::domain::aggregates::SymbolId::new(
                    symbol.fully_qualified_name(),
                )) as u32;
                total_cyclomatic += complexity as u64;
                max_cyclomatic = max_cyclomatic.max(complexity);
            }
        }

        let avg_cyclomatic = if func_count > 0 {
            total_cyclomatic as f64 / func_count as f64
        } else {
            0.0
        };

        let complexity_summary = ComplexitySummary {
            avg_cyclomatic,
            avg_cognitive: avg_cyclomatic * 0.8, // Approximate
            max_cyclomatic: max_cyclomatic as f64,
            files_analyzed: graph.modules().len(),
        };

        let total_types = lcom_scores.len();
        let total_modules = graph.modules().len();

        Self {
            timestamp,
            lcom_scores,
            solid_scores,
            connascence_report,
            complexity_summary,
            total_types,
            total_modules,
        }
    }
}

/// Snapshot of SOLID scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolidScoresSnapshot {
    pub srp_score: f64,
    pub ocp_score: f64,
    pub lsp_score: f64,
    pub isp_score: f64,
    pub dip_score: f64,
    pub overall: f64,
    pub total_violations: usize,
}

/// Snapshot of connascence report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnascenceReportSnapshot {
    pub coupling_score: f64,
    pub total_violations: usize,
    pub by_type: HashMap<String, usize>,
}

/// Summary of complexity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexitySummary {
    pub avg_cyclomatic: f64,
    pub avg_cognitive: f64,
    pub max_cyclomatic: f64,
    pub files_analyzed: usize,
}

/// Delta between two quality snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityDelta {
    pub before: QualitySnapshot,
    pub after: QualitySnapshot,
    pub changes: Vec<QualityChange>,
    pub overall_score: f64,
}

impl QualityDelta {
    /// Compare two snapshots and produce a delta report
    pub fn compare(before: &QualitySnapshot, after: &QualitySnapshot) -> Self {
        let mut changes = Vec::new();

        // Compare LCOM scores
        for (type_name, before_score) in &before.lcom_scores {
            let after_score = after.lcom_scores.get(type_name).copied().unwrap_or(0.0);
            if (*before_score - after_score).abs() > 0.01 {
                changes.push(QualityChange {
                    metric: "lcom".to_string(),
                    target: type_name.clone(),
                    before: *before_score,
                    after: after_score,
                    delta: after_score - *before_score,
                    is_improvement: after_score < *before_score,
                });
            }
        }

        // Check for new types in LCOM
        for type_name in after.lcom_scores.keys() {
            if !before.lcom_scores.contains_key(type_name) {
                changes.push(QualityChange {
                    metric: "lcom".to_string(),
                    target: type_name.clone(),
                    before: 0.0,
                    after: *after.lcom_scores.get(type_name).unwrap(),
                    delta: *after.lcom_scores.get(type_name).unwrap(),
                    is_improvement: false,
                });
            }
        }

        // Compare SOLID scores
        changes.push(QualityChange {
            metric: "solid_srp".to_string(),
            target: "all".to_string(),
            before: before.solid_scores.srp_score,
            after: after.solid_scores.srp_score,
            delta: after.solid_scores.srp_score - before.solid_scores.srp_score,
            is_improvement: after.solid_scores.srp_score < before.solid_scores.srp_score,
        });

        changes.push(QualityChange {
            metric: "solid_overall".to_string(),
            target: "all".to_string(),
            before: before.solid_scores.overall,
            after: after.solid_scores.overall,
            delta: after.solid_scores.overall - before.solid_scores.overall,
            is_improvement: after.solid_scores.overall < before.solid_scores.overall,
        });

        // Compare coupling score
        changes.push(QualityChange {
            metric: "coupling".to_string(),
            target: "all".to_string(),
            before: before.connascence_report.coupling_score,
            after: after.connascence_report.coupling_score,
            delta: after.connascence_report.coupling_score - before.connascence_report.coupling_score,
            is_improvement: after.connascence_report.coupling_score < before.connascence_report.coupling_score,
        });

        // Compare complexity
        changes.push(QualityChange {
            metric: "complexity_avg".to_string(),
            target: "all".to_string(),
            before: before.complexity_summary.avg_cyclomatic,
            after: after.complexity_summary.avg_cyclomatic,
            delta: after.complexity_summary.avg_cyclomatic - before.complexity_summary.avg_cyclomatic,
            is_improvement: after.complexity_summary.avg_cyclomatic < before.complexity_summary.avg_cyclomatic,
        });

        // Calculate overall score (positive = improvement)
        let improvement_count = changes.iter().filter(|c| c.is_improvement).count();
        let regression_count = changes.iter().filter(|c| !c.is_improvement && c.metric != "files_analyzed").count();

        let overall_score = if changes.is_empty() {
            0.0
        } else {
            (improvement_count as f64 - regression_count as f64) / changes.len() as f64
        };

        Self {
            before: before.clone(),
            after: after.clone(),
            changes,
            overall_score,
        }
    }

    /// Check if any changes constitute a regression
    pub fn is_regression(&self) -> bool {
        // A regression is when LCOM increases, coupling increases, or SOLID scores worsen
        self.changes.iter().any(|c| {
            !c.is_improvement
                && c.metric != "files_analyzed"
                && c.delta > 0.01
                && (c.metric.starts_with("lcom") || c.metric == "coupling" || c.metric.starts_with("solid"))
        })
    }

    /// Get summary of regressions
    pub fn regressions(&self) -> Vec<&QualityChange> {
        self.changes
            .iter()
            .filter(|c| {
                !c.is_improvement
                    && c.metric != "files_analyzed"
                    && c.delta > 0.01
            })
            .collect()
    }

    /// Get summary of improvements
    pub fn improvements(&self) -> Vec<&QualityChange> {
        self.changes.iter().filter(|c| c.is_improvement && c.delta.abs() > 0.01).collect()
    }
}

/// A single quality change between snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityChange {
    pub metric: String,
    pub target: String,
    pub before: f64,
    pub after: f64,
    pub delta: f64,
    pub is_improvement: bool,
}

impl QualityChange {
    /// Human-readable description of the change
    pub fn description(&self) -> String {
        let direction = if self.is_improvement {
            "improved"
        } else {
            "worsened"
        };

        format!(
            "{} for {}: {:.3} -> {:.3} ({})",
            self.metric, self.target, self.before, self.after, direction
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_snapshot() -> QualitySnapshot {
        let mut lcom_scores = HashMap::new();
        lcom_scores.insert("UserService".to_string(), 0.3);
        lcom_scores.insert("OrderService".to_string(), 0.5);

        QualitySnapshot {
            timestamp: Utc::now(),
            lcom_scores,
            solid_scores: SolidScoresSnapshot {
                srp_score: 0.2,
                ocp_score: 0.1,
                lsp_score: 0.0,
                isp_score: 0.15,
                dip_score: 0.1,
                overall: 0.11,
                total_violations: 3,
            },
            connascence_report: ConnascenceReportSnapshot {
                coupling_score: 0.25,
                total_violations: 2,
                by_type: HashMap::new(),
            },
            complexity_summary: ComplexitySummary {
                avg_cyclomatic: 2.5,
                avg_cognitive: 2.0,
                max_cyclomatic: 8.0,
                files_analyzed: 10,
            },
            total_types: 2,
            total_modules: 5,
        }
    }

    #[test]
    fn test_compare_no_change() {
        let before = create_test_snapshot();
        let after = before.clone();

        let delta = QualityDelta::compare(&before, &after);

        assert!(!delta.is_regression());
        assert_eq!(delta.changes.len(), 4); // lcom for 2 types + solid + coupling + complexity
    }

    #[test]
    fn test_compare_with_improvement() {
        let before = create_test_snapshot();
        let mut after = before.clone();
        after.lcom_scores.insert("UserService".to_string(), 0.1); // Improved

        let delta = QualityDelta::compare(&before, &after);

        let improvements: Vec<_> = delta.improvements();
        assert!(!improvements.is_empty());
    }

    #[test]
    fn test_compare_with_regression() {
        let before = create_test_snapshot();
        let mut after = before.clone();
        after.lcom_scores.insert("UserService".to_string(), 0.7); // Worsened

        let delta = QualityDelta::compare(&before, &after);

        assert!(delta.is_regression());
    }

    #[test]
    fn test_quality_change_description() {
        let change = QualityChange {
            metric: "lcom".to_string(),
            target: "UserService".to_string(),
            before: 0.3,
            after: 0.1,
            delta: -0.2,
            is_improvement: true,
        };

        let desc = change.description();
        assert!(desc.contains("improved"));
        assert!(desc.contains("UserService"));
    }

    #[test]
    fn test_snapshot_serialization() {
        let snapshot = create_test_snapshot();
        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: QualitySnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.total_types, 2);
        assert_eq!(deserialized.solid_scores.srp_score, 0.2);
    }

    #[test]
    fn test_delta_serialization() {
        let before = create_test_snapshot();
        let after = before.clone();
        let delta = QualityDelta::compare(&before, &after);

        let json = serde_json::to_string(&delta).unwrap();
        let deserialized: QualityDelta = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.changes.len(), delta.changes.len());
    }
}