//! S1875 — Channel closed send detection
//!
//! Detects .send() operations on channels that may be closed or disconnected
//! without proper error handling.

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

/// Rule constant for S1875
const RULE_ID: &str = "S1875";

declare_rule! {
    id: "S1875"
    name: "Channel closed: send on potentially disconnected channel"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects send() calls on mpsc channels where the Sender may have been dropped or the Receiver disconnected, without checking the result. This can cause panics in producer-consumer patterns."
    clean_code: Clear,
    impacts: [Reliability: High],

    agent_semantics: {
        summary: "Detects send() on closed/disconnected channel",
        fix_playbook: "1. Check channel lifetime - ensure sender outlives send\n2. Use try_send() for non-blocking, check result\n3. Consider buffering or mpsc::sync_channel",
        review_questions: [
            "Is the sender actually dropped before send?",
            "Is this intentional for signaling termination?",
        ],
        semantic_chunks: [
            "Channel send on closed channel causes panic",
            "Always check Result from send() or use try_send()",
            "Ensure sender lifetime exceeds all send operations"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires understanding of channel lifetime and error handling"
    }

    check: => {
        detect_channel_closed_send(&ctx)
    }
}

/// Detects send() calls that may operate on closed channels.
fn detect_channel_closed_send(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    // Check if mpsc is used
    if !source.contains("mpsc::") && !source.contains("std::sync::mpsc") {
        return issues;
    }

    // Pattern 1: tx.send(...) without unwrap/expect/if_let_err
    // Uses fancy_regex for lookahead support (?!)
    let send_pattern = match fancy_regex::Regex::new(r"(\w+)\.send\([^)]+\)(?!\.(unwrap|expect))") {
        Ok(re) => re,
        Err(_) => return issues, // Skip if regex is invalid
    };

    // Manual iteration since fancy_regex doesn't have find_iter
    let mut search_start = 0;
    while let Ok(Some(cap)) = send_pattern.find(&source[search_start..]) {
        let abs_start = search_start + cap.start();
        let text = cap.as_str();
        let tx_name = regex::Regex::new(r"(\w+)\.send").unwrap()
            .captures(text)
            .and_then(|m| m.get(1))
            .map(|x| x.as_str())
            .unwrap_or("tx");

        // Skip if there's proper error handling
        let full_line = source.lines().nth(source[..abs_start].lines().count()).unwrap_or("");
        if full_line.contains("if let Err") || full_line.contains("match ") {
            search_start = abs_start + 1;
            continue;
        }

        let pt = source[..abs_start].lines().count();
        issues.push(Issue::new(
            RULE_ID,
            format!("Send on channel '{}' may fail if receiver is dropped - result unchecked", tx_name),
            Severity::Critical,
            Category::Bug,
            ctx.file_path,
            pt + 1,
        ).with_remediation(Remediation::moderate(
            "Handle the Result from send() or use try_send() with proper error handling"
        )));

        search_start = abs_start + 1;
    }

    // Pattern 2: send after drop(tx)
    let drop_pattern = regex::Regex::new(r"drop\((\w+)\)").unwrap();
    let drop_positions: Vec<(usize, String)> = drop_pattern.captures_iter(source)
        .filter_map(|m| {
            m.get(1).map(|x| (x.start(), x.as_str().to_string()))
        })
        .collect();

    for (drop_pos, dropped_var) in &drop_positions {
        // Look for send on the same variable after the drop
        let after_drop = &source[*drop_pos..];
        let send_after = regex::Regex::new(&format!(r"{}\.send\(", dropped_var)).unwrap();

        if let Some(m) = send_after.find(after_drop) {
            let pt = source[..*drop_pos + m.start()].lines().count();
            issues.push(Issue::new(
                RULE_ID,
                format!("Send on '{}' after it has been dropped - channel is closed", dropped_var),
                Severity::Critical,
                Category::Bug,
                ctx.file_path,
                pt + 1,
            ).with_remediation(Remediation::moderate(
                "Ensure the Sender is not dropped before all sends complete"
            )));
        }
    }

    // Pattern 3: tx.clone() in thread spawn without proper synchronization
    let clone_thread_pattern = regex::Regex::new(r"let\s+\w+\s*=\s*(\w+)\.clone\(\)[^;]*?thread::spawn|thread::spawn[^;]*?let\s+\w+\s*=\s*(\w+)\.clone\(\)").unwrap();

    for cap in clone_thread_pattern.find_iter(source) {
        let text = cap.as_str();
        let tx_name = regex::Regex::new(r"(\w+)\.clone\(\)").unwrap()
            .captures(text)
            .and_then(|m| m.get(1))
            .map(|x| x.as_str())
            .unwrap_or("tx");

        // Check if this is an mpsc sender
        let context_start = cap.start().saturating_sub(200);
        let context = &source[context_start..cap.end()];
        if context.contains("mpsc::") || context.contains("std::sync::mpsc") {
            let pt = source[..cap.start()].lines().count();
            issues.push(Issue::new(
                RULE_ID,
                format!("Channel sender '{}' cloned into thread - ensure sender stays alive", tx_name),
                Severity::Critical,
                Category::Bug,
                ctx.file_path,
                pt + 1,
            ).with_remediation(Remediation::moderate(
                "Ensure the original Sender is kept alive until all clone senders have completed"
            )));
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s1875_registered() {
        let rule = S1875Rule::new();
        assert_eq!(rule.id(), "S1875");
        assert!(rule.name().len() > 0);
    }
}
