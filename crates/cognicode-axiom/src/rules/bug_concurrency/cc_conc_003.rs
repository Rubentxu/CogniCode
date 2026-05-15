//! CC_CONC003: Deadlock - nested locks acquired in inconsistent order
//!
//! Detects when multiple locks are acquired in different orders in different
//! functions, which can cause deadlocks.
//!
//! # Problem
//! If function A acquires Lock1 then Lock2, and function B acquires Lock2 then Lock1,
//! a deadlock can occur when both functions are called concurrently.
//!
//! # Fix
//! Establish a consistent lock ordering across all functions that acquire
//! multiple locks. Consider using a helper function that acquires all locks
//! in the correct order.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};

/// CC_CONC003 Rule: Deadlock - nested locks acquired in inconsistent order
pub struct DeadlockRiskRule;

impl Default for DeadlockRiskRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for DeadlockRiskRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CONC003")
    }

    fn name(&self) -> &'static str {
        "Deadlock: nested locks acquired in inconsistent order"
    }

    fn description(&self) -> &'static str {
        "Multiple locks are acquired in different orders in different functions. \
         This can cause deadlocks. Use a consistent lock ordering or a helper \
         function that acquires all locks atomically."
    }

    fn category(&self) -> Category {
        Category::Correctness
    }

    fn severity(&self) -> Severity {
        Severity::Critical
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Rust]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Simple detection: look for functions that lock multiple mutexes in different orders
        // Pattern: two different functions each lock() two different static mutexes

        // Find all lock calls
        let mut function_locks: std::collections::HashMap<String, Vec<(String, usize)>> = std::collections::HashMap::new();

        for line in source.lines().enumerate() {
            let (line_num, line_str) = line;
            let trimmed = line_str.trim();

            // Look for lock calls on static items
            if trimmed.contains(".lock().unwrap()") || trimmed.contains(".read().unwrap()") || trimmed.contains(".write().unwrap()") {
                // Get the mutex name (the identifier before .lock)
                if let Some(dot_pos) = trimmed.find(".lock") {
                    let before_dot = &trimmed[..dot_pos];
                    // Get last identifier
                    if let Some(space_pos) = before_dot.rfind(' ') {
                        let mutex_name = before_dot[space_pos + 1..].trim();
                        if mutex_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                            // This looks like a static mutex (capitalized)
                            // Try to find the function this belongs to
                            for func_start in (0..line_num).rev() {
                                let check_line = source.lines().nth(func_start).unwrap_or("").trim();
                                if check_line.starts_with("fn ") || check_line.starts_with("async fn ") {
                                    let func_name_start = check_line.find("fn ").unwrap_or(0) + 3;
                                    let func_name_end = check_line[func_name_start..].find('(').unwrap_or(check_line.len() - func_name_start);
                                    let func_name = &check_line[func_name_start..func_name_start + func_name_end];

                                    function_locks.entry(func_name.to_string())
                                        .or_default()
                                        .push((mutex_name.to_string(), line_num + 1));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Now check for inconsistent ordering
        let orderings: Vec<(String, Vec<String>)> = function_locks.iter()
            .filter(|(_, locks)| locks.len() >= 2)
            .map(|(func, locks)| {
                (func.clone(), locks.iter().map(|(m, _)| m.clone()).collect())
            })
            .collect();

        for i in 0..orderings.len() {
            for j in (i + 1)..orderings.len() {
                let (func1, order1) = &orderings[i];
                let (func2, order2) = &orderings[j];

                if order1.len() == order2.len() && order1.len() >= 2 {
                    let is_reversed = order1.iter().zip(order2.iter().rev()).all(|(a, b)| a == b);
                    if is_reversed && order1.first() == order2.last() {
                        let line2 = function_locks.get(func2).and_then(|l| l.get(1)).map(|(_, l)| *l).unwrap_or(1);
                        issues.push(Issue::new(
                            "CC_CONC003",
                            "Deadlock risk: inconsistent lock order",
                            Severity::Critical,
                            Category::Correctness,
                            ctx.file_path.to_string_lossy(),
                            line2,
                            0,
                            &format!(
                                "Function '{}' acquires locks in order {:?} but '{}' acquires in reverse order {:?}. \
                                 This can cause deadlocks. Use consistent lock ordering.",
                                func1, order1, func2, order2
                            ),
                        ));
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["Mutex", "RwLock", "lock", "deadlock"])
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
        let rule = DeadlockRiskRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_inconsistent_lock_order() {
        let code = r#"
use std::sync::Mutex;

static LOCK_A: Mutex<()> = Mutex::new(());
static LOCK_B: Mutex<()> = Mutex::new(());

fn func1() {
    let a = LOCK_A.lock().unwrap();
    let b = LOCK_B.lock().unwrap();
}

fn func2() {
    let b = LOCK_B.lock().unwrap();
    let a = LOCK_A.lock().unwrap();
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect inconsistent lock ordering");
        assert_eq!(issues[0].rule_id, "CC_CONC003");
    }

    #[test]
    fn test_no_false_positive_consistent_order() {
        let code = r#"
use std::sync::Mutex;

static L1: Mutex<()> = Mutex::new(());
static L2: Mutex<()> = Mutex::new(());

fn func_a() {
    let l1 = L1.lock().unwrap();
    let l2 = L2.lock().unwrap();
}

fn func_b() {
    let l1 = L1.lock().unwrap();
    let l2 = L2.lock().unwrap();
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag consistent lock ordering");
    }

    #[test]
    fn test_no_false_positive_single_lock() {
        let code = r#"
use std::sync::Mutex;

static LOCK: Mutex<u32> = Mutex::new(0);

fn process() {
    let guard = LOCK.lock().unwrap();
    *guard += 1;
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag single lock");
    }
}
