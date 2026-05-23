//! S1874 — Deadlock detection rules
//!
//! S1874a: Detects nested lock acquisition with inconsistent ordering (same function)
//! S1874b: Detects cross-function lock ordering issues that may cause deadlocks

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;
use std::collections::HashSet;

// ─────────────────────────────────────────────────────────────────────────────
// S1874a — Deadlock: lock ordering within a single function
// ─────────────────────────────────────────────────────────────────────────────

/// Rule constant for S1874a
const RULE_ID_S1874A: &str = "S1874a";

declare_rule! {
    id: "S1874a"
    name: "Deadlock: inconsistent lock ordering within function"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects when multiple locks are acquired in a different order within the same function, which can lead to deadlocks if two threads acquire the same locks in opposite orders simultaneously."
    clean_code: Clear,
    impacts: [Reliability: High, Security: Medium],

    agent_semantics: {
        summary: "Detects nested locks acquired in inconsistent order",
        fix_playbook: "1. Identify all lock acquisition sites\n2. Establish a consistent ordering (e.g., always lock A before B)\n3. Refactor code to respect the ordering",
        review_questions: [
            "Is the lock ordering truly inconsistent?",
            "Could this be a false positive from unrelated code?",
        ],
        semantic_chunks: [
            "Deadlock occurs when lock ordering is not consistent across threads",
            "Establish a total ordering on locks and always acquire in that order",
            "Use lock hierarchies or lock managers to enforce ordering"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires architectural analysis of lock ordering"
    }

    check: => {
        detect_inconsistent_lock_ordering(&ctx)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1874b — Deadlock: cross-function lock ordering
// ─────────────────────────────────────────────────────────────────────────────

/// Rule constant for S1874b
const RULE_ID_S1874B: &str = "S1874b";

declare_rule! {
    id: "S1874b"
    name: "Deadlock: cross-function lock ordering violation"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects when functions acquire locks in different orders, and those functions are callable from each other or from threads that also acquire locks. This can lead to deadlocks when locks are acquired in an inconsistent order across call chains."
    clean_code: Clear,
    impacts: [Reliability: High, Security: Medium],

    agent_semantics: {
        summary: "Detects potential deadlock across function boundaries",
        fix_playbook: "1. Trace lock acquisitions through function calls\n2. Ensure consistent lock ordering across call sites\n3. Consider lock-free alternatives",
        review_questions: [
            "Does the function actually acquire locks in different orders?",
            "Is this intentional for different operations?",
        ],
        semantic_chunks: [
            "Cross-function deadlocks are harder to detect than intra-function deadlocks",
            "Lock ordering must be consistent across all code paths",
            "Consider using coarse-grained locking or lock-free data structures"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires cross-function analysis and refactoring"
    }

    check: => {
        detect_cross_function_deadlock(&ctx)
    }
}

/// Extracts lock names from a lock expression like "LOCK_A.lock()"
fn extract_lock_name(lock_expr: &str) -> Option<String> {
    // Pattern: WORD.lock() or word.lock()
    let pattern = regex::Regex::new(r"([A-Z_][A-Z0-9_]*)\.lock\(\)|([a-z_][a-z0-9_]*)\.lock\(\)").unwrap();
    pattern.captures(lock_expr)
        .and_then(|m| {
            m.get(1).or(m.get(2)).map(|x| x.as_str().to_string())
        })
}

/// Detects inconsistent lock ordering within a single function.
fn detect_inconsistent_lock_ordering(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    // Find all function definitions
    let func_pattern = regex::Regex::new(r"fn\s+(\w+)\s*<[^>]*>[^}]*?\{|\bfn\s+(\w+)\s*\([^)]*\)\s*(?:->\s*[^}]+)?\{").unwrap();

    for cap in func_pattern.find_iter(source) {
        let func_text = cap.as_str();
        let func_name = regex::Regex::new(r"fn\s+(\w+)")
            .unwrap()
            .captures(func_text)
            .and_then(|m| m.get(1))
            .map(|x| x.as_str())
            .unwrap_or("anonymous");

        // Extract all lock acquisitions in order
        let lock_pattern = regex::Regex::new(r"\b([A-Z_][A-Z0-9_]*)\.lock\(\)|\b([a-z_][a-z0-9_]*)\.lock\(\)").unwrap();

        let lock_order: Vec<String> = lock_pattern.captures_iter(func_text)
            .filter_map(|m| {
                m.get(1).or(m.get(2)).map(|x| x.as_str().to_string())
            })
            .collect();

        // If we have multiple locks, check for inconsistencies in patterns like A then B vs B then A
        if lock_order.len() >= 2 {
            // Look for patterns where we have A then B in one place and B then A elsewhere
            for window in lock_order.windows(2) {
                let first = &window[0];
                let second = &window[1];

                // Check if there's a reversed order elsewhere in the same function
                let reversed = format!("{}.lock()", second);
                let original = format!("{}.lock()", first);

                // Count occurrences of each order
                let first_then_second = func_text.matches(&format!("{}.lock().*{}.lock()", first, second)).count();
                let second_then_first = func_text.matches(&format!("{}.lock().*{}.lock()", second, first)).count();

                if first_then_second > 0 && second_then_first > 0 {
                    let pt = source[..cap.start()].lines().count();
                    issues.push(Issue::new(
                        RULE_ID_S1874A,
                        format!("Inconsistent lock ordering in function '{}': locks acquired in different orders", func_name),
                        Severity::Critical,
                        Category::Bug,
                        ctx.file_path,
                        pt + 1,
                    ).with_remediation(Remediation::substantial(
                        "Establish a consistent lock ordering across all code paths and functions"
                    )));
                    break;
                }
            }
        }
    }

    issues
}

/// Detects cross-function lock ordering violations.
fn detect_cross_function_deadlock(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    // Extract all functions with their lock patterns
    #[derive(Clone)]
    struct FuncLockInfo {
        name: String,
        locks: Vec<String>,
        line: usize,
    }

    let mut func_locks: Vec<FuncLockInfo> = Vec::new();

    // Find all function definitions with lock acquisitions
    let lock_pattern = regex::Regex::new(r"\b([A-Z_][A-Z0-9_]*)\.lock\(\)|\b([a-z_][a-z0-9_]*)\.lock\(\)").unwrap();

    let func_pattern = regex::Regex::new(r"fn\s+(\w+)\s*<[^>]*>[^}]*?\{|\bfn\s+(\w+)\s*\([^)]*\)\s*(?:->\s*[^}]+)?\{").unwrap();

    for cap in func_pattern.find_iter(source) {
        let func_text = cap.as_str();
        let func_name = regex::Regex::new(r"fn\s+(\w+)")
            .unwrap()
            .captures(func_text)
            .and_then(|m| m.get(1).or(m.get(2)))
            .map(|x| x.as_str())
            .unwrap_or("anonymous");

        let locks: Vec<String> = lock_pattern.captures_iter(func_text)
            .filter_map(|m| m.get(1).or(m.get(2)).map(|x| x.as_str().to_string()))
            .collect();

        if !locks.is_empty() {
            let line = source[..cap.start()].lines().count();
            func_locks.push(FuncLockInfo {
                name: func_name.to_string(),
                locks,
                line,
            });
        }
    }

    // Check for cross-function lock ordering violations
    for i in 0..func_locks.len() {
        for j in (i + 1)..func_locks.len() {
            let fn1 = &func_locks[i];
            let fn2 = &func_locks[j];

            // Find common locks
            let common: HashSet<_> = fn1.locks.iter()
                .filter(|l| fn2.locks.contains(l))
                .collect();

            if common.len() >= 2 {
                // Get the order in fn1 and fn2
                let order1: Vec<_> = fn1.locks.iter().filter(|l| common.contains(l)).collect();
                let order2: Vec<_> = fn2.locks.iter().filter(|l| common.contains(l)).collect();

                // Check for reverse ordering
                if order1.len() >= 2 && order2.len() >= 2 {
                    let is_reversed = order1[0] == order2[1] && order1[1] == order2[0];

                    if is_reversed {
                        issues.push(Issue::new(
                            RULE_ID_S1874B,
                            format!("Cross-function lock ordering violation: '{}' and '{}' acquire locks in different orders", fn1.name, fn2.name),
                            Severity::Critical,
                            Category::Bug,
                            ctx.file_path,
                            fn1.line + 1,
                        ).with_remediation(Remediation::substantial(
                            "Standardize lock ordering across all functions that acquire multiple locks"
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
    fn test_s1874a_registered() {
        let rule = S1874aRule::new();
        assert_eq!(rule.id(), "S1874a");
    }

    #[test]
    fn test_s1874b_registered() {
        let rule = S1874bRule::new();
        assert_eq!(rule.id(), "S1874b");
    }
}
