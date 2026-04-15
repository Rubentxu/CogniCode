//! Artifact and Result Models for Sandbox Scenarios
//!
//! Defines the per-scenario result JSON structure and aggregate summary models.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::failure::FailureClass;
use super::history::DimensionAverages;
use super::scoring::DimensionScores;

/// Timing information for a scenario, broken down by phase (in milliseconds).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timing {
    /// Time spent in workspace setup (clone, snapshot, etc.)
    pub setup_ms: u64,
    /// Time spent starting the MCP server
    pub server_startup_ms: u64,
    /// Time spent executing the MCP tool call
    pub tool_call_ms: u64,
    /// Time spent running the validation pipeline
    pub validation_ms: u64,
    /// Time spent in teardown (container stop, artifact copy)
    pub teardown_ms: u64,
    /// Total wall-clock time
    pub total_ms: u64,
}

/// Resource usage observed during scenario execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Peak resident set size in MB
    pub peak_rss_mb: f64,
    /// Total CPU time in seconds
    pub cpu_time_s: f64,
}

/// Warmup report for a tool's cold vs warm latency measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmupReport {
    /// Tool name
    pub tool: String,
    /// Latency of the first (cold) call in ms
    pub cold_latency_ms: u64,
    /// Median latency of warm calls in ms
    pub warm_median_ms: u64,
    /// Warmup penalty ratio (cold / warm_median)
    pub warmup_penalty: f64,
    /// Whether penalty exceeds 3x threshold
    pub penalty_flagged: bool,
}

/// Mutation information, present only for mutation-class scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationInfo {
    /// Number of files touched by the mutation
    pub files_touched: u32,
    /// Total lines changed (added + deleted)
    pub lines_changed: u32,
    /// Number of changes in the preview (for preview-only scenarios)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_changes: Option<u32>,
}

/// Result of a single validation pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStageResult {
    /// Stage name (syntax, format, lint, build, test)
    pub stage: String,
    /// pass | fail | skipped
    pub status: String,
    /// Exit code from the stage command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Truncated stdout excerpt (max 2KB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_excerpt: Option<String>,
    /// Truncated stderr excerpt (max 2KB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_excerpt: Option<String>,
    /// Duration of this stage in milliseconds
    pub duration_ms: u64,
}

/// Validation pipeline results per scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Per-stage results in order
    pub stages: Vec<PipelineStageResult>,
    /// Whether the overall validation passed
    pub passed: bool,
}

/// A single scenario result, emitted as JSON for each scenario execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    /// Unique scenario identifier
    pub scenario_id: String,
    /// Programming language (rust, python, javascript, typescript, java, go)
    pub language: String,
    /// Tier A (functional) or Tier B (expected-fail probe)
    pub tier: String,
    /// Source repo name
    pub repo: String,
    /// Pinned git commit
    pub commit: String,
    /// MCP tool name (e.g., safe_refactor, read_file, edit_file)
    pub tool: String,
    /// Tool action (e.g., rename, extract, inline, read)
    pub action: String,
    /// Expected outcome as declared in manifest
    pub expected_outcome: String,
    /// Actual outcome: pass | expected_fail | unexpected_fail | unexpected_pass
    pub outcome: String,
    /// Failure taxonomy key (null if outcome is pass or expected_fail)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<FailureClass>,
    /// Timing breakdown by phase
    pub timing_ms: Timing,
    /// Resource usage statistics
    pub resource_usage: ResourceUsage,
    /// Mutation details (null for non-mutation scenarios)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mutation: Option<MutationInfo>,
    /// Validation pipeline results (null for read-only scenarios)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationResult>,
    /// Quality dimension scores (null if no ground truth provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension_scores: Option<DimensionScores>,
    /// Paths to artifact files produced by this scenario
    pub artifacts: Vec<String>,
    /// Container image digest used (e.g., docker.io/library/rust@sha256:...)
    pub container_image: String,
    /// Workspace snapshot ID (content-addressed)
    pub workspace_snapshot_id: String,
    /// ISO 8601 timestamp when scenario started
    pub started_at: String,
    /// ISO 8601 timestamp when scenario completed
    pub completed_at: String,
}

