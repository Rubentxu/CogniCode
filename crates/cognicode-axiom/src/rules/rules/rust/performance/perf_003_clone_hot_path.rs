//! PERF_003 — Clone in Hot Path
//!
//! Detects .clone() calls in loops or frequently-called functions,
//! causing repeated memory allocation.

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use crate::rules::rules::rust::performance::perf_helpers::extract_loop_body;
use cognicode_macros::declare_rule;
use regex::Regex;

/// Rule constant for PERF_003
const RULE_ID: &str = "PERF_003";

declare_rule! {
    id: "PERF_003"
    name: ".clone() called inside loop without benchmarking justification"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects .clone() calls inside loops. Each clone involves memory allocation and data copying. In hot paths, this compounds quickly and becomes a performance bottleneck."
    clean_code: Clear,
    impacts: [Maintainability: High, Reliability: Medium],

    agent_semantics: {
        summary: "Detects .clone() calls inside loops",
        fix_playbook: "1. Pass reference (&T) instead of cloning\n2. Restructure to avoid repeated clones\n3. Use Arc<T> only when shared ownership is genuinely needed\n4. Profile first to confirm clone is hot",
        review_questions: [
            "Would passing a reference break borrowing rules?",
            "Is the clone actually in a hot path?",
            "Could this be restructured to clone once and reuse?"
        ],
        semantic_chunks: [
            ".clone() involves memory allocation and data copying",
            "Pass &T when you only need to read the shared data",
            "Profile first - don't optimize without measurement"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires understanding ownership semantics and confirming hot path"
    }

    check: => {
        detect_clone_in_hot_path(&ctx)
    }
}

static CLONE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w+\.clone\(\)").unwrap());
static LOOP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(for\s+\w+\s+in|while\s+)").unwrap());

// Primitive types where clone is cheap (no allocation)
static PRIMITIVE_PREFIXES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128",
    "i8", "i16", "i32", "i64", "i128",
    "f32", "f64", "bool", "char",
];

use std::sync::LazyLock;

fn is_primitive_clone(clone_match: &str) -> bool {
    PRIMITIVE_PREFIXES.iter().any(|p| clone_match.starts_with(p))
}

/// Detects .clone() calls in loops.
fn detect_clone_in_hot_path(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    for loop_cap in LOOP_RE.find_iter(source) {
        let loop_start = loop_cap.start();
        if let Some((_, loop_body)) = extract_loop_body(source, loop_start) {
            for clone_cap in CLONE_RE.find_iter(&loop_body) {
                let clone_match = clone_cap.as_str();

                // Skip primitives (clone is cheap)
                if is_primitive_clone(clone_match) {
                    continue;
                }

                // Skip Arc::clone() - it's cheap (atomic ref count only)
                if clone_match.contains("Arc::clone") {
                    continue;
                }

                let line_num = source[..loop_start + clone_cap.start()].lines().count();
                issues.push(Issue::new(
                    RULE_ID,
                    format!("Expensive .clone() called inside loop: {}", clone_match),
                    Severity::Critical,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::substantial(
                    "Pass a reference (&T) or restructure to avoid repeated cloning"
                )));
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_003_registered() {
        let rule = PERF_003Rule::new();
        assert_eq!(rule.id(), "PERF_003");
        assert!(rule.name().len() > 0);
    }
}
