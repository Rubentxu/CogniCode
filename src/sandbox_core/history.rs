//! Run History — JSONL append-only log of sandbox run results.
//!
//! Each line in runs.jsonl is a self-contained JSON object with
//! the MCP Health Score and per-dimension aggregates for a run.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use super::scoring::DimensionScores;

/// A single run entry in the JSONL history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEntry {
    /// ISO 8601 timestamp of the run
    pub timestamp: String,
    /// MCP Health Score (weighted average of dimensions, 0-100)
    pub health_score: f64,
    /// Per-dimension average scores
    pub dimensions: DimensionAverages,
    /// Number of scenarios in this run
    pub total_scenarios: u32,
    /// Number of passed scenarios
    pub passed_scenarios: u32,
    /// Pass rate
    pub pass_rate: f64,
    /// Orchestrator version
    pub orchestrator_version: String,
}

/// Average scores per dimension for a run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DimensionAverages {
    pub correctitud: f64,
    pub latencia: f64,
    pub escalabilidad: f64,
    pub consistencia: f64,
    pub robustez: f64,
}

/// Trend direction for a dimension.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrendDirection {
    Improving,
    Stable,
    Regressing,
    InsufficientData,
}

/// Trend analysis result comparing latest run to previous runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendReport {
    pub latest_timestamp: String,
    pub comparisons: HashMap<String, DimensionTrend>,
    pub health_score_trend: TrendDirection,
    pub health_score_change_pct: f64,
}

/// Per-dimension trend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionTrend {
    pub dimension: String,
    pub current: f64,
    pub previous_avg: f64,
    pub change_pct: f64,
    pub direction: TrendDirection,
}

/// Append a run entry to the JSONL history file.
/// Creates the file and parent directories if they don't exist.
pub fn append_run(history_path: &PathBuf, entry: &RunEntry) -> Result<(), String> {
    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path)
        .map_err(|e| e.to_string())?;
    let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    writeln!(file, "{}", line).map_err(|e| e.to_string())?;
    Ok(())
}

/// Read all run entries from the JSONL history file.
/// Returns empty vec if file doesn't exist or is empty.
pub fn read_history(history_path: &PathBuf) -> Result<Vec<RunEntry>, String> {
    if !history_path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(history_path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line_result in reader.lines() {
        let line = line_result.map_err(|e| e.to_string())?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<RunEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(_) => continue, // Skip malformed lines
        }
    }
    Ok(entries)
}

/// Compute trend analysis comparing the latest run to the previous N runs.
/// Returns "insufficient data" if fewer than 2 runs exist.
pub fn compute_trends(history_path: &PathBuf, n_previous: usize) -> Result<TrendReport, String> {
    let entries = read_history(history_path)?;
    if entries.len() < 2 {
        return Ok(TrendReport {
            latest_timestamp: entries
                .last()
                .map(|e| e.timestamp.clone())
                .unwrap_or_default(),
            comparisons: HashMap::new(),
            health_score_trend: TrendDirection::InsufficientData,
            health_score_change_pct: 0.0,
        });
    }

    let latest = entries.last().unwrap();
    let start = if entries.len() > n_previous {
        entries.len() - n_previous - 1
    } else {
        0
    };
    let previous: Vec<&RunEntry> = entries[start..entries.len() - 1].iter().collect();

    let mut comparisons = HashMap::new();

    // Compare each dimension
    for (key, current_val) in [
        ("correctitud", latest.dimensions.correctitud),
        ("latencia", latest.dimensions.latencia),
        ("escalabilidad", latest.dimensions.escalabilidad),
        ("consistencia", latest.dimensions.consistencia),
        ("robustez", latest.dimensions.robustez),
    ] {
        let prev_avg: f64 = match key {
            "correctitud" => {
                previous
                    .iter()
                    .map(|e| e.dimensions.correctitud)
                    .sum::<f64>()
                    / previous.len() as f64
            }
            "latencia" => {
                previous.iter().map(|e| e.dimensions.latencia).sum::<f64>() / previous.len() as f64
            }
            "escalabilidad" => {
                previous
                    .iter()
                    .map(|e| e.dimensions.escalabilidad)
                    .sum::<f64>()
                    / previous.len() as f64
            }
            "consistencia" => {
                previous
                    .iter()
                    .map(|e| e.dimensions.consistencia)
                    .sum::<f64>()
                    / previous.len() as f64
            }
            "robustez" => {
                previous.iter().map(|e| e.dimensions.robustez).sum::<f64>() / previous.len() as f64
            }
            _ => 0.0,
        };

        let change_pct = if prev_avg > 0.0 {
            ((current_val - prev_avg) / prev_avg) * 100.0
        } else {
            0.0
        };

        let direction = if change_pct > 2.0 {
            TrendDirection::Improving
        } else if change_pct < -2.0 {
            TrendDirection::Regressing
        } else {
            TrendDirection::Stable
        };

        comparisons.insert(
            key.to_string(),
            DimensionTrend {
                dimension: key.to_string(),
                current: current_val,
                previous_avg: prev_avg,
                change_pct,
                direction,
            },
        );
    }

    // Health score trend
    let prev_health_avg: f64 =
        previous.iter().map(|e| e.health_score).sum::<f64>() / previous.len() as f64;
    let health_change_pct = if prev_health_avg > 0.0 {
        ((latest.health_score - prev_health_avg) / prev_health_avg) * 100.0
    } else {
        0.0
    };
    let health_direction = if health_change_pct > 2.0 {
        TrendDirection::Improving
    } else if health_change_pct < -2.0 {
        TrendDirection::Regressing
    } else {
        TrendDirection::Stable
    };

    Ok(TrendReport {
        latest_timestamp: latest.timestamp.clone(),
        comparisons,
        health_score_trend: health_direction,
        health_score_change_pct: health_change_pct,
    })
}