impl ScenarioResult {
    /// Serialize to JSON bytes.
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec_pretty(self)
    }

    /// Serialize to a formatted JSON string.
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Aggregate summary across all scenarios in a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// Total scenarios run
    pub total: u32,
    /// Scenarios that passed
    pub passed: u32,
    /// Scenarios that failed unexpectedly
    pub failed: u32,
    /// Scenarios that failed as expected
    pub expected_failures: u32,
    /// Scenarios that passed unexpectedly (should have failed)
    pub unexpected_passes: u32,
    /// Pass rate as a fraction [0.0, 1.0]
    pub pass_rate: f64,
    /// Per-language breakdown: language -> LanguageBreakdown
    pub by_language: HashMap<String, LanguageBreakdown>,
    /// Per-tool breakdown: tool -> ToolBreakdown
    pub by_tool: HashMap<String, ToolBreakdown>,
    /// Failure class distribution
    pub failure_distribution: HashMap<String, u32>,
    /// p50/p95/p99 total duration in milliseconds
    pub duration_p50_ms: Option<u64>,
    pub duration_p95_ms: Option<u64>,
    pub duration_p99_ms: Option<u64>,
    /// Number of CI-blocking failures (excludes expected_fail, capability_missing, preexisting_repo_failure)
    pub ci_blocking: u32,
    /// Regressions vs baseline (scenario IDs that regressed)
    #[serde(default)]
    pub regressions_vs_baseline: Vec<String>,
    /// ISO 8601 timestamp of run start
    pub run_started_at: String,
    /// ISO 8604 timestamp of run completion
    pub run_completed_at: String,
    /// Orchestrator version
    pub orchestrator_version: String,
    /// MCP Health Score (0-100)
    #[serde(default)]
    pub health_score: f64,
    /// Per-dimension average scores
    #[serde(default)]
    pub dimension_scores: DimensionAverages,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageBreakdown {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    /// Pass rate as a fraction [0.0, 1.0]
    pub pass_rate: f64,
    /// p50 timing in milliseconds (None if no data)
    pub timing_p50_ms: Option<u64>,
    /// p95 timing in milliseconds
    pub timing_p95_ms: Option<u64>,
    /// p99 timing in milliseconds
    pub timing_p99_ms: Option<u64>,
}

impl LanguageBreakdown {
    pub fn new() -> Self {
        Self {
            total: 0,
            passed: 0,
            failed: 0,
            pass_rate: 0.0,
            timing_p50_ms: None,
            timing_p95_ms: None,
            timing_p99_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolBreakdown {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    /// Pass rate as a fraction [0.0, 1.0]
    pub pass_rate: f64,
    /// p50 timing in milliseconds
    pub timing_p50_ms: Option<u64>,
    /// p95 timing in milliseconds
    pub timing_p95_ms: Option<u64>,
    /// p99 timing in milliseconds
    pub timing_p99_ms: Option<u64>,
}

impl ToolBreakdown {
    pub fn new() -> Self {
        Self {
            total: 0,
            passed: 0,
            failed: 0,
            pass_rate: 0.0,
            timing_p50_ms: None,
            timing_p95_ms: None,
            timing_p99_ms: None,
        }
    }
}

impl Summary {
    pub fn new(started_at: String, version: String) -> Self {
        Self {
            total: 0,
            passed: 0,
            failed: 0,
            expected_failures: 0,
            unexpected_passes: 0,
            pass_rate: 0.0,
            by_language: HashMap::new(),
            by_tool: HashMap::new(),
            failure_distribution: HashMap::new(),
            duration_p50_ms: None,
            duration_p95_ms: None,
            duration_p99_ms: None,
            ci_blocking: 0,
            regressions_vs_baseline: Vec::new(),
            run_started_at: started_at,
            run_completed_at: String::new(),
            orchestrator_version: version,
            health_score: 0.0,
            dimension_scores: DimensionAverages::default(),
        }
    }
}

// ============================================================================
// Session Benchmark Types (Phase B2)
// ============================================================================

/// Statistics computed from latency measurements during a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkStats {
    /// Minimum latency in milliseconds
    pub min_ms: u64,
    /// Maximum latency in milliseconds
    pub max_ms: u64,
    /// Mean latency in milliseconds
    pub mean_ms: f64,
    /// Median latency in milliseconds
    pub median_ms: u64,
    /// p50 latency in milliseconds
    pub p50_ms: u64,
    /// p95 latency in milliseconds
    pub p95_ms: u64,
    /// p99 latency in milliseconds
    pub p99_ms: u64,
    /// Standard deviation in milliseconds
    pub std_dev_ms: f64,
    /// Operations per second
    pub ops_per_second: f64,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
}

/// Warmup analysis comparing first call to subsequent calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmupInfo {
    /// Cold start latency (first call) in milliseconds
    pub cold_latency_ms: u64,
    /// Median latency of warm (non-first) calls in milliseconds
    pub warm_median_ms: u64,
    /// Ratio of cold to warm median (warmup penalty)
    pub warmup_penalty: f64,
    /// Whether the warmup penalty is significant (>1.5x)
    pub penalty_flagged: bool,
}

/// Result of a session benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Tool name that was benchmarked
    pub tool: String,
    /// Total iterations requested
    pub iterations_requested: u32,
    /// Iterations actually completed
    pub iterations_completed: u32,
    /// Whether the benchmark completed fully (false if server crashed)
    pub completed: bool,
    /// Individual latency measurements in ms
    pub latencies_ms: Vec<u64>,
    /// Statistics computed from latencies
    pub stats: BenchmarkStats,
    /// Warmup analysis
    pub warmup: WarmupInfo,
    /// ISO 8601 timestamp
    pub timestamp: String,
}

