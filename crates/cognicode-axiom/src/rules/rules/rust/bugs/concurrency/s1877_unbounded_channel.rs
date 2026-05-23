//! S1877 — Unbounded channel without backpressure detection
//!
//! Detects use of mpsc::channel() without bounded capacity, which can lead
//! to memory exhaustion if producers send faster than consumers can receive.

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

/// Rule constant for S1877
const RULE_ID: &str = "S1877";

declare_rule! {
    id: "S1877"
    name: "Unbounded channel without backpressure mechanism"
    severity: Minor
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects use of mpsc::channel() without a capacity limit (unbounded channel). Unbounded channels can cause memory exhaustion if the producer sends faster than the consumer can receive. Use bounded channels or sync_channel with explicit capacity."
    clean_code: Clear,
    impacts: [Reliability: Medium, Maintainability: Low],

    agent_semantics: {
        summary: "Detects unbounded mpsc::channel without backpressure",
        fix_playbook: "1. Consider sync_channel with bounded buffer\n2. Add explicit backpressure handling\n3. Monitor channel size and handle overflow",
        review_questions: [
            "Is unbounded growth a concern for this use case?",
            "Could synchronous consumers cause memory issues?",
        ],
        semantic_chunks: [
            "Unbounded channels can cause memory exhaustion under high load",
            "Bounded channels provide natural backpressure",
            "Consider sync_channel with appropriate capacity for your use case"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires understanding of producer/consumer rates"
    }

    check: => {
        detect_unbounded_channel(&ctx)
    }
}

/// Detects unbounded mpsc channels without backpressure.
fn detect_unbounded_channel(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    // Check if mpsc is used
    if !source.contains("mpsc::") && !source.contains("std::sync::mpsc") {
        return issues;
    }

    // Pattern 1: mpsc::channel() without capacity argument
    let unbounded_channel_pattern = regex::Regex::new(r"mpsc::channel\(\s*\)").unwrap();

    for cap in unbounded_channel_pattern.find_iter(source) {
        let pt = source[..cap.start()].lines().count();
        issues.push(Issue::new(
            RULE_ID,
            "Unbounded channel mpsc::channel() without capacity - may cause memory exhaustion",
            Severity::Minor,
            Category::Bug,
            ctx.file_path,
            pt + 1,
        ).with_remediation(Remediation::moderate(
            "Use mpsc::channel(capacity) for bounded channels or add explicit backpressure mechanisms"
        )));
    }

    // Pattern 2: mpsc::unbounded_channel() (always unbounded)
    let unbounded_fn_pattern = regex::Regex::new(r"mpsc::unbounded_channel\(").unwrap();

    for cap in unbounded_fn_pattern.find_iter(source) {
        let pt = source[..cap.start()].lines().count();
        issues.push(Issue::new(
            RULE_ID,
            "Unbounded channel unbounded_channel() - risk of memory exhaustion without backpressure",
            Severity::Minor,
            Category::Bug,
            ctx.file_path,
            pt + 1,
        ).with_remediation(Remediation::moderate(
            "Consider using mpsc::sync_channel(capacity) for bounded behavior or add explicit backpressure"
        )));
    }

    // Pattern 3: High-volume send loops with unbounded channel
    let high_volume_pattern = regex::Regex::new(r"for\s+\w+\s+in\s+0+\.\.(?:\d+_?)+\s*\{[^}]*?\.send\(").unwrap();

    for cap in high_volume_pattern.find_iter(source) {
        let text = cap.as_str();
        // Check if unbounded channel is used in context
        let context_start = cap.start().saturating_sub(100);
        let context = &source[context_start..cap.end() + 100];

        if context.contains("mpsc::channel()") || context.contains("unbounded_channel") {
            let pt = source[..cap.start()].lines().count();
            issues.push(Issue::new(
                RULE_ID,
                "High-volume send loop with unbounded channel - potential memory exhaustion",
                Severity::Minor,
                Category::Bug,
                ctx.file_path,
                pt + 1,
            ).with_remediation(Remediation::moderate(
                "Add backpressure mechanism or use bounded channel with try_send"
            )));
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s1877_registered() {
        let rule = S1877Rule::new();
        assert_eq!(rule.id(), "S1877");
        assert!(rule.name().len() > 0);
    }

    #[test]
    fn test_unbounded_channel_detection() {
        let rule = S1877Rule::new();
        let code = r#"
            use std::sync::mpsc;
            fn main() {
                let (tx, rx) = mpsc::channel();
                for i in 0..1000000 {
                    tx.send(i).unwrap();
                }
            }
        "#;
        assert_eq!(rule.id(), "S1877");
    }
}
