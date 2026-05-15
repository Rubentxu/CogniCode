//! CC_CONC006: Unbounded channel without backpressure mechanism
//!
//! Detects when an unbounded mpsc channel is used without any backpressure
//! mechanism, which can lead to memory exhaustion.
//!
//! # Problem
//! Unbounded channels can grow indefinitely if the receiver cannot keep up
//! with the sender. This can lead to memory exhaustion and OOM conditions.
//!
//! # Fix
//! Use bounded channels with capacity limits, sync_channel (which has
//! internal backpressure), or implement external backpressure using
//! semaphores or token buckets.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};

/// CC_CONC006 Rule: Unbounded channel without backpressure
pub struct UnboundedChannelRule;

impl Default for UnboundedChannelRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for UnboundedChannelRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CONC006")
    }

    fn name(&self) -> &'static str {
        "Unbounded channel without backpressure mechanism"
    }

    fn description(&self) -> &'static str {
        "An unbounded mpsc::channel() is used without backpressure. \
         If the receiver cannot keep up, this can cause memory exhaustion. \
         Use bounded channels, sync_channel, or implement backpressure."
    }

    fn category(&self) -> Category {
        Category::Performance
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

        // Detect mpsc::channel() without capacity argument
        for line in source.lines().enumerate() {
            let (line_num, line_str) = line;
            let trimmed = line_str.trim();

            // Look for mpsc::channel() without arguments (unbounded)
            if trimmed.contains("mpsc::channel()") || trimmed.contains("channel::<") && !trimmed.contains(",") {
                issues.push(Issue::new(
                    "CC_CONC006",
                    "Unbounded channel without backpressure",
                    Severity::Major,
                    Category::Performance,
                    ctx.file_path.to_string_lossy(),
                    line_num + 1,
                    0,
                    "Unbounded mpsc::channel() without capacity limit. \
                     Use mpsc::channel(capacity) for bounded channels or \
                     mpsc::sync_channel(capacity) for synchronous channels with backpressure.",
                ));
            }
        }

        // Also check for sync_channel with capacity 0
        for line in source.lines().enumerate() {
            let (line_num, line_str) = line;
            let trimmed = line_str.trim();

            if trimmed.contains("sync_channel(0)") {
                issues.push(Issue::new(
                    "CC_CONC006",
                    "Zero-capacity sync_channel provides no buffering",
                    Severity::Minor,
                    Category::Performance,
                    ctx.file_path.to_string_lossy(),
                    line_num + 1,
                    0,
                    "sync_channel with capacity 0 provides no buffering. \
                     Consider a small positive capacity for better performance.",
                ));
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["mpsc", "channel", "sync_channel", "backpressure"])
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
        let rule = UnboundedChannelRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_unbounded_channel() {
        let code = r#"
use std::sync::mpsc;

fn main() {
    let (tx, rx) = mpsc::channel();
    tx.send(1).unwrap();
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect unbounded channel");
        assert_eq!(issues[0].rule_id, "CC_CONC006");
    }

    #[test]
    fn test_no_false_positive_bounded_channel() {
        let code = r#"
use std::sync::mpsc;

fn bounded_channel() {
    let (tx, rx) = mpsc::channel::<u32>(10);
    tx.send(1).unwrap();
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag bounded channel");
    }

    #[test]
    fn test_no_false_positive_sync_channel() {
        let code = r#"
use std::sync::mpsc;

fn synchronous_channel() {
    let (tx, rx) = mpsc::sync_channel(1);
    tx.send(1).unwrap();
    rx.recv().unwrap();
}
"#;
        let issues = check_rule(code);
        // sync_channel with capacity is OK
        assert!(issues.is_empty(), "Should not flag sync_channel with capacity");
    }
}
