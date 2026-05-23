//! PERF_012 — Box<Vec<T>> Unnecessary Indirection
//!
//! Detects Box<Vec<T>> where Vec<T> would suffice,
//! adding unnecessary double indirection.

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use crate::rules::rules::rust::performance::perf_helpers::{
    count_brace_balance, find_brace_close,
};
use cognicode_macros::declare_rule;
use regex::Regex;

/// Rule constant for PERF_012
const RULE_ID: &str = "PERF_012";

declare_rule! {
    id: "PERF_012"
    name: "Box<Vec<T>> adds unnecessary indirection - Vec<T> suffices"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Detects Box<Vec<T>> type annotations or allocations. Box<Vec<T>> adds double indirection (pointer to Box, pointer to Vec data) with no benefit over Vec<T>, except in specific cases like trait returns or recursive structures."
    clean_code: Clear,
    impacts: [Maintainability: Low],

    agent_semantics: {
        summary: "Detects Box<Vec<T>> where Vec<T> would suffice",
        fix_playbook: "1. Replace Box<Vec<T>> with Vec<T>\n2. If needed for trait return, use impl Trait or dyn Trait\n3. If for async, use Pin<Box<Vec<T>>>\n4. If for recursive type, keep Box but reconsider structure",
        review_questions: [
            "Is Box actually needed for trait object safety?",
            "Would Vec<T> cause stack overflow?",
            "Is this in an FFI boundary?"
        ],
        semantic_chunks: [
            "Box<Vec<T>> is double indirection: Box -> Vec -> data",
            "Vec<T> is single indirection: Vec -> data",
            "Box is needed for: trait objects, async futures, recursive types, FFI"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires understanding why Box was used"
    }

    check: => {
        detect_box_vec_indirection(&ctx)
    }
}

static TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:pub\s+)?(?:const\s+)?(?:static\s+)?(\w+)\s*:\s*Box<Vec<[^>]+>>").unwrap()
});
static LET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"let\s+(?:mut\s+)?(\w+)\s*:\s*Box<Vec<[^>]+>>\s*=").unwrap()
});
static BOX_NEW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Box::new\s*\(\s*Vec::").unwrap());
static FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"fn\s+(\w+)\s*\([^)]*\)").unwrap()
});

use std::sync::LazyLock;

/// Detects unnecessary Box<Vec<T>> patterns.
fn detect_box_vec_indirection(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    for cap in FN_RE.captures_iter(source) {
        let fn_start = cap.get(0).map(|m| m.end()).unwrap_or(0);
        let brace_count = count_brace_balance(source, fn_start - 1);

        if let Some(fn_end) = find_brace_close(source, fn_start - 1, brace_count) {
            let fn_body = &source[fn_start - 1..fn_end.min(source.len())];

            // Check all Box<Vec<T>> patterns
            for (re, desc) in &[(&TYPE_RE, "type annotation"), (&LET_RE, "variable binding"), (&BOX_NEW_RE, "Box::new(Vec::...)")] {
                if let Some(cap_inner) = re.find(fn_body) {
                    let before = &fn_body[..cap_inner.start()];
                    // Valid cases: trait impl, recursive, async
                    let is_valid = before.contains("impl ")
                        || before.contains("-> Box<")
                        || fn_body.contains("Box::new(Vec::")
                        || source[..fn_start].contains("async fn");

                    if !is_valid {
                        let line_num = source[..fn_start - 1 + cap_inner.start()].lines().count();
                        issues.push(Issue::new(
                            RULE_ID,
                            format!("Box<Vec<T>> in {} adds unnecessary indirection", desc),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::moderate(
                            "Use Vec<T> directly unless Box is needed for trait objects, async, or recursion"
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
    fn test_perf_012_registered() {
        let rule = PERF_012Rule::new();
        assert_eq!(rule.id(), "PERF_012");
        assert!(rule.name().len() > 0);
    }
}
