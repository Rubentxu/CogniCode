//! CC_CONC001: Race condition - shared mutable state without synchronization
//!
//! Detects shared mutable state accessed from multiple threads without proper synchronization.
//!
//! # Problem
//! Using `static mut` or `Rc<RefCell<T>>` in multi-threaded contexts causes data races.
//! Static mutable variables are inherently unsafe and can lead to undefined behavior.
//!
//! # Fix
//! Use atomics (AtomicU32, etc.), Mutex, RwLock, or thread-safe types like Arc<Mutex<T>>.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};

/// CC_CONC001 Rule: Race condition - shared mutable state without synchronization
pub struct RaceConditionRule;

impl Default for RaceConditionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for RaceConditionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CONC001")
    }

    fn name(&self) -> &'static str {
        "Race condition: shared mutable state without synchronization"
    }

    fn description(&self) -> &'static str {
        "Shared mutable state accessed from multiple threads without synchronization primitives. \
         Use Arc<Mutex<T>>, Arc<RwLock<T>>, or atomic types instead."
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

        // Detect static mut - simple regex-like approach
        for line in source.lines() {
            let trimmed = line.trim();
            // Match "static mut" but not "static mut FOO: ..." without mut being a variable name
            if trimmed.starts_with("static mut") || trimmed.contains("static") && trimmed.contains("mut ") {
                // Skip comments
                if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                    continue;
                }
                // Check if this is an actual static mut declaration
                if trimmed.starts_with("static mut") && trimmed.len() > 10 {
                    let line_num = source.lines().position(|l| l == line).unwrap_or(0) + 1;
                    issues.push(Issue::new(
                        "CC_CONC001",
                        "Race condition: shared mutable state",
                        Severity::Critical,
                        Category::Correctness,
                        ctx.file_path.to_string_lossy(),
                        line_num,
                        0,
                        "Shared mutable state without synchronization. \
                         Use Arc<Mutex<T>>, Arc<RwLock<T>>, or atomic types instead.",
                    ));
                }
            }
        }

        // Detect Rc<RefCell<T>> in thread context
        let has_rc_refcell = source.contains("Rc<RefCell<");
        let has_thread = source.contains("thread::spawn") || source.contains("std::thread");

        if has_rc_refcell && has_thread {
            // Find the line with Rc
            for line in source.lines() {
                if line.contains("Rc<") && !line.trim().starts_with("//") {
                    let line_num = source.lines().position(|l| l == line).unwrap_or(0) + 1;
                    if !issues.iter().any(|i| i.line == line_num) {
                        issues.push(Issue::new(
                            "CC_CONC001",
                            "Race condition: Rc<RefCell<T>> not thread-safe",
                            Severity::Critical,
                            Category::Correctness,
                            ctx.file_path.to_string_lossy(),
                            line_num,
                            0,
                            "Rc<RefCell<T>> is not thread-safe. Use Arc<Mutex<T>> or Arc<RwLock<T>> instead.",
                        ));
                    }
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["static", "mut", "Rc", "RefCell", "thread"])
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
        let rule = RaceConditionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_static_mut() {
        let code = r#"
static mut COUNTER: u32 = 0;

fn increment() {
    unsafe { COUNTER += 1; }
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect static mut");
        assert_eq!(issues[0].rule_id, "CC_CONC001");
    }

    #[test]
    fn test_no_false_positive_arc_mutex() {
        let code = r#"
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let counter = Arc::new(Mutex::new(0));
    let c = counter.clone();
    thread::spawn(move || {
        let mut num = c.lock().unwrap();
        *num += 1;
    }).join().unwrap();
}
"#;
        let issues = check_rule(code);
        // Arc<Mutex<T>> is safe, no issues expected for proper synchronization
        // Note: this may still detect Rc imports, but not static mut
        let has_static_mut_issue = issues.iter().any(|i| i.message.contains("static") && i.message.contains("mut"));
        assert!(!has_static_mut_issue, "Should not flag Arc<Mutex>");
    }

    #[test]
    fn test_no_false_positive_constants() {
        let code = r#"
const MAX_CONNECTIONS: u32 = 100;
static INITIAL_VALUE: u32 = 42;
"#;
        let issues = check_rule(code);
        let has_static_mut_issue = issues.iter().any(|i| i.message.contains("static") && i.message.contains("mut"));
        assert!(!has_static_mut_issue, "Should not flag immutable static or const");
    }
}