/// Compute dimension averages from a list of scenario results.
/// Returns average scores per dimension, ignoring scenarios without scores.
pub fn compute_dimension_averages(
    dimension_scores_list: &[Option<DimensionScores>],
) -> DimensionAverages {
    // Count how many entries have each dimension and sum them
    let (mut corr_sum, mut corr_count) = (0.0, 0);
    let (mut lat_sum, mut lat_count) = (0.0, 0);
    let (mut esc_sum, mut esc_count) = (0.0, 0);
    let (mut con_sum, mut con_count) = (0.0, 0);
    let (mut rob_sum, mut rob_count) = (0.0, 0);

    for ds_opt in dimension_scores_list {
        if let Some(ds) = ds_opt {
            if let Some(v) = ds.correctitud {
                corr_sum += v;
                corr_count += 1;
            }
            if let Some(v) = ds.latencia {
                lat_sum += v;
                lat_count += 1;
            }
            if let Some(v) = ds.escalabilidad {
                esc_sum += v;
                esc_count += 1;
            }
            if let Some(v) = ds.consistencia {
                con_sum += v;
                con_count += 1;
            }
            if let Some(v) = ds.robustez {
                rob_sum += v;
                rob_count += 1;
            }
        }
    }

    DimensionAverages {
        correctitud: if corr_count > 0 {
            corr_sum / corr_count as f64
        } else {
            0.0
        },
        latencia: if lat_count > 0 {
            lat_sum / lat_count as f64
        } else {
            0.0
        },
        escalabilidad: if esc_count > 0 {
            esc_sum / esc_count as f64
        } else {
            0.0
        },
        consistencia: if con_count > 0 {
            con_sum / con_count as f64
        } else {
            0.0
        },
        robustez: if rob_count > 0 {
            rob_sum / rob_count as f64
        } else {
            0.0
        },
    }
}

/// Compute the MCP Health Score from dimension averages.
/// Uses weighted average: CORR×0.35 + LAT×0.20 + ESC×0.15 + CON×0.15 + ROB×0.15
/// Only includes dimensions that have measured values (> 0.0 or explicitly present).
/// Weights are re-normalized when some dimensions are absent.
pub fn compute_health_from_averages(dims: &DimensionAverages) -> f64 {
    let weights: [(&str, f64, f64); 5] = [
        ("correctitud", 0.35, dims.correctitud),
        ("latencia", 0.20, dims.latencia),
        ("escalabilidad", 0.15, dims.escalabilidad),
        ("consistencia", 0.15, dims.consistencia),
        ("robustez", 0.15, dims.robustez),
    ];

    let mut weighted_sum = 0.0;
    let mut total_weight = 0.0;

    for (_name, weight, score) in &weights {
        // Include dimension only if it was measured (score > 0.0)
        // A score of exactly 0.0 means "not measured" since compute_dimension_averages
        // returns 0.0 for dimensions with no contributing scenarios.
        if *score > 0.0 {
            weighted_sum += score * weight;
            total_weight += weight;
        }
    }

    if total_weight == 0.0 {
        return 0.0;
    }

    weighted_sum / total_weight
}

