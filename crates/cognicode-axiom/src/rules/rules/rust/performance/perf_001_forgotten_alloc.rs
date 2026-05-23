//! PERF_001 — Forgotten Box/Vec Allocation in Loop
//!
//! Detects Box::new(), Vec::new(), String::new() allocated inside loops
//! without being stored/used, causing immediate drop and wasted allocation.

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use crate::rules::rules::rust::performance::perf_helpers::{
    count_brace_balance, find_brace_close, extract_loop_body, is_test_file,
};
use cognicode_macros::declare_rule;
use regex::Regex;

/// Rule constant for PERF_001
const RULE_ID: &str = "PERF_001";

declare_rule! {
    id: "PERF_001"
    name: "Forgotten allocation inside loop - value immediately dropped"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects Box::new(), Vec::new(), or String::new() allocations inside loops that are immediately dropped. Each iteration allocates and immediately frees memory, wasting CPU cycles and fragmenting heap."
    clean_code: Clear,
    impacts: [Maintainability: High, Reliability: Medium],

    agent_semantics: {
        summary: "Detects forgotten Box/Vec/String allocations inside loops",
        fix_playbook: "1. Move allocation before the loop\n2. Use iterator chain that doesn't reallocate\n3. Pre-allocate with capacity hint if size is known",
        review_questions: [
            "Is the allocation value actually used after the loop?",
            "Could this be replaced with an iterator that doesn't reallocate?"
        ],
        semantic_chunks: [
            "Box::new()/Vec::new()/String::new() inside loops are immediately dropped",
            "Moving allocation outside loop avoids repeated allocation/deallocation",
            "Iterator chains often avoid intermediate allocations"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires understanding whether allocation is truly forgotten or legitimately scoped to each iteration"
    }

    check: => {
        detect_forgotten_allocation_in_loop(&ctx)
    }
}

// Pre-compiled patterns
static ALLOC_PATTERNS: &[&str] = &[
    r"Box::new\(",
    r"Vec::new\(\)",
    r"String::new\(\)",
    r"Vec::with_capacity\(",
    r"String::from\(",
];
static LOOP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(for\s+\w+\s+in|while\s+)").unwrap());
// Fixed: match actual Box::new, Vec::new, String::new patterns assigned to a variable
static ASSIGNMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"let\s+\w+\s*=\s*(?:Box::new|Vec::new|String::new)\s*\(").unwrap()
});

use std::sync::LazyLock;

/// Detects Box/Vec/String allocations inside loops.
fn detect_forgotten_allocation_in_loop(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    if is_test_file(source) {
        return issues;
    }

    for cap in LOOP_RE.find_iter(source) {
        let loop_start = cap.start();
        if let Some((_, loop_body)) = extract_loop_body(source, loop_start) {
            for pattern in ALLOC_PATTERNS {
                let alloc_re = Regex::new(pattern).unwrap();
                if let Some(alloc_match) = alloc_re.find(&loop_body) {
                    let line_num = source[..loop_start + alloc_match.start()].lines().count();
                    // Check if assigned - fixed regex
                    if !ASSIGNMENT_RE.is_match(&loop_body) {
                        issues.push(Issue::new(
                            RULE_ID,
                            format!("Allocation {} inside loop may be forgotten (immediately dropped)", pattern),
                            Severity::Critical,
                            Category::Bug,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::substantial(
                            "Move allocation before the loop, or use iterator chain, or confirm each iteration legitimately needs fresh allocation"
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
    fn test_perf_001_registered() {
        let rule = PERF_001Rule::new();
        assert_eq!(rule.id(), "PERF_001");
        assert!(rule.name().len() > 0);
    }
}
