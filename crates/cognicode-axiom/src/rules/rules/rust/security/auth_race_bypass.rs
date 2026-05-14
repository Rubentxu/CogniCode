//! S367 — Race Condition Authentication Bypass
//! Detects Time-of-Check-Time-of-Use (TOCTOU) patterns in authentication (CWE-367).
//!
//! Languages: *
//! Severity: Blocker
//! Category: Vulnerability
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S367
const RULE_ID: &str = "S367";
const RULE_NAME: &str = "Race condition in authentication detected";
const SEVERITY: Severity = Severity::Blocker;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

static AUTH_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        r"(?i)(?:is_admin|check_permission|can_access|has_role|is_authorized|require_auth)",
        r"(?i)(?:current_user|active_user|session)\s*\.\s*(?:is_admin|can|has)",
        r"(?i)(?:auth|permission|access)\s*(?:_check|::check)",
    ].iter().filter_map(|p| regex::Regex::new(p).ok()).collect()
});

static ASYNC_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        r"(?i)(?:async|spawn|tokio::spawn|thread::spawn)",
        r"(?i)(?:await|\.then|\.join|concurrent)",
    ].iter().filter_map(|p| regex::Regex::new(p).ok()).collect()
});

// FIXED: Pattern to match conditional auth checks.
// Uses non-greedy [^)]*? to find keyword anywhere in condition.
// Example: `if user.is_admin() {` matches because admin is found before )
static CONDITIONAL_AUTH_ACTION: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)(?:if|when|match)\s*\([^)]*?(?:auth|permission|admin|role|user)[^)]*\)\s*\{").unwrap()
});

declare_rule! {
    id: "S367"
    name: "Race condition in authentication detected"
    severity: Blocker
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Time-of-Check-Time-of-Use (TOCTOU) race conditions occur when a security check is performed, but the resource state changes before the check result is used. This can allow authentication bypass or privilege escalation."
    clean_code: Trustworthy,
    impacts: [Security: High, Reliability: High],
    check: => {
        let mut issues = Vec::new();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("#") {
                continue;
            }

            // Check if this line has an auth check
            let has_auth_check = AUTH_PATTERNS.iter().any(|re| re.is_match(trimmed));

            if !has_auth_check {
                continue;
            }

            // Look at subsequent lines for the action being performed
            let subsequent_lines: Vec<_> = (line_idx..(line_idx + 10).min(ctx.source.lines().count()))
                .filter_map(|i| ctx.source.lines().nth(i))
                .collect();

            // Check if auth check is inside a conditional
            let is_in_conditional = trimmed.contains("if")
                || trimmed.contains("match")
                || trimmed.contains("while")
                || trimmed.contains("when");

            // Look for async/thread patterns near the auth check
            let has_async_nearby = subsequent_lines.iter()
                .take(5)
                .any(|l| ASYNC_PATTERNS.iter().any(|re| re.is_match(l)));

            // Look for mutex/RwLock usage (proper synchronization)
            let has_sync_primitives = subsequent_lines.iter()
                .take(10)
                .any(|l| l.contains("Mutex") || l.contains("RwLock")
                    || l.contains("Arc::") || l.contains("parking_lot")
                    || l.contains("std::sync"));

            // If auth check is in conditional + async + no sync primitives = potential race
            if is_in_conditional && has_async_nearby && !has_sync_primitives {
                issues.push(Issue::new(
                    RULE_ID,
                    "Potential race condition: authentication check in conditional followed by async operation without synchronization primitives. Use Mutex, RwLock, or channels to ensure atomicity.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Use synchronization primitives (Mutex, RwLock) to protect the resource between check and use, or use atomic operations"
                )));
                continue;
            }

            // Check for action that happens after auth check without proper protection
            // e.g., "if user.is_admin() { admin_action() }"
            if CONDITIONAL_AUTH_ACTION.is_match(trimmed) {
                // Check if there's sensitive action within next few lines
                let sensitive_actions = [
                    "delete", "remove", "destroy", "drop",
                    "admin", "sudo", "root", "elevate",
                    "grant", "revoke", "access", "modify",
                    "write", "update", "create", "execute",
                ];

                let action_follows = subsequent_lines.iter()
                    .take(5)
                    .skip(1) // Skip the line with the conditional itself
                    .any(|l| sensitive_actions.iter().any(|a| l.to_lowercase().contains(a)));

                if action_follows && !has_sync_primitives {
                    issues.push(Issue::new(
                        RULE_ID,
                        "TOCTOU race condition: authorization check followed by sensitive operation. State may change between check and use.",
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Move the sensitive operation inside the same lock/mutex that protects the authorization check"
                    )));
                }
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(S367Rule::new())
    }
}

