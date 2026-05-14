//! PERF_004 — Vec::push without reserve()
//!
//! Detects Vec::push() called without prior reserve(), causing repeated reallocation.

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use crate::rules::rules::rust::performance::perf_helpers::{
    count_brace_balance, find_brace_close, extract_loop_body,
};
use cognicode_macros::declare_rule;
use regex::Regex;

/// Rule constant for PERF_004
const RULE_ID: &str = "PERF_004";

declare_rule! {
    id: "PERF_004"
    name: "Vec::push() without prior reserve() causes repeated reallocation"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects Vec::push() calls inside loops without prior reserve(). Each push that exceeds capacity triggers reallocation, copying all elements. This causes O(n²) behavior for building collections."
    clean_code: Clear,
    impacts: [Maintainability: Medium, Reliability: Low],

    agent_semantics: {
        summary: "Detects Vec::push() without reserve() in loops",
        fix_playbook: "1. Add reserve(n) before the loop where n is expected size\n2. Use Vec::with_capacity(n) instead of Vec::new()\n3. If size is unknown, consider estimating with capacity hints",
        review_questions: [
            "Is the number of pushes predictable?",
            "What is the expected capacity needed?"
        ],
        semantic_chunks: [
            "Vec::push may trigger reallocation when capacity is exceeded",
            "reserve(n) pre-allocates space for n elements",
            "Vec::with_capacity(n) creates a Vec with pre-allocated space"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires knowing expected capacity"
    }

    check: => {
        detect_push_without_reserve(&ctx)
    }
}

static PUSH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w+\.push\(").unwrap());
static RESERVE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w+\.reserve\(").unwrap());
static WITH_CAP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Vec::with_capacity\(").unwrap());
static LOOP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(for\s+\w+\s+in|while\s+)").unwrap());
static FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"fn\s+(\w+)\s*\([^)]*\)\s*(?:->\s*[^=]+)?\s*\{").unwrap()
});

use std::sync::LazyLock;

/// Detects Vec::push without reserve.
fn detect_push_without_reserve(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    // Process each function to properly scope the reserve check
    for fn_cap in FN_RE.captures_iter(source) {
        let fn_start = fn_cap.get(0).map(|m| m.start()).unwrap_or(0);
        let brace_count = count_brace_balance(source, fn_start);

        if let Some(fn_end) = find_brace_close(source, fn_start, brace_count) {
            let fn_body = &source[fn_start..fn_end.min(source.len())];

            for loop_cap in LOOP_RE.find_iter(fn_body) {
                let loop_start = loop_cap.start();

                // Extract only this loop's body
                if let Some((loop_end, loop_body)) = extract_loop_body(fn_body, loop_start) {
                    let has_push = PUSH_RE.is_match(loop_body);

                    if has_push {
                        // Check for reserve BEFORE this loop within the function body
                        let before_loop = &fn_body[..loop_start];
                        let has_reserve = RESERVE_RE.is_match(before_loop)
                            || WITH_CAP_RE.is_match(before_loop);

                        if !has_reserve {
                            let line_num = source[..fn_start + loop_start].lines().count();
                            issues.push(Issue::new(
                                RULE_ID,
                                "Vec::push() inside loop without prior reserve()",
                                Severity::Major,
                                Category::Bug,
                                ctx.file_path,
                                line_num + 1,
                            ).with_remediation(Remediation::moderate(
                                "Add reserve() call before the loop, or use Vec::with_capacity()"
                            )));
                        }
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
    fn test_perf_004_registered() {
        let rule = PERF_004Rule::new();
        assert_eq!(rule.id(), "PERF_004");
        assert!(rule.name().len() > 0);
    }
}
