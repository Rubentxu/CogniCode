//! CC_CONC005: RefCell borrowed across await
//!
//! Detects when a RefCell borrow is held across an .await point, which can
//! cause panics in async contexts due to RefCell's non-Send nature.
//!
//! # Problem
//! RefCell provides interior mutability with runtime borrow checking.
//! When a RefCell borrow is held across an .await point, and the future
//! is resumed on a different thread, it causes a panic because RefCell
//! is not Send.
//!
//! # Fix
//! Use Mutex or RwLock instead of RefCell for async code, or ensure the
//! borrow is released (goes out of scope) before any .await point.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};

/// CC_CONC005 Rule: RefCell borrowed across await
pub struct RefCellBorrowAcrossAwaitRule;

impl Default for RefCellBorrowAcrossAwaitRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for RefCellBorrowAcrossAwaitRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CONC005")
    }

    fn name(&self) -> &'static str {
        "RefCell borrowed across await: RefCell::borrow() held across .await"
    }

    fn description(&self) -> &'static str {
        "RefCell borrow is held across an .await point. RefCell is not Send, \
         so holding a borrow across .await can cause panics if the future \
         is resumed on a different thread. Use Mutex or RwLock instead."
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

        // Simple detection: look for RefCell + async + await + borrow
        let has_refcell = source.contains("RefCell");
        let has_async = source.contains("async fn");
        let has_await = source.contains(".await");

        if has_refcell && has_async && has_await {
            // Look for borrow() before .await in async functions
            let lines: Vec<&str> = source.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                // Check for borrow in async function
                if trimmed.contains("borrow()") || trimmed.contains("borrow_mut()") {
                    // Check if there's an await later in the same function
                    let remaining: String = lines[i..].iter().take(20).map(|s| *s).collect();
                    if remaining.contains(".await") {
                        // Check if there's a drop() before the await
                        let before_await = remaining.split(".await").next().unwrap_or("");
                        if before_await.contains("drop(") {
                            continue; // Drop found, skip this detection
                        }
                        issues.push(Issue::new(
                            "CC_CONC005",
                            "RefCell borrow across await",
                            Severity::Critical,
                            Category::Correctness,
                            ctx.file_path.to_string_lossy(),
                            i + 1,
                            0,
                            "RefCell borrow is held across an .await point. \
                             Use Mutex or RwLock instead for async code.",
                        ));
                        break; // Only report once
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["RefCell", "borrow", "await", "async"])
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
        let rule = RefCellBorrowAcrossAwaitRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_refcell_borrow_across_await() {
        let code = r#"
use std::cell::RefCell;
use std::future::Future;

async fn bad_read(refcell: &RefCell<u32>) -> u32 {
    let value = refcell.borrow();
    async_op().await;
    *value
}

async fn async_op() {}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect RefCell borrow across await");
        assert_eq!(issues[0].rule_id, "CC_CONC005");
    }

    #[test]
    fn test_no_false_positive_borrow_dropped_before_await() {
        let code = r#"
use std::cell::RefCell;

async fn good_read(refcell: &RefCell<u32>) -> u32 {
    let value = refcell.borrow();
    let result = *value;
    drop(value);
    async_op().await;
    result
}

async fn async_op() {}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag when borrow is explicitly dropped before await");
    }

    #[test]
    fn test_no_false_positive_no_async() {
        let code = r#"
use std::cell::RefCell;

fn sync_read(refcell: &RefCell<u32>) -> u32 {
    *refcell.borrow()
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag synchronous code");
    }

    #[test]
    fn test_no_false_positive_mutex_for_async() {
        let code = r#"
use std::sync::Mutex;

async fn good_async(mtx: &Mutex<u32>) -> u32 {
    let guard = mtx.lock().unwrap();
    let val = *guard;
    drop(guard);
    async_op().await;
    val
}

async fn async_op() {}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag Mutex usage in async");
    }
}
