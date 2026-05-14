//! PERF_008 — Sync in Async Blocking
//!
//! Detects synchronous blocking operations (file I/O, mutex lock, sleep)
//! inside async functions, blocking the async executor.

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use crate::rules::rules::rust::performance::perf_helpers::{
    count_brace_balance, find_brace_close,
};
use cognicode_macros::declare_rule;
use regex::Regex;

/// Rule constant for PERF_008
const RULE_ID: &str = "PERF_008";

declare_rule! {
    id: "PERF_008"
    name: "Synchronous blocking operation inside async function"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects synchronous blocking operations inside async functions. These block the entire async executor thread, preventing other tasks from running. Use async-aware alternatives like tokio::fs or tokio::sync::Mutex."
    clean_code: Clear,
    impacts: [Reliability: High, Maintainability: Medium],

    agent_semantics: {
        summary: "Detects blocking operations inside async functions",
        fix_playbook: "1. std::thread::sleep -> tokio::time::sleep\n2. std::fs::read/write -> tokio::fs::read/write\n3. std::sync::Mutex -> tokio::sync::Mutex\n4. std::io::Read/Write -> tokio::io::AsyncRead/AsyncWrite",
        review_questions: [
            "Is this function running in an async runtime?",
            "Are tokio async alternatives available?",
            "Is blocking truly in a hot path?"
        ],
        semantic_chunks: [
            "Blocking in async blocks the entire executor thread",
            "tokio::fs and tokio::sync provide async equivalents",
            "tokio::time::sleep yields to the executor during wait"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires ensuring tokio runtime and async alternatives are available"
    }

    check: => {
        detect_sync_in_async(&ctx)
    }
}

// Pre-compiled blocking patterns
static BLOCKING_PATTERNS: &[(&str, &str)] = &[
    (r"std::thread::sleep\s*\(", "std::thread::sleep"),
    (r"std::fs::(read|write|create|remove|rename|copy)\s*\(", "std::fs::*"),
    (r"std::io::(Read|Write)::\w+\s*\(", "std::io::Read/Write"),
    (r"std::net::(TcpStream|UdpSocket|IpAddr|AddrInfo)::\w+\s*\(", "std::net::*"),
];
static ASYNC_FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"async\s+fn\s+(\w+)\s*\([^)]*\)").unwrap()
});

use std::sync::LazyLock;

/// Detects sync operations in async functions.
fn detect_sync_in_async(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    for cap in ASYNC_FN_RE.captures_iter(source) {
        let fn_name = cap.get(1).map(|m| m.as_str()).unwrap_or("unknown");
        let fn_start = cap.get(0).map(|m| m.start()).unwrap_or(0);

        let brace_count = count_brace_balance(source, fn_start);
        if let Some(fn_end) = find_brace_close(source, fn_start, brace_count) {
            let fn_body = &source[fn_start..fn_end.min(source.len())];

            for (pattern, name) in BLOCKING_PATTERNS {
                let block_re = Regex::new(pattern).unwrap();
                if let Some(block_cap) = block_re.find(fn_body) {
                    let before_match = &fn_body[..block_cap.start()];
                    // Skip if it's a tokio/async_std equivalent
                    if before_match.contains("tokio::") || before_match.contains("async_std::") {
                        continue;
                    }
                    let line_num = source[..fn_start + block_cap.start()].lines().count();
                    issues.push(Issue::new(
                        RULE_ID,
                        format!("Blocking operation {} inside async fn '{}'", name, fn_name),
                        Severity::Critical,
                        Category::Bug,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::substantial(
                        "Use tokio::time::sleep, tokio::fs::*, or tokio::sync::Mutex instead"
                    )));
                }
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_008_registered() {
        let rule = PERF_008Rule::new();
        assert_eq!(rule.id(), "PERF_008");
        assert!(rule.name().len() > 0);
    }
}
