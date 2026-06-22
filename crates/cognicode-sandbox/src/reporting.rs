//! Reporting module — scenario result aggregation, summary generation,
//! and regression detection.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use cognicode_core::sandbox_core::artifacts::{ScenarioResult, Summary};
use cognicode_core::sandbox_core::failure::FailureClass;

const ORCHESTRATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

fn iso8601_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let nanos = now.subsec_nanos();
    let t = std::time::UNIX_EPOCH + std::time::Duration::new(secs, nanos);
    let datetime: chrono::DateTime<chrono::Utc> = t.into();
    datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Aggregate per-scenario results into a summary with pass rates,
/// per-language and per-tool breakdowns, and timing percentiles.
pub fn aggregate_summary(results: &[ScenarioResult]) -> Summary {
    let mut summary = Summary::new(iso8601_now(), ORCHESTRATOR_VERSION.to_string());
    let mut all_durations: Vec<u64> = Vec::new();

    let mut lang_durations: HashMap<String, Vec<u64>> = HashMap::new();
    let mut tool_durations: HashMap<String, Vec<u64>> = HashMap::new();

    for r in results {
        summary.total += 1;
        all_durations.push(r.timing_ms.total_ms);

        lang_durations
            .entry(r.language.clone())
            .or_default()
            .push(r.timing_ms.total_ms);

        tool_durations
            .entry(r.tool.clone())
            .or_default()
            .push(r.timing_ms.total_ms);

        match r.outcome.as_str() {
            "pass" => {
                summary.passed += 1;
            }
            "expected_fail" | "preexisting_fail" => {
                summary.expected_failures += 1;
            }
            "unexpected_pass" => {
                summary.unexpected_passes += 1;
                summary.failed += 1;
            }
            _ => {
                summary.failed += 1;
            }
        }

        let lang_entry = summary.by_language.entry(r.language.clone()).or_default();
        lang_entry.total += 1;
        if r.outcome == "pass" || r.outcome == "expected_fail" || r.outcome == "preexisting_fail" {
            lang_entry.passed += 1;
        } else {
            lang_entry.failed += 1;
            if let Some(fc) = &r.failure_class
                && fc.is_ci_blocking() {
                    summary.ci_blocking += 1;
                }
        }

        let tool_entry = summary.by_tool.entry(r.tool.clone()).or_default();
        tool_entry.total += 1;
        if r.outcome == "pass" || r.outcome == "expected_fail" || r.outcome == "preexisting_fail" {
            tool_entry.passed += 1;
        } else {
            tool_entry.failed += 1;
        }

        if let Some(fc) = &r.failure_class {
            let key = fc.to_string();
            *summary.failure_distribution.entry(key).or_insert(0) += 1;
        }
    }

    if summary.total > 0 {
        summary.pass_rate = summary.passed as f64 / summary.total as f64;
    }

    for (lang, durations) in &mut lang_durations {
        if let Some(lang_entry) = summary.by_language.get_mut(lang) {
            if lang_entry.total > 0 {
                lang_entry.pass_rate = lang_entry.passed as f64 / lang_entry.total as f64;
            }
            durations.sort();
            let len = durations.len();
            lang_entry.timing_p50_ms = Some(durations[len / 2]);
            lang_entry.timing_p95_ms = Some(durations[(len * 95 / 100).min(len - 1)]);
            lang_entry.timing_p99_ms = Some(durations[(len * 99 / 100).min(len - 1)]);
        }
    }

    for (tool, durations) in &mut tool_durations {
        if let Some(tool_entry) = summary.by_tool.get_mut(tool) {
            if tool_entry.total > 0 {
                tool_entry.pass_rate = tool_entry.passed as f64 / tool_entry.total as f64;
            }
            durations.sort();
            let len = durations.len();
            tool_entry.timing_p50_ms = Some(durations[len / 2]);
            tool_entry.timing_p95_ms = Some(durations[(len * 95 / 100).min(len - 1)]);
            tool_entry.timing_p99_ms = Some(durations[(len * 99 / 100).min(len - 1)]);
        }
    }

    if !all_durations.is_empty() {
        all_durations.sort();
        let len = all_durations.len();
        summary.duration_p50_ms = Some(all_durations[len / 2]);
        summary.duration_p95_ms = Some(all_durations[(len * 95 / 100).min(len - 1)]);
        summary.duration_p99_ms = Some(all_durations[(len * 99 / 100).min(len - 1)]);
    }

    summary.run_completed_at = iso8601_now();
    summary
}

