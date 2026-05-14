//! PERF_005 — String Concatenation in Loop
//!
//! Detects String concatenation using + or push_str with variables
//! inside loops, causing O(n²) behavior.

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use crate::rules::rules::rust::performance::perf_helpers::extract_loop_body;
use cognicode_macros::declare_rule;
use regex::Regex;

/// Rule constant for PERF_005
const RULE_ID: &str = "PERF_005";

declare_rule! {
    id: "PERF_005"
    name: "String concatenation in loop causes O(n²) behavior"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects string concatenation using + or push_str with loop variables inside loops. Each + on String reallocates and copies the entire string, resulting in O(n²) time complexity."
    clean_code: Clear,
    impacts: [Maintainability: Medium, Reliability: Low],

    agent_semantics: {
        summary: "Detects string + or push_str in loops",
        fix_playbook: "1. Use format!() macro for complex concatenation\n2. Use String::from_iter(iterator) for simple cases\n3. Collect strings in Vec and use join()\n4. Use write! to String buffer",
        review_questions: [
            "What is being concatenated?",
            "Is the number of iterations predictable?"
        ],
        semantic_chunks: [
            "String::push_str with + inside loop reallocates each iteration",
            "format!() is optimized and doesn't suffer from this issue",
            "String::from_iter and join() are efficient for collecting strings"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires understanding the concatenation pattern"
    }

    check: => {
        detect_string_concat_in_loop(&ctx)
    }
}

static LOOP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(for\s+\w+\s+in|while\s+)").unwrap());
// Fixed: detect push_str with variable (not literal), and += with String
static CONCAT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?x)
        (?:String\s*::\s*\w+\s*\+\s*\w+)  # String::xxx() + var
        |(?:\w+\s*\+\s*String)              # var + String
        |(?:\w+\s*\+=\s*\w+)                # var += something (potential string concat)
        |(?:\.push_str\s*\(\s*\w+\s*\))     # .push_str(var)
    ").unwrap()
});
static FORMAT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"format!\s*\(").unwrap());

use std::sync::LazyLock;

/// Detects string concatenation in loops.
fn detect_string_concat_in_loop(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    for loop_cap in LOOP_RE.find_iter(source) {
        let loop_start = loop_cap.start();
        if let Some((_, loop_body)) = extract_loop_body(source, loop_start) {
            if CONCAT_RE.is_match(&loop_body) {
                // Exclude format! macro
                if !FORMAT_RE.is_match(&loop_body) {
                    let line_num = source[..loop_start].lines().count();
                    issues.push(Issue::new(
                        RULE_ID,
                        "String concatenation in loop may cause O(n²) behavior",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::moderate(
                        "Use format!(), String::from_iter(), or Vec<String>::join() instead"
                    )));
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
    fn test_perf_005_registered() {
        let rule = PERF_005Rule::new();
        assert_eq!(rule.id(), "PERF_005");
        assert!(rule.name().len() > 0);
    }
}