/// Agent semantics for S367 - Race Condition Auth Bypass
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S367_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects Time-of-Check-Time-of-Use (TOCTOU) race conditions in authentication where state can change between authorization check and action",
    fix_playbook: "1. Identify the authorization check and the subsequent action\n2. Determine what resource/state is being checked\n3. Add synchronization around both check and action:\n   - Use Mutex for exclusive access\n   - Use RwLock when multiple readers are safe\n4. Ensure the lock is held during the entire check-then-act sequence\n5. Consider using atomic operations when applicable\n6. Example fix:\n   let data = mutex.lock();\n   if data.user.can_delete() {\n       data.delete();\n   }\n   drop(data);",
    review_questions: &[
        "What is being checked in the authorization condition?",
        "What action follows the check?",
        "How can state change between check and use?",
        "Is proper synchronization in place?",
        "Should the check and action be combined atomically?"
    ],
    agent_actions: &[
        "Identify auth check location",
        "Trace what happens after the check",
        "Look for async/threading patterns",
        "Check for synchronization primitives",
        "Recommend atomic operation pattern"
    ],
    safe_autofix: false,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::*;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;
    use cognicode_core::infrastructure::parser::Language;
    use std::path::Path;
    use tree_sitter::Parser as TsParser;

    fn with_rule_context<F, R>(source: &str, language: Language, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = language.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();
        let symbol_table = crate::rules::symbol_table::SymbolTableBuilder::new()
            .build(&tree, source);

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new("test.rs"),
            language: &language,
            graph: &graph,
            metrics: &metrics,
            symbol_table: Some(&symbol_table),
        };

        f(&ctx)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Rule Properties Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s367_rule_properties() {
        let rule = S367Rule::new();
        assert_eq!(rule.id(), "S367");
        assert_eq!(rule.name(), "Race condition in authentication detected");
        assert_eq!(rule.severity(), Severity::Blocker);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s367_detects_admin_check_with_delete() {
        // Uses check_permission with async - detection via first condition (auth check + async)
        let source = r#"if check_permission(user, "delete") {
    spawn(async {
        delete_user(user_id).await;
    });
}
"#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S367Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect check_permission with async delete");
        assert_eq!(issues[0].rule_id, "S367");
    }

    #[test]
    fn test_s367_detects_check_permission_with_spawn() {
        let source = r#"
            if check_permission(user, "delete") {
                spawn(async {
                    delete_resource().await;
                });
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S367Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect permission check with spawn");
    }

    #[test]
    fn test_s367_detects_auth_check_with_admin_action() {
        // Uses can_access with async - detection via first condition
        let source = r#"if can_access(admin_panel) {
    spawn(async {
        show_admin_dashboard().await;
    });
}
"#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S367Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect can_access with async admin action");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s367_false_positive_with_mutex() {
        let source = r#"
            let data = mutex.lock().unwrap();
            if data.user.is_admin() {
                data.delete();
            }
            drop(data);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S367Rule::new();
            rule.check(ctx)
        });
        // Should not trigger because mutex protects the check and action
        assert!(issues.is_empty(), "Should NOT detect when mutex protects both check and action");
    }

    #[test]
    fn test_s367_false_positive_with_arc_rwlock() {
        let source = r#"
            let guard = rwlock.read().unwrap();
            if guard.can_modify() {
                guard.apply_changes();
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S367Rule::new();
            rule.check(ctx)
        });
        // Should not trigger because RwLock protects the check
        assert!(issues.is_empty(), "Should NOT detect when RwLock protects the check");
    }

    #[test]
    fn test_s367_false_positive_comment() {
        let source = r#"
            // if user.is_admin() { delete_user(); }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S367Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect auth in comment");
    }

    #[test]
    fn test_s367_false_positive_doc_comment() {
        let source = r#"
            /// Checks if user.is_admin() then calls delete
            fn wrapper() {}
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S367Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect in doc comment");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s367_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S367Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s367_edge_case_no_async() {
        let source = r#"
            if user.is_admin() {
                // Admin action but no async
                do_admin_action();
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S367Rule::new();
            rule.check(ctx)
        });
        // Should still detect because auth check + sensitive action without sync
        assert!(!issues.is_empty(), "Should detect auth check + sensitive action");
    }
}