/// Generate a Markdown validation report from scenario results and summary.
pub fn generate_markdown_summary(results: &[ScenarioResult], summary: &Summary) -> String {
    use std::fmt::Write;
    let mut md = String::new();

    writeln!(md, "# CogniCode Sandbox Validation Report").unwrap();
    writeln!(md).unwrap();
    writeln!(md, "**Date**: {}", summary.run_started_at).unwrap();
    writeln!(
        md,
        "**Total**: {} | **Passed**: {} | **Failed**: {} | **Expected Failures**: {}",
        summary.total, summary.passed, summary.failed, summary.expected_failures
    )
    .unwrap();
    writeln!(md, "**Pass Rate**: {:.1}%", summary.pass_rate * 100.0).unwrap();

    fn fmt_dim(v: f64) -> String {
        if v > 0.0 {
            format!("{:.0}", v)
        } else {
            "N/A".to_string()
        }
    }
    writeln!(md, "**MCP Health Score**: {:.1}", summary.health_score).unwrap();
    writeln!(
        md,
        "**Dimensions**: CORR:{} LAT:{} ESC:{} CON:{} ROB:{}",
        fmt_dim(summary.dimension_scores.correctitud),
        fmt_dim(summary.dimension_scores.latencia),
        fmt_dim(summary.dimension_scores.escalabilidad),
        fmt_dim(summary.dimension_scores.consistencia),
        fmt_dim(summary.dimension_scores.robustez)
    )
    .unwrap();
    writeln!(md).unwrap();

    if !summary.by_language.is_empty() {
        writeln!(md, "## Per-Language Breakdown").unwrap();
        writeln!(md).unwrap();
        writeln!(
            md,
            "| Language | Total | Passed | Failed | Pass Rate | p50 | p95 | p99 |"
        )
        .unwrap();
        writeln!(md, "|----------|-------|--------|--------|-----------|-----|-----|-----|").unwrap();

        let mut languages: Vec<_> = summary.by_language.iter().collect();
        languages.sort_by(|a, b| a.0.cmp(b.0));

        for (lang, stats) in languages {
            writeln!(
                md,
                "| {} | {} | {} | {} | {:.1}% | {}ms | {}ms | {}ms |",
                lang,
                stats.total,
                stats.passed,
                stats.failed,
                stats.pass_rate * 100.0,
                stats.timing_p50_ms.map_or(0, |v| v),
                stats.timing_p95_ms.map_or(0, |v| v),
                stats.timing_p99_ms.map_or(0, |v| v)
            )
            .unwrap();
        }
        writeln!(md).unwrap();
    }

    if !summary.by_tool.is_empty() {
        writeln!(md, "## Per-Tool Breakdown").unwrap();
        writeln!(md).unwrap();
        writeln!(md, "| Tool | Total | Passed | Failed | Pass Rate | p50 | p95 | p99 |").unwrap();
        writeln!(md, "|------|-------|--------|--------|-----------|-----|-----|-----|").unwrap();

        let mut tools: Vec<_> = summary.by_tool.iter().collect();
        tools.sort_by(|a, b| a.0.cmp(b.0));

        for (tool, stats) in tools {
            writeln!(
                md,
                "| {} | {} | {} | {} | {:.1}% | {}ms | {}ms | {}ms |",
                tool,
                stats.total,
                stats.passed,
                stats.failed,
                stats.pass_rate * 100.0,
                stats.timing_p50_ms.map_or(0, |v| v),
                stats.timing_p95_ms.map_or(0, |v| v),
                stats.timing_p99_ms.map_or(0, |v| v)
            )
            .unwrap();
        }
        writeln!(md).unwrap();
    }

    if summary.duration_p50_ms.is_some() {
        writeln!(md, "## Timing Distribution").unwrap();
        writeln!(md).unwrap();
        writeln!(md, "- **p50**: {}ms", summary.duration_p50_ms.unwrap_or(0)).unwrap();
        writeln!(md, "- **p95**: {}ms", summary.duration_p95_ms.unwrap_or(0)).unwrap();
        writeln!(md, "- **p99**: {}ms", summary.duration_p99_ms.unwrap_or(0)).unwrap();
        writeln!(md).unwrap();
    }

    writeln!(md, "## Results").unwrap();
    writeln!(md).unwrap();
    writeln!(md, "| Scenario | Language | Tool | Action | Outcome | Duration |").unwrap();
    writeln!(md, "|----------|----------|------|--------|---------|----------|").unwrap();

    for r in results {
        writeln!(
            md,
            "| {} | {} | {} | {} | {} | {}ms |",
            r.scenario_id, r.language, r.tool, r.action, r.outcome, r.timing_ms.total_ms
        )
        .unwrap();
    }

    writeln!(md).unwrap();

    if !summary.failure_distribution.is_empty() {
        writeln!(md, "## Failure Distribution").unwrap();
        writeln!(md).unwrap();
        for (class, count) in &summary.failure_distribution {
            writeln!(md, "- **{}**: {}", class, count).unwrap();
        }
        writeln!(md).unwrap();
    }

    md
}

