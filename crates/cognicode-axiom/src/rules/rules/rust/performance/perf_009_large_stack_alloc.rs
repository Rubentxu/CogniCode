//! PERF_009 — Large Stack Allocation
//!
//! Detects large fixed-size arrays (>10KB) allocated on stack,
//! risking stack overflow.

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use regex::Regex;

/// Rule constant for PERF_009
const RULE_ID: &str = "PERF_009";

declare_rule! {
    id: "PERF_009"
    name: "Large array (>10KB) allocated on stack may cause stack overflow"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects large fixed-size arrays (>10KB) allocated on the stack. Default Rust stack size is 2-8MB, but large stack allocations can still cause overflow, especially in recursive functions where stack frames compound."
    clean_code: Clear,
    impacts: [Reliability: High, Maintainability: Medium],

    agent_semantics: {
        summary: "Detects large fixed-size arrays (>10KB) on stack",
        fix_playbook: "1. Use Box::new([value; N]) to heap-allocate\n2. Use Vec::with_capacity(N) and fill\n3. Use a static/const if data is known at compile time\n4. Consider using a fixed-size heap allocator",
        review_questions: [
            "Is the array size actually large enough to cause issues?",
            "Is this in a recursive function (stack frames add up)?",
            "Could this be a static/const?"
        ],
        semantic_chunks: [
            "Default Rust stack is 2-8MB, but large allocations add up",
            "Recursive functions compound stack usage across frames",
            "Box::new heap-allocates and is bounded by available heap"
        ],
        safe_autofix: true,
        autofix_guidance: "Can autofix by wrapping in Box::new([...]) but verify no performance regression"
    }

    check: => {
        detect_large_stack_alloc(&ctx)
    }
}

// Type sizes in bytes
static TYPE_SIZE_MAP: &[(&str, usize)] = &[
    ("u8", 1), ("u16", 2), ("u32", 4), ("u64", 8), ("u128", 16),
    ("i8", 1), ("i16", 2), ("i32", 4), ("i64", 8), ("i128", 16),
    ("f32", 4), ("f64", 8), ("bool", 1), ("char", 4),
];
// Size threshold: 10KB
const SIZE_THRESHOLD: usize = 10 * 1024;

static ARRAY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\s*(?:\w+\s*;?\s*)?(\d+)\s*\]").unwrap()
});
static ELEM_TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\w+)\s*\[\s*(?:\w+\s*;?\s*)?(\d+)\s*\]").unwrap()
});

use std::sync::LazyLock;

/// Detects large stack allocations.
fn detect_large_stack_alloc(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    for cap in ARRAY_RE.find_iter(source) {
        let array_start = cap.start();
        let before = &source[..array_start];

        // Skip const/static and test code
        if before.contains("const ") || before.contains("static ")
            || before.contains("#[test]") || before.contains("#[cfg(test)]")
        {
            continue;
        }

        if let Some(elem_cap) = ELEM_TYPE_RE.captures(cap.as_str()) {
            let type_name = elem_cap.get(1).map(|m| m.as_str()).unwrap_or("u8");
            let count: usize = elem_cap.get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);

            let elem_size = TYPE_SIZE_MAP.iter()
                .find(|(name, _)| *name == type_name)
                .map(|(_, size)| *size)
                .unwrap_or(1);

            let total_size = count * elem_size;

            if total_size > SIZE_THRESHOLD {
                let line_num = before.lines().count();
                issues.push(Issue::new(
                    RULE_ID,
                    format!("Large array [{}; {}] = {} bytes allocated on stack (threshold: 10KB)",
                        type_name, count, total_size),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Use Box::new([...]) to heap-allocate, or use Vec, or make static/const"
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
    fn test_perf_009_registered() {
        let rule = PERF_009Rule::new();
        assert_eq!(rule.id(), "PERF_009");
        assert!(rule.name().len() > 0);
    }
}
