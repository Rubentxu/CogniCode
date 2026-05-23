//! PERF_006 — N+1 Query Pattern
//!
//! Detects database queries inside loops fetching related entities
//! one-by-one instead of batch loading.

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use crate::rules::rules::rust::performance::perf_helpers::extract_loop_body;
use cognicode_macros::declare_rule;
use regex::Regex;

/// Rule constant for PERF_006
const RULE_ID: &str = "PERF_006";

declare_rule! {
    id: "PERF_006"
    name: "N+1 query pattern: database query inside loop"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects database query operations inside loops. Each iteration makes a separate database round-trip, causing O(n) database calls instead of O(1) or O(log n)."
    clean_code: Clear,
    impacts: [Maintainability: Medium, Reliability: Medium],

    agent_semantics: {
        summary: "Detects database queries inside loops (N+1 pattern)",
        fix_playbook: "1. Load all data upfront with a single batch query\n2. Use IN clause with all IDs\n3. Or use a JOIN to fetch related data\n4. Then look up data in memory during iteration",
        review_questions: [
            "Is this actually hitting a database?",
            "Is there a batch API available?",
            "What ORM/framework is being used?"
        ],
        semantic_chunks: [
            "N+1 queries cause one database round-trip per iteration",
            "Batch loading fetches all data in one or few queries",
            "In-memory lookup after batch load is O(1) per item"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires understanding the data model and ORM being used"
    }

    check: => {
        detect_n_plus_one_query(&ctx)
    }
}

// Query method name patterns (actual DB queries)
static QUERY_PATTERNS: &[&str] = &[
    r"\.find\s*\(",
    r"\.select\s*\(",
    r"\.get_by_\w+\s*\(",
    r"\.fetch\s*\(",
    r"\.load\s*\(",
    r"\.query\s*\(",
    r"\.get\s*\(\s*\w+\s*\)",
];
// Negative filter: non-DB methods that match query patterns
static NOT_DB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:HashMap|BTreeMap|Option|Vec|Slice)\s*\.\s*(?:find|get|query)\s*\(").unwrap()
});
static BATCH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"batch|in_parallel|par_iter").unwrap()
});
static LOOP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(for\s+\w+\s+in|while\s+)").unwrap());

use std::sync::LazyLock;

/// Detects N+1 query patterns.
fn detect_n_plus_one_query(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    for loop_cap in LOOP_RE.find_iter(source) {
        let loop_start = loop_cap.start();
        if let Some((_, loop_body)) = extract_loop_body(source, loop_start) {
            for pattern in QUERY_PATTERNS {
                let query_re = Regex::new(pattern).unwrap();
                if query_re.is_match(&loop_body) {
                    // Skip if it's actually a non-DB method (HashMap::find, etc.)
                    if NOT_DB_RE.is_match(&loop_body) {
                        continue;
                    }
                    // Exclude known-safe patterns
                    if BATCH_RE.is_match(&loop_body) {
                        continue;
                    }

                    let line_num = source[..loop_start].lines().count();
                    issues.push(Issue::new(
                        RULE_ID,
                        "Possible N+1 query pattern: database query inside loop",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::substantial(
                        "Load all required data upfront with a batch query, then use in-memory lookups"
                    )));
                    break;
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
    fn test_perf_006_registered() {
        let rule = PERF_006Rule::new();
        assert_eq!(rule.id(), "PERF_006");
        assert!(rule.name().len() > 0);
    }
}
