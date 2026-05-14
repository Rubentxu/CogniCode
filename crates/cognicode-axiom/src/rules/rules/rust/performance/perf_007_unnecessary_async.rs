//! PERF_007 — Unnecessary async/await Wrapper
//!
//! Detects async fn that immediately returns without any await,
//! adding async overhead for no benefit.

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use crate::rules::rules::rust::performance::perf_helpers::{
    count_brace_balance, find_brace_close,
};
use cognicode_macros::declare_rule;
use regex::Regex;

/// Rule constant for PERF_007
const RULE_ID: &str = "PERF_007";

declare_rule! {
    id: "PERF_007"
    name: "async fn with no await expressions - unnecessary async overhead"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Detects async fn that contains no await expressions. The async keyword adds overhead (Future allocation, state machine) without any benefit when no actual asynchronous operations are performed."
    clean_code: Clear,
    impacts: [Maintainability: Medium],

    agent_semantics: {
        summary: "Detects async fn with no await expressions",
        fix_playbook: "1. Remove async keyword if no async operations\n2. Change return type from Future to the actual type\n3. If in trait, consider async_trait macro or redesign",
        review_questions: [
            "Is this function required to be async by a trait?",
            "Will this function ever need to use await?",
            "Is the overhead of async actually a concern here?"
        ],
        semantic_chunks: [
            "async fn without await still creates a Future state machine",
            "Removing async can simplify code and reduce binary size",
            "Traits may require async for implementation consistency"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - may break trait implementations or API contracts"
    }

    check: => {
        detect_unnecessary_async(&ctx)
    }
}

static ASYNC_FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"async\s+fn\s+(\w+)\s*\([^)]*\)").unwrap()
});
static AWAIT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bawait\b").unwrap());

use std::sync::LazyLock;

/// Detects async fn without await.
fn detect_unnecessary_async(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    for cap in ASYNC_FN_RE.captures_iter(source) {
        let fn_name = cap.get(1).map(|m| m.as_str()).unwrap_or("unknown");
        let fn_start = cap.get(0).map(|m| m.start()).unwrap_or(0);

        let brace_count = count_brace_balance(source, fn_start);
        if let Some(fn_end) = find_brace_close(source, fn_start, brace_count) {
            let fn_body = &source[fn_start..fn_end.min(source.len())];

            if !AWAIT_RE.is_match(fn_body) {
                // Check if trait impl (may require async)
                let before = &source[..fn_start];
                let is_trait_impl = before.contains("impl ") || before.contains("trait ");

                if !is_trait_impl {
                    let line_num = source[..fn_start].lines().count();
                    issues.push(Issue::new(
                        RULE_ID,
                        format!("async fn '{}' has no await expressions - unnecessary async overhead", fn_name),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::moderate(
                        "Remove async keyword if no async operations are performed"
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
    fn test_perf_007_registered() {
        let rule = PERF_007Rule::new();
        assert_eq!(rule.id(), "PERF_007");
        assert!(rule.name().len() > 0);
    }
}