/// Load a baseline summary from a JSON file.
pub fn load_baseline_summary(path: &Path) -> std::io::Result<Summary> {
    let content = fs::read_to_string(path)?;
    let summary: Summary = serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(summary)
}

/// Compute regressions vs baseline by comparing current results against baseline.
///
/// A regression is detected when pass rate drops >5% or new CI-blocking failures appear.
pub fn compute_regressions(current: &[ScenarioResult], baseline: &Summary) -> Vec<String> {
    let mut regressions = Vec::new();

    let baseline_pass_rate = baseline.pass_rate;
    let current_pass_rate = current
        .iter()
        .filter(|r| r.outcome == "pass" || r.outcome == "expected_fail")
        .count() as f64
        / current.len().max(1) as f64;

    if current_pass_rate < baseline_pass_rate - 0.05 {
        regressions.push(format!(
            "Pass rate dropped from {:.1}% to {:.1}%",
            baseline_pass_rate * 100.0,
            current_pass_rate * 100.0
        ));
    }

    let current_ci_blocking = current
        .iter()
        .filter(|r| {
            r.outcome == "unexpected_fail"
                && r.failure_class.as_ref().is_some_and(|fc| {
                    !matches!(
                        fc,
                        FailureClass::CapabilityMissing
                            | FailureClass::PreexistingRepoFailure
                            | FailureClass::ExpectedFail
                    )
                })
        })
        .count();

    if current_ci_blocking > baseline.ci_blocking as usize {
        regressions.push(format!(
            "CI-blocking failures increased from {} to {}",
            baseline.ci_blocking, current_ci_blocking
        ));
    }

    regressions
}

/// Write summary JSON to {results_dir}/summary.json.
pub fn write_summary(summary: &Summary, results_dir: &Path) -> std::io::Result<PathBuf> {
    fs::create_dir_all(results_dir)?;
    let path = results_dir.join("summary.json");
    let json = serde_json::to_string_pretty(summary).unwrap_or_else(|_| "{}".into());
    fs::write(&path, json)?;
    Ok(path)
}
