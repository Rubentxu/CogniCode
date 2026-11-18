//! Peak memory of one pipeline, measured in a process of its own.
//!
//! `VmHWM` is the high-water mark of a whole process, so pipelines that share
//! a process cannot be told apart. Each (scenario, mode, cache) pipeline is
//! therefore run by a fresh copy of this test binary that does nothing else:
//! it builds the scenario, resets the kernel's peak counter, runs the
//! pipeline and reports the peak. The memory of building the scenario is
//! reported separately, so it is never charged to the pipeline.
//!
//! The peak is the resident set, not the allocator's view: memory the
//! allocator already held when the counter was reset is reused without
//! showing up as growth, so `growth_kib` is a lower bound of what the
//! pipeline needs and `peak_kib` is the figure to compare between pipelines.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

use super::bench::{context, run_once};
use super::harness::{Projection, materialize, scenarios};
use cognicode_core::application::change_tracking::mode_benchmark::{
    BenchCache as Cache, BenchMode as Mode,
};

/// Environment variable that selects the pipeline an isolated run measures.
pub const SELECTOR_ENV: &str = "MODE_BENCH_ISOLATED";

/// The line the isolated run prints its JSON on, so test-harness noise
/// around it can be ignored.
const MARKER: &str = "ISOLATED_PIPELINE ";

/// The name of the test that is the isolated run's entry point.
pub const ENTRY_POINT: &str = "mode_equivalence::bench_isolated_pipeline";

/// `<scenario name>#<mode>.<cache>`.
pub fn selector(scenario: &str, mode: Mode, cache: Cache) -> String {
    format!("{scenario}#{}.{}", mode.name(), cache.name())
}

fn parse_selector(text: &str) -> Option<(&str, Mode, Cache)> {
    let (scenario, pipeline) = text.rsplit_once('#')?;
    let (mode, cache) = pipeline.split_once('.')?;
    let mode = Mode::ALL.into_iter().find(|m| m.name() == mode)?;
    let cache = Cache::ALL.into_iter().find(|c| c.name() == cache)?;
    Some((scenario, mode, cache))
}

fn status_kib(field: &str) -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix(field)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

/// Resets the kernel's peak resident set to the current one (Linux 4.0+).
fn reset_peak() -> bool {
    fs::write("/proc/self/clear_refs", "5").is_ok()
}

/// What the pipeline added on top of what the process already held.
fn growth_kib(rss_before: u64, peak: u64) -> u64 {
    peak.saturating_sub(rss_before)
}

/// Runs one selected pipeline `runs` times (after one discarded run) in this
/// process and describes its memory. Panics on a selector that names nothing:
/// a measurement of nothing must not look like a small number.
pub fn measure_selected(corpus: &Path, selected: &str, runs: usize) -> Value {
    let (name, mode, cache) = parse_selector(selected)
        .unwrap_or_else(|| panic!("`{selected}` is not <scenario>#<mode>.<cache>"));
    let scenario = scenarios(corpus)
        .into_iter()
        .find(|s| s.name(Projection::Rendered) == name)
        .unwrap_or_else(|| panic!("no scenario is named `{name}`"));
    let fixture = materialize(corpus, &scenario, Projection::Rendered);
    let ctx = context(&fixture);

    let setup_peak = status_kib("VmHWM:");
    let reset = reset_peak();
    let rss_before = status_kib("VmRSS:");
    let _ = run_once(&ctx, mode, cache);
    for _ in 0..runs {
        let _ = run_once(&ctx, mode, cache);
    }
    let peak = status_kib("VmHWM:");

    json!({
        "pipeline": format!("{}.{}", mode.name(), cache.name()),
        "runs": runs,
        "peak_reset_supported": reset,
        "setup_peak_kib": setup_peak,
        "rss_after_setup_kib": rss_before,
        "peak_kib": peak,
        "growth_kib": reset.then(|| rss_before.zip(peak).map(|(b, p)| growth_kib(b, p))).flatten(),
    })
}

/// The JSON an isolated run printed, if it printed one.
fn parse_output(stdout: &str) -> Option<Value> {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(MARKER))
        .and_then(|json| serde_json::from_str(json).ok())
}

/// The line an isolated run prints for `report`.
pub fn render(report: &Value) -> String {
    format!("{MARKER}{report}")
}

/// Runs one pipeline in a fresh process of this same test binary.
pub fn spawn(selected: &str, runs: usize) -> Result<Value, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find the test binary: {e}"))?;
    let output = Command::new(exe)
        .args([ENTRY_POINT, "--ignored", "--exact", "--nocapture"])
        .env(SELECTOR_ENV, selected)
        .env("MODE_BENCH_RUNS", runs.to_string())
        .output()
        .map_err(|e| format!("cannot start the isolated run: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_output(&stdout).ok_or_else(|| {
        format!(
            "isolated run of `{selected}` printed no report (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .last()
                .unwrap_or_default()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_selector_round_trips() {
        let text = selector("rust/simple [rendered]", Mode::FullRebuild, Cache::Warm);
        assert_eq!(
            parse_selector(&text),
            Some(("rust/simple [rendered]", Mode::FullRebuild, Cache::Warm))
        );
    }

    #[test]
    fn a_selector_that_names_no_pipeline_is_refused() {
        assert_eq!(parse_selector("rust/simple"), None);
        assert_eq!(parse_selector("rust/simple#full_rebuild"), None);
        assert_eq!(parse_selector("rust/simple#nothing.warm"), None);
        assert_eq!(parse_selector("rust/simple#full_rebuild.tepid"), None);
    }

    #[test]
    fn growth_never_goes_below_zero() {
        assert_eq!(growth_kib(100, 250), 150);
        assert_eq!(growth_kib(300, 250), 0);
    }

    #[test]
    fn the_report_is_found_among_harness_noise() {
        let stdout = format!(
            "running 1 test\n{}\ntest result: ok\n",
            render(&json!({"peak_kib": 42}))
        );
        assert_eq!(parse_output(&stdout), Some(json!({"peak_kib": 42})));
        assert_eq!(parse_output("running 1 test\n"), None);
    }
}
