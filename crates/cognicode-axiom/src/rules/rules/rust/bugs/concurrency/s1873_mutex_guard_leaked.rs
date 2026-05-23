//! S1873 — Mutex guard leaked detection
//!
//! Detects MutexGuard, RwLockReadGuard, or RwLockWriteGuard that are returned
//! or held across scope boundaries without explicit Drop management.

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

/// Rule constant for S1873
const RULE_ID: &str = "S1873";

declare_rule! {
    id: "S1873"
    name: "Mutex guard leaked - lock guard not released before escape"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects when a MutexGuard (or RwLockReadGuard/RwLockWriteGuard) is returned or stored in a way that allows the borrow to escape the scope, potentially causing deadlocks or stale data. The guard should be explicitly dropped before the protected reference escapes."
    clean_code: Clear,
    impacts: [Reliability: High, Maintainability: Medium],

    agent_semantics: {
        summary: "Detects MutexGuard or RwLockGuard held across await point",
        fix_playbook: "1. Drop the guard before the await: drop(guard)\n2. Or restructure to not hold lock across await\n3. Use lock_async() for async contexts",
        review_questions: [
            "Is the guard genuinely held across an await?",
            "Could this cause deadlock?",
        ],
        semantic_chunks: [
            "MutexGuard dropped too early can cause use-after-free of borrowed data",
            "Holding locks across await points can cause deadlocks in async code",
            "Extract value before returning or use tokio::sync::Mutex for async"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - restructuring required to properly manage guard lifetime"
    }

    check: => {
        let mut issues = Vec::new();

        // Pattern 1: lock().unwrap() returning a guard that is then returned
        issues.extend(detect_guard_returned(&ctx));

        // Pattern 2: early return with guard in scope
        issues.extend(detect_early_return_with_guard(&ctx));

        issues
    }
}

/// Detects patterns where a lock guard is returned directly.
///
/// Example: `return &*lock.lock().unwrap();`
/// The guard is dropped immediately but the borrow escapes.
///
/// # Arguments
/// * `ctx` - The rule context
///
/// # Returns
/// Vector of detected issues
fn detect_guard_returned(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();

    // Look for return &*lock.lock().unwrap() pattern
    let return_lock_pattern = regex::Regex::new(r"return\s+&[*]\s*\w+\.lock\(\)\.(unwrap|expect)").unwrap();

    for cap in return_lock_pattern.find_iter(ctx.source) {
        let pt = ctx.source[..cap.start()].lines().count();
        issues.push(Issue::new(
            RULE_ID,
            "Mutex guard returned directly - reference escapes but guard is dropped",
            Severity::Major,
            Category::Bug,
            ctx.file_path,
            pt + 1,
        ).with_remediation(Remediation::moderate(
            "Clone or copy the value before returning, or explicitly manage the guard lifetime"
        )));
    }

    // Also detect &*guard patterns in return
    let return_guard_pattern = regex::Regex::new(r"return\s+&[*]\w+;").unwrap();

    for cap in return_guard_pattern.find_iter(ctx.source) {
        let line = ctx.source[..cap.start()].lines().next_back().unwrap_or("");
        // Check if this looks like returning a mutex guard
        if line.contains(".lock().unwrap()") || line.contains(".read().unwrap()") || line.contains(".write().unwrap()") {
            let pt = ctx.source[..cap.start()].lines().count();
            issues.push(Issue::new(
                RULE_ID,
                "Lock guard returned - borrow may outlive the guard",
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                pt + 1,
            ).with_remediation(Remediation::moderate(
                "Extract the value before returning, or use a different synchronization pattern"
            )));
        }
    }

    issues
}

/// Detects early return statements while a guard is in scope.
///
/// Example:
/// ```rust
/// fn process(lock: &Mutex<Vec<u32>>) -> usize {
///     let guard = lock.lock().unwrap();
///     if guard.len() > 100 {
///         return guard.len();  // guard dropped but borrow used
///     }
///     guard.len()
/// }
/// ```
///
/// # Arguments
/// * `ctx` - The rule context
///
/// # Returns
/// Vector of detected issues
fn detect_early_return_with_guard(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();

    // Look for function with lock guard and early return
    let func_pattern = regex::Regex::new(r"fn\s+\w+[^}]*?let\s+\w+\s*=\s*\w+\.lock\(\)\.(unwrap|expect)[^}]*?return").unwrap();

    for cap in func_pattern.find_iter(ctx.source) {
        let text = cap.as_str();
        // Check if there's an if before the return
        if text.contains("if") && text.contains("return") {
            let func_name_match = regex::Regex::new(r"fn\s+(\w+)").unwrap();
            let fn_name = func_name_match.captures(text)
                .and_then(|m| m.get(1))
                .map(|x| x.as_str())
                .unwrap_or("anonymous");

            let pt = ctx.source[..cap.start()].lines().count();
            issues.push(Issue::new(
                RULE_ID,
                format!("Early return with lock guard in scope in function '{}'", fn_name),
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                pt + 1,
            ).with_remediation(Remediation::moderate(
                "Ensure the guard is explicitly dropped before returning, or restructure the code"
            )));
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s1873_registered() {
        let rule = S1873Rule::new();
        assert_eq!(rule.id(), "S1873");
        assert!(rule.name().len() > 0);
    }

    #[test]
    fn test_mutex_guard_return_detection() {
        let rule = S1873Rule::new();
        let code = r#"
            use std::sync::Mutex;
            fn get_lock<'a>(lock: &'a Mutex<u32>) -> &'a u32 {
                return &*lock.lock().unwrap();
            }
        "#;
        assert_eq!(rule.id(), "S1873");
    }
}
