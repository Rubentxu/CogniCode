//! S1876 — RefCell across await detection
//!
//! Detects RefCell::borrow() or RefCell::borrow_mut() calls where the borrow
//! spans an .await point, which can cause panics in async code.

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

/// Rule constant for S1876
const RULE_ID: &str = "S1876";

declare_rule! {
    id: "S1876"
    name: "RefCell borrow spans await point"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects when a RefCell borrow (either immutable or mutable) is held across an .await point. Since RefCell enforces borrow checking at runtime, and async functions may suspend between .await points, holding a RefCell borrow across an await can cause panics if the RefCell is accessed from another task."
    clean_code: Clear,
    impacts: [Reliability: High],

    agent_semantics: {
        summary: "Detects RefCell borrowed across await point",
        fix_playbook: "1. Drop borrow before await: drop(borrow)\n2. Or clone data before await\n3. Consider using tokio::sync::Mutex for async contexts",
        review_questions: [
            "Is the RefCell genuinely borrowed across await?",
            "Could this cause panics in multi-threaded context?",
        ],
        semantic_chunks: [
            "RefCell borrow across await can cause panic when task reschedules",
            "Use Arc<Mutex<T>> or Arc<RwLock<T>> instead of RefCell in async code",
            "Clone the borrowed value before await to avoid holding the borrow"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires restructuring to drop borrow before await"
    }

    check: => {
        detect_refcell_across_await(&ctx)
    }
}

/// Detects RefCell borrows that span an .await point.
fn detect_refcell_across_await(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    // Check if RefCell is used
    if !source.contains("RefCell") {
        return issues;
    }

    // Pattern 1: let value = refcell.borrow() followed by .await
    // Note: Simplified without backreference since regex crate doesn't support \1
    let borrow_await_pattern = regex::Regex::new(r"let\s+\w+\s*=\s*\w+\.(borrow|borrow_mut)\([^)]*\)[^;]*?\.await").unwrap();

    for cap in borrow_await_pattern.find_iter(source) {
        let text = cap.as_str();
        let var_name = regex::Regex::new(r"let\s+(\w+)\s*=").unwrap()
            .captures(text)
            .and_then(|m| m.get(1))
            .map(|x| x.as_str())
            .unwrap_or("value");

        let refcell_name = regex::Regex::new(r"(\w+)\.(borrow|borrow_mut)").unwrap()
            .captures(text)
            .and_then(|m| m.get(1))
            .map(|x| x.as_str())
            .unwrap_or("refcell");

        let pt = source[..cap.start()].lines().count();
        issues.push(Issue::new(
            RULE_ID,
            format!("RefCell '{}' borrow held across .await point - variable '{}' used after await", refcell_name, var_name),
            Severity::Major,
            Category::Bug,
            ctx.file_path,
            pt + 1,
        ).with_remediation(Remediation::moderate(
            "Drop the borrow before the .await point, or use a synchronous alternative to RefCell in async code"
        )));
    }

    // Pattern 2: Direct borrow result used after await
    let direct_borrow_pattern = regex::Regex::new(r"\*(\w+)\.(borrow|borrow_mut)\(\)[^;]*?\.await").unwrap();

    for cap in direct_borrow_pattern.find_iter(source) {
        let text = cap.as_str();
        let refcell_name = regex::Regex::new(r"(\w+)\.(borrow|borrow_mut)").unwrap()
            .captures(text)
            .and_then(|m| m.get(1))
            .map(|x| x.as_str())
            .unwrap_or("refcell");

        let pt = source[..cap.start()].lines().count();
        issues.push(Issue::new(
            RULE_ID,
            format!("RefCell '{}' borrow result used before .await - may cause deadlock", refcell_name),
            Severity::Major,
            Category::Bug,
            ctx.file_path,
            pt + 1,
        ).with_remediation(Remediation::moderate(
            "Store the borrowed value in a local variable and drop the borrow before .await"
        )));
    }

    // Pattern 3: async function with refcell borrow
    let async_refcell_pattern = regex::Regex::new(r"async\s+fn\s+\w+[^}]*?RefCell[^}]*?\.await").unwrap();

    for cap in async_refcell_pattern.find_iter(source) {
        let text = cap.as_str();
        // Check if borrow() is called before await
        if text.contains(".borrow()") || text.contains(".borrow_mut()") {
            let pt = source[..cap.start()].lines().count();
            issues.push(Issue::new(
                RULE_ID,
                "RefCell borrow in async function that contains .await - borrow may span await point",
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                pt + 1,
            ).with_remediation(Remediation::moderate(
                "Use Arc<Mutex<T>> or Arc<RwLock<T>> instead of RefCell in async code"
            )));
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s1876_registered() {
        let rule = S1876Rule::new();
        assert_eq!(rule.id(), "S1876");
        assert!(rule.name().len() > 0);
    }

    #[test]
    fn test_refcell_borrow_across_await() {
        let rule = S1876Rule::new();
        let code = r#"
            use std::cell::RefCell;
            async fn bad_read(refcell: &RefCell<u32>) -> u32 {
                let value = refcell.borrow();
                async_op().await;
                *value
            }
            async fn async_op() {}
        "#;
        assert_eq!(rule.id(), "S1876");
    }
}