impl BenchmarkResult {
    /// Serialize to JSON bytes.
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec_pretty(self)
    }

    /// Serialize to a formatted JSON string.
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_timing() -> Timing {
        Timing {
            setup_ms: 100,
            server_startup_ms: 50,
            tool_call_ms: 30,
            validation_ms: 200,
            teardown_ms: 20,
            total_ms: 400,
        }
    }

    fn make_test_result() -> ScenarioResult {
        ScenarioResult {
            scenario_id: "rust_rename_function".into(),
            language: "rust".into(),
            tier: "A".into(),
            repo: "serde-rs/serde".into(),
            commit: "abc123".into(),
            tool: "safe_refactor".into(),
            action: "rename".into(),
            expected_outcome: "pass".into(),
            outcome: "pass".into(),
            failure_class: Some(FailureClass::Pass),
            timing_ms: make_test_timing(),
            resource_usage: ResourceUsage {
                peak_rss_mb: 48.0,
                cpu_time_s: 0.5,
            },
            mutation: Some(MutationInfo {
                files_touched: 1,
                lines_changed: 7,
                preview_changes: Some(3),
            }),
            validation: Some(ValidationResult {
                stages: vec![PipelineStageResult {
                    stage: "syntax".into(),
                    status: "pass".into(),
                    exit_code: Some(0),
                    stdout_excerpt: None,
                    stderr_excerpt: None,
                    duration_ms: 50,
                }],
                passed: true,
            }),
            dimension_scores: None,
            artifacts: vec![],
            container_image: "docker.io/library/rust@sha256:abc".into(),
            workspace_snapshot_id: "snap123".into(),
            started_at: "2026-01-01T00:00:00Z".into(),
            completed_at: "2026-01-01T00:00:01Z".into(),
        }
    }

    #[test]
    fn test_scenario_result_json_roundtrip() {
        let result = make_test_result();
        let json = result.to_json_string().unwrap();
        let parsed: ScenarioResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.scenario_id, "rust_rename_function");
        assert_eq!(parsed.language, "rust");
        assert!(matches!(parsed.failure_class, Some(FailureClass::Pass)));
    }

    #[test]
    fn test_scenario_result_failure_class_serde() {
        let result = make_test_result();
        let json = serde_json::to_string(&result.failure_class).unwrap();
        assert_eq!(json, "\"pass\"");
    }

    #[test]
    fn test_summary_new() {
        let summary = Summary::new("2026-01-01T00:00:00Z".into(), "0.1.0".into());
        assert_eq!(summary.total, 0);
        assert_eq!(summary.pass_rate, 0.0);
    }

    #[test]
    fn test_pipeline_stage_result_serde() {
        let stage = PipelineStageResult {
            stage: "build".into(),
            status: "fail".into(),
            exit_code: Some(101),
            stdout_excerpt: None,
            stderr_excerpt: Some("error: expected `;`".into()),
            duration_ms: 1200,
        };
        let json = serde_json::to_string_pretty(&stage).unwrap();
        assert!(json.contains("\"status\": \"fail\""));
        assert!(json.contains("\"exit_code\": 101"));
    }
}
