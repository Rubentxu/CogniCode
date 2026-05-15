//! CC_CONC007: Arc::clone in hot path without justification
//!
//! Detects unnecessary Arc::clone() calls in hot paths (loops, recursive
//! functions) where the clone may not be needed.
//!
//! # Problem
//! Arc::clone() increments an atomic reference count, which has a cost.
//! Calling clone inside a tight loop or frequently called function can
//! cause unnecessary CPU overhead.
//!
//! # Fix
//! Clone Arc once before entering a loop and reuse the reference, or
//! reconsider if Arc is the right choice for the use case.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};

/// CC_CONC007 Rule: Arc::clone in hot path without justification
pub struct ArcCloneInHotPathRule;

impl Default for ArcCloneInHotPathRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for ArcCloneInHotPathRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CONC007")
    }

    fn name(&self) -> &'static str {
        "Arc::clone in hot path without justification"
    }

    fn description(&self) -> &'static str {
        "Arc::clone() is called inside a loop or hot path. Each clone increments \
         the atomic reference count, which adds overhead. Consider cloning once \
         before the loop or checking if Arc is necessary."
    }

    fn category(&self) -> Category {
        Category::Performance
    }

    fn severity(&self) -> Severity {
        Severity::Minor
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Rust]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Simple detection: look for .clone() inside for loops
        let lines: Vec<&str> = source.lines().collect();
        let mut in_loop = false;
        let mut loop_start = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Track loop boundaries
            if trimmed.starts_with("for ") {
                in_loop = true;
                loop_start = i;
            }
            if in_loop && (trimmed.starts_with("fn ") || trimmed.starts_with("async fn ")) {
                in_loop = false;
            }
            if in_loop && trimmed == "}" && i > loop_start + 2 {
                in_loop = false;
            }

            // Check for clone inside loop
            if in_loop && (trimmed.contains(".clone()") || trimmed.contains("Arc::clone")) {
                issues.push(Issue::new(
                    "CC_CONC007",
                    "Arc::clone in loop",
                    Severity::Minor,
                    Category::Performance,
                    ctx.file_path.to_string_lossy(),
                    i + 1,
                    0,
                    "Arc::clone() inside a loop. Consider cloning once before \
                     the loop or restructuring to avoid unnecessary clones.",
                ));
                in_loop = false; // Only report once per loop
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["Arc", "clone", "thread", "loop"])
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
        let rule = ArcCloneInHotPathRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_arc_clone_in_loop() {
        let code = r#"
use std::sync::Arc;
use std::thread;

fn process_items() {
    let data = Arc::new(vec![1, 2, 3, 4, 5]);

    for i in 0..1000 {
        let data_clone = data.clone();
        thread::spawn(move || {
            let _ = data_clone;
        });
    }
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect Arc::clone in loop");
        assert_eq!(issues[0].rule_id, "CC_CONC007");
    }

    #[test]
    fn test_no_false_positive_clone_once_before_loop() {
        let code = r#"
use std::sync::Arc;
use std::thread;

fn efficient_clone() {
    let data = Arc::new(vec![1, 2, 3]);
    let data_clone = data.clone();

    let handles: Vec<_> = (0..4).map(|_| {
        let d = data_clone.clone();
        thread::spawn(move || { *d })
    }).collect();

    for h in handles { h.join().unwrap(); }
}
"#;
        let issues = check_rule(code);
        // Clone is done once before the loop, then clones from that
        // This is acceptable - but our simple detector may still flag it
        // That's acceptable for now
        assert!(issues.is_empty(), "Should not flag clone once before loop");
    }

    #[test]
    fn test_no_false_positive_single_clone() {
        let code = r#"
use std::sync::Arc;

fn single_use() {
    let data = Arc::new(42);
    let clone = data.clone();
    println!("{}", *clone);
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag single clone outside loop");
    }
}