// =========================================================================
// Phase C4: Regression Alerting
// =========================================================================

/// Regression alert for a dimension that dropped significantly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionAlert {
    /// Dimension name (correctitud, latencia, etc.)
    pub dimension: String,
    /// Current score
    pub current_score: f64,
    /// Previous average score
    pub previous_score: f64,
    /// Absolute percentage drop
    pub drop_pct: f64,
    /// Human-readable alert message
    pub message: String,
}

/// Check for regressions (>5% drop) in any dimension.
/// Returns list of alerts (empty if no regressions detected).
pub fn check_regressions(trends: &TrendReport) -> Vec<RegressionAlert> {
    let threshold = 5.0;
    let mut alerts = Vec::new();

    for (dim_name, trend) in &trends.comparisons {
        if trend.change_pct < -threshold {
            alerts.push(RegressionAlert {
                dimension: dim_name.clone(),
                current_score: trend.current,
                previous_score: trend.previous_avg,
                drop_pct: trend.change_pct.abs(),
                message: format!(
                    "REGRESSION: {} dropped {:.1}% (was {:.1}, now {:.1})",
                    dim_name,
                    trend.change_pct.abs(),
                    trend.previous_avg,
                    trend.current
                ),
            });
        }
    }

    // Check overall health score
    if trends.health_score_change_pct < -threshold {
        alerts.push(RegressionAlert {
            dimension: "health_score".to_string(),
            current_score: 0.0,
            previous_score: 0.0,
            drop_pct: trends.health_score_change_pct.abs(),
            message: format!(
                "REGRESSION: MCP Health Score dropped {:.1}%",
                trends.health_score_change_pct.abs()
            ),
        });
    }

    alerts
}

// =========================================================================
// Phase C5: Improvement Prioritization
// =========================================================================

/// Improvement recommendation for a dimension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementTarget {
    /// Dimension name
    pub name: String,
    /// Current score (0-100)
    pub current_score: f64,
    /// Weight in health score calculation
    pub weight: f64,
    /// Potential health score gain if this dimension reaches 100
    pub potential_gain: f64,
    /// Human-readable recommendation
    pub recommendation: String,
}

