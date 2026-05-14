//! PERF_010 — Missing Drop/Deref Cleanup
//!
//! Detects structs holding resources (files, connections) without Drop impl,
//! causing resource leaks.

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use crate::rules::rules::rust::performance::perf_helpers::{
    count_brace_balance, find_brace_close,
};
use cognicode_macros::declare_rule;
use regex::Regex;

/// Rule constant for PERF_010
const RULE_ID: &str = "PERF_010";

declare_rule! {
    id: "PERF_010"
    name: "Struct holding resources without implementing Drop"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects structs with resource handle fields (File, TcpStream, Connection, etc.) that don't implement Drop. Resources may leak if not explicitly closed."
    clean_code: Clear,
    impacts: [Reliability: Medium, Maintainability: Low],

    agent_semantics: {
        summary: "Detects structs with resource fields but no Drop implementation",
        fix_playbook: "1. Implement Drop trait for the struct\n2. Or use a wrapper type that already handles cleanup\n3. Or use Arc/Mutex for reference-counted cleanup\n4. Consider if the resource can be made to cleanup via RAII",
        review_questions: [
            "Is this struct intentionally not cleaning up resources?",
            "Is there a wrapper type that could handle this?",
            "Should this use reference counting instead?"
        ],
        semantic_chunks: [
            "Types like File, TcpStream, Connection hold OS resources",
            "Drop ensures resources are cleaned up when struct goes out of scope",
            "RAII pattern: resource acquisition in constructor, release in Drop"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires understanding the resource lifecycle"
    }

    check: => {
        detect_missing_drop(&ctx)
    }
}

// Resource types that require cleanup
static RESOURCE_TYPES: &[&str] = &[
    "File", "TcpStream", "TcpListener", "UdpSocket",
    "Connection", "Pool", "Client", "Session",
    "Handle", "Socket", "Channel", "Lock", "RwLock",
];
static STRUCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:pub\s+)?struct\s+(\w+)\s*\{").unwrap()
});
static IMPL_DROP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"impl\s+(?:Drop\s+for\s+(\w+))").unwrap()
});
static DERIVE_DROP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#\[derive\([^\]]*Drop[^\]]*\)\]").unwrap()
});

use std::sync::LazyLock;

/// Detects structs with resource fields but no Drop.
fn detect_missing_drop(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    for struct_cap in STRUCT_RE.captures_iter(source) {
        let struct_name = struct_cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let struct_start = struct_cap.get(0).map(|m| m.end()).unwrap_or(0);

        let brace_count = count_brace_balance(source, struct_start - 1);
        if let Some(struct_end) = find_brace_close(source, struct_start - 1, brace_count) {
            let struct_body = &source[struct_start - 1..struct_end.min(source.len())];

            let has_resource_field = RESOURCE_TYPES.iter().any(|r| struct_body.contains(r));

            if has_resource_field {
                let struct_cap_start = struct_cap.get(0).map(|m| m.start()).unwrap_or(0);
                let before_struct = &source[..struct_cap_start];

                // Check for Drop impl
                let has_drop_impl = IMPL_DROP_RE.is_match(source) || DERIVE_DROP_RE.is_match(before_struct);
                // Check for Clone (resource handles often shouldn't be cloned)
                let has_clone = before_struct.contains("Clone")
                    || before_struct.contains("#[derive(Clone)]");

                if !has_drop_impl && !has_clone {
                    let line_num = source[..struct_cap_start].lines().count();
                    issues.push(Issue::new(
                        RULE_ID,
                        format!("Struct '{}' has resource fields but no Drop implementation", struct_name),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::substantial(
                        "Implement Drop trait or use a wrapper type that handles cleanup"
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
    fn test_perf_010_registered() {
        let rule = PERF_010Rule::new();
        assert_eq!(rule.id(), "PERF_010");
        assert!(rule.name().len() > 0);
    }
}
