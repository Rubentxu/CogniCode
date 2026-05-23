//! PERF_002 — Unnecessary Allocation in Hot Path
//!
//! Detects memory allocations (String, Vec, Box) inside frequently-executed
//! code paths without necessity.

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use crate::rules::rules::rust::performance::perf_helpers::{
    count_brace_balance, find_brace_close, extract_loop_body,
};
use cognicode_macros::declare_rule;
use regex::Regex;

/// Rule constant for PERF_002
const RULE_ID: &str = "PERF_002";

declare_rule! {
    id: "PERF_002"
    name: "Memory allocation inside hot path without justification"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects allocations of String, Vec, or Box inside loops in functions likely to be hot paths (named process, handle, update, tick, etc.). Without proper benchmarking, these allocations can become performance bottlenecks."
    clean_code: Clear,
    impacts: [Maintainability: High, Reliability: Medium],

    agent_semantics: {
        summary: "Detects memory allocations inside hot path functions",
        fix_playbook: "1. Hoist allocation outside the loop\n2. Reuse a buffer across iterations\n3. Use stack allocation for fixed-size data\n4. Consider arena allocation for complex cases",
        review_questions: [
            "Is this function actually called frequently?",
            "Could the allocation be moved outside the loop?",
            "Is the buffer size predictable?"
        ],
        semantic_chunks: [
            "Functions named process/handle/update/tick are likely hot paths",
            "Allocations inside loops in hot paths compound quickly",
            "Buffer reuse avoids repeated allocation overhead"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires confirming hot path and safe buffer reuse"
    }

    check: => {
        detect_allocation_in_hot_path(&ctx)
    }
}

// Pre-compiled patterns
static HOT_PATH_NAMES: &[&str] = &[
    "process", "handle", "update", "tick", "run", "loop",
    "execute", "dispatch", "on_event", "on_message",
];
static ALLOC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(String::from\(|String::new\(\)|Vec::new\(\)|Vec::with_capacity\(|Box::new\()").unwrap()
});
static LOOP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(for\s+\w+\s+in|while\s+)").unwrap());
static FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"fn\s+(\w+)\s*\([^)]*\)\s*(?:->\s*[^=]+)?\s*\{").unwrap()
});

use std::sync::LazyLock;

fn is_hot_path_fn(name: &str) -> bool {
    let lower = name.to_lowercase();
    HOT_PATH_NAMES.iter().any(|h| lower.contains(h))
}

/// Detects allocations inside hot paths.
fn detect_allocation_in_hot_path(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    for cap in FN_RE.captures_iter(source) {
        let fn_name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let fn_start = cap.get(0).map(|m| m.start()).unwrap_or(0);

        if !is_hot_path_fn(fn_name) {
            continue;
        }

        let brace_count = count_brace_balance(source, fn_start);
        if let Some(fn_end) = find_brace_close(source, fn_start, brace_count) {
            let fn_body = &source[fn_start..fn_end.min(source.len())];

            for loop_cap in LOOP_RE.find_iter(fn_body) {
                let loop_start = loop_cap.start();
                if let Some((_, loop_body)) = extract_loop_body(fn_body, loop_start) {
                    if let Some(alloc_match) = ALLOC_RE.find(&loop_body) {
                        let line_offset = source[..fn_start + loop_start + alloc_match.start()].lines().count();
                        issues.push(Issue::new(
                            RULE_ID,
                            format!("Memory allocation inside hot path function '{}' loop", fn_name),
                            Severity::Critical,
                            Category::Bug,
                            ctx.file_path,
                            line_offset + 1,
                        ).with_remediation(Remediation::substantial(
                            "Hoist allocation outside the loop, reuse a buffer, or confirm via profiling this is actually hot"
                        )));
                    }
                }
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_002_registered() {
        let rule = PERF_002Rule::new();
        assert_eq!(rule.id(), "PERF_002");
        assert!(rule.name().len() > 0);
    }
}
