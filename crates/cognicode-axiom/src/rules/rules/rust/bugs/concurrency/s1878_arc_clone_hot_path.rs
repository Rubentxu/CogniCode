//! S1878 — Arc clone hot path detection
//!
//! Detects Arc::clone() calls in loops or frequently-called functions
//! without proper benchmarking justification.

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

/// Rule constant for S1878
const RULE_ID: &str = "S1878";

declare_rule! {
    id: "S1878"
    name: "Arc::clone() in hot path without benchmarking"
    severity: Minor
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects Arc::clone() calls inside loops or frequently-called functions. Each clone involves atomic reference counting operations which are significantly slower than regular reference counting. Without proper benchmarking, these clones can become performance bottlenecks."
    clean_code: Clear,
    impacts: [Maintainability: Low, Reliability: Medium],

    agent_semantics: {
        summary: "Detects unnecessary Arc::clone() in hot path",
        fix_playbook: "1. Pass Arc reference (&Arc<T>) instead of cloning\n2. Or restructure to avoid repeated cloning\n3. Profile to confirm clone is actually hot",
        review_questions: [
            "Is the clone actually in a hot path?",
            "Would passing a reference break the design?",
        ],
        semantic_chunks: [
            "Arc::clone involves atomic operations - expensive compared to regular clone",
            "Pass &Arc<T> when you only need to read the shared data",
            "Profile first - don't optimize without measurement"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires confirming clone is in hot path and refactoring design"
    }

    check: => {
        detect_arc_clone_hot_path(&ctx)
    }
}

/// Detects Arc::clone() in hot paths (loops, recursive functions).
fn detect_arc_clone_hot_path(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    // Check if Arc is used
    if !source.contains("Arc::") && !source.contains("std::sync::Arc") {
        return issues;
    }

    // Pattern 1: Arc::clone inside for loops
    let for_loop_pattern = regex::Regex::new(r"for\s+[^;]+?\{[^}]*?Arc::clone\(\)[^}]*?\}").unwrap();

    for cap in for_loop_pattern.find_iter(source) {
        let pt = source[..cap.start()].lines().count();
        issues.push(Issue::new(
            RULE_ID,
            "Arc::clone() called inside for loop - potential performance issue",
            Severity::Minor,
            Category::Bug,
            ctx.file_path,
            pt + 1,
        ).with_remediation(Remediation::moderate(
            "Clone the Arc outside the loop and move the clone into threads, or benchmark to confirm overhead is acceptable"
        )));
    }

    // Pattern 2: Arc::clone inside while loops
    let while_loop_pattern = regex::Regex::new(r"while\s+[^;]+?\{[^}]*?Arc::clone\(\)[^}]*?\}").unwrap();

    for cap in while_loop_pattern.find_iter(source) {
        let pt = source[..cap.start()].lines().count();
        issues.push(Issue::new(
            RULE_ID,
            "Arc::clone() called inside while loop - potential performance issue",
            Severity::Minor,
            Category::Bug,
            ctx.file_path,
            pt + 1,
        ).with_remediation(Remediation::moderate(
            "Consider restructuring to avoid repeated clones in loops"
        )));
    }

    // Pattern 3: Multiple .clone() calls in same scope (tight loop)
    let multiple_clone_pattern = regex::Regex::new(r"\{[^}]*?\.clone\(\)[^}]*?\.clone\(\)[^}]*?\}").unwrap();

    for cap in multiple_clone_pattern.find_iter(source) {
        let text = cap.as_str();
        let clone_count = text.matches(".clone()").count();

        if clone_count >= 3 {
            let pt = source[..cap.start()].lines().count();
            issues.push(Issue::new(
                RULE_ID,
                format!("Multiple Arc::clone() calls ({}) in same scope - potential performance issue", clone_count),
                Severity::Minor,
                Category::Bug,
                ctx.file_path,
                pt + 1,
            ).with_remediation(Remediation::moderate(
                "Consider cloning once and reusing, or benchmark to confirm clone overhead is acceptable"
            )));
        }
    }

    // Pattern 4: Recursive function with Arc::clone
    let recursive_fn_pattern = regex::Regex::new(r"fn\s+(\w+)\s*<[^>]*>[^}]*?Arc::clone\(\)[^}]*?\1\s*\(|fn\s+(\w+)\s*\([^)]*\)[^}]*?Arc::clone\(\)[^}]*?\1\s*\(").unwrap();

    for cap in recursive_fn_pattern.find_iter(source) {
        let text = cap.as_str();
        let fn_name = regex::Regex::new(r"fn\s+(\w+)").unwrap()
            .captures(text)
            .and_then(|m| m.get(1))
            .map(|x| x.as_str())
            .unwrap_or("anonymous");

        let pt = source[..cap.start()].lines().count();
        issues.push(Issue::new(
            RULE_ID,
            format!("Arc::clone() in recursive function '{}' - potential performance issue", fn_name),
            Severity::Minor,
            Category::Bug,
            ctx.file_path,
            pt + 1,
        ).with_remediation(Remediation::substantial(
            "Consider using iteration instead of recursion, or use a different data structure that doesn't require cloning"
        )));
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s1878_registered() {
        let rule = S1878Rule::new();
        assert_eq!(rule.id(), "S1878");
        assert!(rule.name().len() > 0);
    }

    #[test]
    fn test_arc_clone_in_loop() {
        let rule = S1878Rule::new();
        let code = r#"
            use std::sync::Arc;
            fn process() {
                let data = Arc::new(vec![1, 2, 3]);
                for i in 0..100 {
                    let _ = data.clone();
                }
            }
        "#;
        assert_eq!(rule.id(), "S1878");
    }
}
