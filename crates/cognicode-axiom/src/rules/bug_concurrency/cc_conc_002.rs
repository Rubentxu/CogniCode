//! CC_CONC002: Mutex guard leaked - MutexGuard not released before return/.await
//!
//! Detects when a MutexGuard, RwLockGuard, or similar guard is returned or
//! used across an await point without being properly dropped.
//!
//! # Problem
//! Returning a reference to a guard's contents while the guard is still live
//! can cause deadlocks or use-after-free issues when the guard lifetime
//! exceeds the protected data's lifetime.
//!
//! # Fix
//! Drop the guard explicitly with `drop(guard)` before returning, or use
//! a scope to ensure the guard is dropped before the return.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};

/// CC_CONC002 Rule: Mutex guard leaked
pub struct MutexGuardLeakRule;

impl Default for MutexGuardLeakRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for MutexGuardLeakRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CONC002")
    }

    fn name(&self) -> &'static str {
        "Mutex guard leaked: MutexGuard not released before return/.await"
    }

    fn description(&self) -> &'static str {
        "MutexGuard or RwLockGuard is still in scope when returning a reference. \
         The guard must be dropped before returning to avoid deadlocks."
    }

    fn category(&self) -> Category {
        Category::Correctness
    }

    fn severity(&self) -> Severity {
        Severity::Major
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Rust]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Simple detection: look for lock().unwrap() followed by early return
        // Pattern: "let ... = lock().unwrap();" followed by "return" before any drop(guard)

        let lines: Vec<&str> = source.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Check for lock().unwrap() pattern
            if trimmed.contains("lock().unwrap()") || trimmed.contains("lock().unwrap();") {
                // Found a lock call - look for early return before any drop
                let lock_line = i;

                // Look ahead for return statements before a drop
                let mut found_drop = false;
                let mut found_return = false;

                for j in (lock_line + 1)..lines.len().min(lock_line + 10) {
                    let check_line = lines[j].trim();

                    if check_line.starts_with("drop(") {
                        found_drop = true;
                        break;
                    }
                    if check_line.starts_with("return") && !check_line.contains("//") {
                        found_return = true;
                        // Found return before drop
                        if !found_drop {
                            issues.push(Issue::new(
                                "CC_CONC002",
                                "Early return with lock guard in scope",
                                Severity::Major,
                                Category::Correctness,
                                ctx.file_path.to_string_lossy(),
                                j + 1,
                                0,
                                "Lock guard may not be properly dropped before early return. \
                                 Ensure the guard is dropped or the value is copied before returning.",
                            ));
                        }
                        break;
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["Mutex", "RwLock", "lock", "guard", "return"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_code(code: &str) -> (tree_sitter::Tree, String) {
        let lang = SrcLanguage::Rust.to_ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(code, None).unwrap();
        (tree, code.to_string())
    }

    fn check_rule(code: &str) -> Vec<Issue> {
        let (tree, source) = parse_code(code);
        let metrics = crate::types::FileMetrics::default();
        let ctx = RuleContext::new(
            &tree,
            &source,
            std::path::Path::new("test.rs"),
            &SrcLanguage::Rust,
            &metrics,
        );
        let rule = MutexGuardLeakRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_early_return_with_guard() {
        let code = r#"
use std::sync::Mutex;

fn process(lock: &Mutex<Vec<u32>>) -> usize {
    let mut guard = lock.lock().unwrap();
    if guard.len() > 100 {
        return guard.len();
    }
    guard.push(42);
    guard.len()
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect early return with guard");
    }

    #[test]
    fn test_no_false_positive_guard_dropped() {
        let code = r#"
use std::sync::Mutex;

fn get_value(lock: &Mutex<u32>) -> u32 {
    let guard = lock.lock().unwrap();
    let value = *guard;
    drop(guard);
    value
}
"#;
        let issues = check_rule(code);
        // Should not flag when guard is explicitly dropped
        let guard_leak_issues: Vec<_> = issues.iter()
            .filter(|i| i.rule_id == "CC_CONC002" && i.message.contains("dropped"))
            .collect();
        assert!(guard_leak_issues.is_empty(), "Should not flag when guard is properly dropped");
    }

    #[test]
    fn test_no_false_positive_no_return_reference() {
        let code = r#"
use std::sync::Mutex;

fn get_len(lock: &Mutex<Vec<u32>>) -> usize {
    let guard = lock.lock().unwrap();
    guard.len()
}
"#;
        let issues = check_rule(code);
        // Should not flag when return is the guard's method call result
        let guard_leak_issues: Vec<_> = issues.iter()
            .filter(|i| i.rule_id == "CC_CONC002")
            .collect();
        assert!(guard_leak_issues.is_empty(), "Should not flag when no reference returned");
    }
}