/// Rank dimensions by improvement potential.
/// Formula: potential_gain = weight × (100 - current_score)
/// Returns top N targets sorted by potential_gain descending.
pub fn rank_improvement_targets(
    dimensions: &DimensionAverages,
    top_n: usize,
) -> Vec<ImprovementTarget> {
    let weights: [(&str, f64, f64); 5] = [
        ("correctitud", 0.35, dimensions.correctitud),
        ("latencia", 0.20, dimensions.latencia),
        ("escalabilidad", 0.15, dimensions.escalabilidad),
        ("consistencia", 0.15, dimensions.consistencia),
        ("robustez", 0.15, dimensions.robustez),
    ];

    let mut targets: Vec<ImprovementTarget> = weights
        .iter()
        .map(|(name, weight, score)| {
            let potential_gain = weight * (100.0 - score);
            let recommendation = if *score < 50.0 {
                format!(
                    "CRITICAL: {} is at {:.0}/100 — investigate failures",
                    name, score
                )
            } else if *score < 75.0 {
                format!(
                    "IMPROVE: {} is at {:.0}/100 — review and optimize",
                    name, score
                )
            } else if *score < 90.0 {
                format!(
                    "POLISH: {} is at {:.0}/100 — minor improvements possible",
                    name, score
                )
            } else {
                format!("GOOD: {} is at {:.0}/100 — maintain quality", name, score)
            };

            ImprovementTarget {
                name: name.to_string(),
                current_score: *score,
                weight: *weight,
                potential_gain,
                recommendation,
            }
        })
        .collect();

    targets.sort_by(|a, b| {
        b.potential_gain
            .partial_cmp(&a.potential_gain)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    targets.truncate(top_n);
    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_dimension_scores(
        corr: Option<f64>,
        lat: Option<f64>,
        esc: Option<f64>,
        con: Option<f64>,
        rob: Option<f64>,
    ) -> DimensionScores {
        DimensionScores {
            correctitud: corr,
            latencia: lat,
            escalabilidad: esc,
            consistencia: con,
            robustez: rob,
        }
    }

    fn make_run_entry(
        timestamp: &str,
        health: f64,
        corr: f64,
        lat: f64,
        esc: f64,
        con: f64,
        rob: f64,
    ) -> RunEntry {
        RunEntry {
            timestamp: timestamp.to_string(),
            health_score: health,
            dimensions: DimensionAverages {
                correctitud: corr,
                latencia: lat,
                escalabilidad: esc,
                consistencia: con,
                robustez: rob,
            },
            total_scenarios: 10,
            passed_scenarios: 8,
            pass_rate: 0.8,
            orchestrator_version: "1.0.0".to_string(),
        }
    }

    #[test]
    fn test_append_and_read_history() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join("runs.jsonl");

        let entry1 = make_run_entry("2024-01-01T00:00:00Z", 85.0, 90.0, 80.0, 85.0, 80.0, 75.0);
        let entry2 = make_run_entry("2024-01-02T00:00:00Z", 87.0, 92.0, 82.0, 86.0, 81.0, 76.0);

        append_run(&history_path, &entry1).unwrap();
        append_run(&history_path, &entry2).unwrap();

        let entries = read_history(&history_path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].health_score, 85.0);
        assert_eq!(entries[1].health_score, 87.0);
    }

    #[test]
    fn test_read_empty_history() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join("nonexistent.jsonl");

        let entries = read_history(&history_path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_read_malformed_line_skipped() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join("runs.jsonl");

        let mut file = std::fs::File::create(&history_path).unwrap();
        file.write_all(
            br#"{"timestamp": "2024-01-01T00:00:00Z", "health_score": 85.0, "dimensions": {"correctitud": 90.0, "latencia": 80.0, "escalabilidad": 85.0, "consistencia": 80.0, "robustez": 75.0}, "total_scenarios": 10, "passed_scenarios": 8, "pass_rate": 0.8, "orchestrator_version": "1.0.0"}"# 
        ).unwrap();
        file.write_all(b"\n").unwrap();
        file.write_all(b"this is not json").unwrap();
        file.write_all(b"\n").unwrap();
        file.write_all(
            br#"{"timestamp": "2024-01-02T00:00:00Z", "health_score": 87.0, "dimensions": {"correctitud": 92.0, "latencia": 82.0, "escalabilidad": 86.0, "consistencia": 81.0, "robustez": 76.0}, "total_scenarios": 10, "passed_scenarios": 9, "pass_rate": 0.9, "orchestrator_version": "1.0.0"}"#
        ).unwrap();
        file.write_all(b"\n").unwrap();

        let entries = read_history(&history_path).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_compute_trends_insufficient_data() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join("runs.jsonl");

        // No entries
        let report = compute_trends(&history_path, 5).unwrap();
        assert_eq!(report.health_score_trend, TrendDirection::InsufficientData);

        // One entry
        let entry = make_run_entry("2024-01-01T00:00:00Z", 85.0, 90.0, 80.0, 85.0, 80.0, 75.0);
        append_run(&history_path, &entry).unwrap();

        let report = compute_trends(&history_path, 5).unwrap();
        assert_eq!(report.health_score_trend, TrendDirection::InsufficientData);
    }

    #[test]
    fn test_compute_trends_improving() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join("runs.jsonl");

        // Three entries with increasing scores
        let entry1 = make_run_entry("2024-01-01T00:00:00Z", 80.0, 80.0, 80.0, 80.0, 80.0, 80.0);
        let entry2 = make_run_entry("2024-01-02T00:00:00Z", 82.0, 82.0, 82.0, 82.0, 82.0, 82.0);
        let entry3 = make_run_entry("2024-01-03T00:00:00Z", 90.0, 95.0, 85.0, 90.0, 88.0, 85.0);

        append_run(&history_path, &entry1).unwrap();
        append_run(&history_path, &entry2).unwrap();
        append_run(&history_path, &entry3).unwrap();

        let report = compute_trends(&history_path, 5).unwrap();
        assert_eq!(report.health_score_trend, TrendDirection::Improving);
        assert!(report.health_score_change_pct > 2.0);

        // Check dimension trends
        let corr_trend = report.comparisons.get("correctitud").unwrap();
        assert_eq!(corr_trend.direction, TrendDirection::Improving);
    }

    #[test]
    fn test_compute_trends_regressing() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join("runs.jsonl");

        // Third entry is worse than average of previous two
        let entry1 = make_run_entry("2024-01-01T00:00:00Z", 90.0, 90.0, 90.0, 90.0, 90.0, 90.0);
        let entry2 = make_run_entry("2024-01-02T00:00:00Z", 88.0, 88.0, 88.0, 88.0, 88.0, 88.0);
        let entry3 = make_run_entry("2024-01-03T00:00:00Z", 75.0, 75.0, 75.0, 75.0, 75.0, 75.0);

        append_run(&history_path, &entry1).unwrap();
        append_run(&history_path, &entry2).unwrap();
        append_run(&history_path, &entry3).unwrap();

        let report = compute_trends(&history_path, 5).unwrap();
        assert_eq!(report.health_score_trend, TrendDirection::Regressing);
        assert!(report.health_score_change_pct < -2.0);
    }

    #[test]
    fn test_compute_trends_stable() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join("runs.jsonl");

        // Scores within 2% - considered stable
        let entry1 = make_run_entry("2024-01-01T00:00:00Z", 85.0, 85.0, 85.0, 85.0, 85.0, 85.0);
        let entry2 = make_run_entry("2024-01-02T00:00:00Z", 86.0, 86.0, 86.0, 86.0, 86.0, 86.0);
        let entry3 = make_run_entry("2024-01-03T00:00:00Z", 85.5, 85.5, 85.5, 85.5, 85.5, 85.5);

        append_run(&history_path, &entry1).unwrap();
        append_run(&history_path, &entry2).unwrap();
        append_run(&history_path, &entry3).unwrap();

        let report = compute_trends(&history_path, 5).unwrap();
        assert_eq!(report.health_score_trend, TrendDirection::Stable);
    }

    #[test]
    fn test_compute_dimension_averages() {
        let scores_list = vec![
            Some(make_dimension_scores(
                Some(100.0),
                Some(80.0),
                Some(90.0),
                Some(85.0),
                Some(70.0),
            )),
            Some(make_dimension_scores(
                Some(80.0),
                Some(90.0),
                Some(85.0),
                Some(80.0),
                Some(75.0),
            )),
            Some(make_dimension_scores(
                Some(90.0),
                Some(85.0),
                Some(95.0),
                None,
                Some(80.0),
            )),
        ];

        let averages = compute_dimension_averages(&scores_list);

        assert_eq!(averages.correctitud, (100.0 + 80.0 + 90.0) / 3.0);
        assert_eq!(averages.latencia, (80.0 + 90.0 + 85.0) / 3.0);
        assert_eq!(averages.escalabilidad, (90.0 + 85.0 + 95.0) / 3.0);
        assert_eq!(averages.consistencia, (85.0 + 80.0) / 2.0); // Only 2 values (one None)
        assert_eq!(averages.robustez, (70.0 + 75.0 + 80.0) / 3.0);
    }

    #[test]
    fn test_compute_dimension_averages_all_none() {
        let scores_list: Vec<Option<DimensionScores>> = vec![None, None, None];

        let averages = compute_dimension_averages(&scores_list);

        assert_eq!(averages.correctitud, 0.0);
        assert_eq!(averages.latencia, 0.0);
        assert_eq!(averages.escalabilidad, 0.0);
        assert_eq!(averages.consistencia, 0.0);
        assert_eq!(averages.robustez, 0.0);
    }

    #[test]
    fn test_compute_health_from_averages() {
        let dims = DimensionAverages {
            correctitud: 100.0,
            latencia: 80.0,
            escalabilidad: 90.0,
            consistencia: 85.0,
            robustez: 75.0,
        };

        let health = compute_health_from_averages(&dims);

        // 100*0.35 + 80*0.20 + 90*0.15 + 85*0.15 + 75*0.15
        // = 35 + 16 + 13.5 + 12.75 + 11.25 = 88.5
        assert!((health - 88.5).abs() < 0.01);
    }

    // =========================================================================
    // Phase C4: Regression Alerting Tests
    // =========================================================================

    #[test]
    fn test_check_regressions_none() {
        let trends = TrendReport {
            latest_timestamp: "2026-01-01T00:00:00Z".into(),
            comparisons: HashMap::from([(
                "correctitud".into(),
                DimensionTrend {
                    dimension: "correctitud".into(),
                    current: 90.0,
                    previous_avg: 88.0,
                    change_pct: 2.3,
                    direction: TrendDirection::Stable,
                },
            )]),
            health_score_trend: TrendDirection::Stable,
            health_score_change_pct: 1.0,
        };
        let alerts = check_regressions(&trends);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_check_regressions_detected() {
        let trends = TrendReport {
            latest_timestamp: "2026-01-01T00:00:00Z".into(),
            comparisons: HashMap::from([(
                "correctitud".into(),
                DimensionTrend {
                    dimension: "correctitud".into(),
                    current: 80.0,
                    previous_avg: 92.0,
                    change_pct: -13.0,
                    direction: TrendDirection::Regressing,
                },
            )]),
            health_score_trend: TrendDirection::Regressing,
            health_score_change_pct: -8.0,
        };
        let alerts = check_regressions(&trends);
        assert_eq!(alerts.len(), 2); // correctitud + health_score
        assert_eq!(alerts[0].dimension, "correctitud");
        assert!((alerts[0].drop_pct - 13.0).abs() < 0.1);
    }

    #[test]
    fn test_check_regressions_below_threshold() {
        // 4% drop is below 5% threshold → no alert
        let trends = TrendReport {
            latest_timestamp: "2026-01-01T00:00:00Z".into(),
            comparisons: HashMap::from([(
                "latencia".into(),
                DimensionTrend {
                    dimension: "latencia".into(),
                    current: 86.0,
                    previous_avg: 90.0,
                    change_pct: -4.4,
                    direction: TrendDirection::Stable,
                },
            )]),
            health_score_trend: TrendDirection::Stable,
            health_score_change_pct: -1.0,
        };
        let alerts = check_regressions(&trends);
        assert!(alerts.is_empty());
    }

    // =========================================================================
    // Phase C5: Improvement Prioritization Tests
    // =========================================================================

    #[test]
    fn test_rank_improvement_targets_ordering() {
        let dims = DimensionAverages {
            correctitud: 60.0,   // weight=0.35, gain=0.35*40=14.0
            latencia: 90.0,      // weight=0.20, gain=0.20*10=2.0
            escalabilidad: 50.0, // weight=0.15, gain=0.15*50=7.5
            consistencia: 80.0,  // weight=0.15, gain=0.15*20=3.0
            robustez: 70.0,      // weight=0.15, gain=0.15*30=4.5
        };
        let targets = rank_improvement_targets(&dims, 5);
        assert_eq!(targets.len(), 5);
        // Highest potential gain first: correctitud (14.0)
        assert_eq!(targets[0].name, "correctitud");
        assert!((targets[0].potential_gain - 14.0).abs() < 0.01);
    }

    #[test]
    fn test_rank_improvement_targets_top_n() {
        let dims = DimensionAverages {
            correctitud: 50.0,
            latencia: 60.0,
            escalabilidad: 70.0,
            consistencia: 80.0,
            robustez: 90.0,
        };
        let targets = rank_improvement_targets(&dims, 3);
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0].name, "correctitud");
    }

    #[test]
    fn test_rank_improvement_targets_all_perfect() {
        let dims = DimensionAverages {
            correctitud: 100.0,
            latencia: 100.0,
            escalabilidad: 100.0,
            consistencia: 100.0,
            robustez: 100.0,
        };
        let targets = rank_improvement_targets(&dims, 5);
        for t in &targets {
            assert_eq!(t.potential_gain, 0.0);
            assert!(t.recommendation.contains("GOOD"));
        }
    }

    #[test]
    fn test_rank_improvement_recommendations() {
        let dims = DimensionAverages {
            correctitud: 30.0,   // CRITICAL
            latencia: 60.0,      // IMPROVE
            escalabilidad: 80.0, // POLISH
            consistencia: 95.0,  // GOOD
            robustez: 95.0,      // GOOD
        };
        let targets = rank_improvement_targets(&dims, 5);
        assert!(targets[0].recommendation.contains("CRITICAL"));
        assert!(targets[1].recommendation.contains("IMPROVE"));
        assert!(targets[2].recommendation.contains("POLISH"));
        assert!(targets[3].recommendation.contains("GOOD"));
    }
}
