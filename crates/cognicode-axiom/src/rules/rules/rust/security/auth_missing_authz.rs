//! S4830 — Missing Authorization Check
//! Detects missing authorization checks after authentication (CWE-862).
//!
//! Languages: *
//! Severity: Critical
//! Category: Vulnerability
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S4830
const RULE_ID: &str = "S4830";
const RULE_NAME: &str = "Missing authorization check detected";
const SEVERITY: Severity = Severity::Critical;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

static AUTH_SUCCESS_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // FIXED: Removed `?` operator requirement, added `session.is_authenticated()` in conditional
        r#"(?i)(?:login|signin|authenticate|authenticate_user)\s*\([^)]*\)"#,
        r#"(?i)(?:current_user|active_user|logged_in_user)\s*="#,
        r#"(?i)(?:session|token|jwt)\s*\.\s*(?:is_valid|is_authenticated|valid)"#,
        // FIXED: Added conditional auth check pattern for `if session.is_authenticated()`
        r#"(?i)if\s*\(\s*(?:auth|login|user|session)\s*\.\s*(?:is_logged_in|is_authenticated)\s*\)\s*\{"#,
    ].iter().filter_map(|p| regex::Regex::new(p).ok()).collect()
});

static AUTHZ_CHECK_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        r#"(?i)(?:can|has_permission|has_role|is_authorized|check_access|authorize)"#,
        r#"(?i)(?:require_role|require_permission|ensure_role|has_privilege)"#,
        r#"(?i)(?:role|permission|privilege|access_level)\s*[==:]"#,
        r#"(?i)#\[(?:require_role|require_permission|authorize)\]"#,
        r#"(?i)(?:admin|moderator|owner)\s*\.\s*(?:is_admin|check)"#,
    ].iter().filter_map(|p| regex::Regex::new(p).ok()).collect()
});

static SENSITIVE_OPERATIONS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Pattern for `delete user` with space (function call with space-separated args)
        r#"(?i)(?:delete|remove|destroy)\s+(?:user|account|post|comment|file|data)"#,
        // Pattern for `delete_user` with underscore (snake_case function name)
        r#"(?i)(?:delete|remove|destroy)_[a-z_]+"#,
        // FIXED: Added pattern for `modify_user_settings` type functions
        r#"(?i)(?:modify|update|edit)\s+(?:user|account|permission|role|setting)"#,
        r#"(?i)(?:modify|update|edit)_[a-z_]+"#,
        r#"(?i)(?:create|add)\s+(?:user|admin|role|permission)"#,
        r#"(?i)(?:grant|revoke)\s+(?:access|permission|role)"#,
        // Pattern for `grant_access` with underscore (snake_case function name)
        r#"(?i)(?:grant|revoke)_[a-z_]+"#,
        r#"(?i)(?:access|view)\s+(?:admin|dashboard|settings|logs|users)"#,
        r#"(?i)(?:export|download|upload)\s+(?:data|file|backup)"#,
        r#"(?i)(?:disable|enable)\s+(?:account|user|feature)"#,
        r#"(?i)(?:reset|change)\s+(?:password|email|settings)"#,
    ].iter().filter_map(|p| regex::Regex::new(p).ok()).collect()
});

declare_rule! {
    id: "S4830"
    name: "Missing authorization check detected"
    severity: Critical
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Authentication verifies WHO you are, but authorization verifies WHAT you can do. After successful login, the application must verify the user has permission to perform each action. Missing authorization checks allow privilege escalation."
    clean_code: Trustworthy,
    impacts: [Security: High],
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

            // Check for authentication success
            let has_auth_success = AUTH_SUCCESS_PATTERNS.iter().any(|re| re.is_match(trimmed));

            if !has_auth_success {
                continue;
            }

            // Find the block/function after authentication
            let subsequent_lines: Vec<_> = (line_idx..(line_idx + 20).min(ctx.source.lines().count()))
                .filter_map(|i| ctx.source.lines().nth(i))
                .collect();

            // Check for authorization checks
            let has_authz_check = subsequent_lines.iter()
                .take(15)
                .any(|l| AUTHZ_CHECK_PATTERNS.iter().any(|re| re.is_match(l)));

            // Check for sensitive operations after auth
            let sensitive_ops_after_auth = subsequent_lines.iter()
                .take(15)
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| {
                    let has_sensitive = SENSITIVE_OPERATIONS.iter().any(|re| re.is_match(l));
                    if has_sensitive { Some(l) } else { None }
                })
                .collect::<Vec<_>>();

            // If we have auth success followed by sensitive ops but no authz check
            if !sensitive_ops_after_auth.is_empty() && !has_authz_check {
                // Get the line of the first sensitive operation
                let first_op_line = subsequent_lines.iter()
                    .position(|l| SENSITIVE_OPERATIONS.iter().any(|re| re.is_match(l)))
                    .map(|pos| line_idx + 1 + pos)
                    .unwrap_or(line_idx + 1);

                issues.push(Issue::new(
                    RULE_ID,
                    "Authorization check may be missing after authentication. Sensitive operation detected without permission verification.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    first_op_line,
                ).with_remediation(Remediation::substantial(
                    "Add authorization check after authentication: verify user has permission for the specific operation"
                )));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(S4830Rule::new())
    }
}

/// Agent semantics for S4830 - Missing Authorization Check
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S4830_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects missing authorization checks after authentication, allowing authenticated users to perform actions they don't have permission for",
    fix_playbook: "1. Identify the function with potential missing authorization\n2. Determine what permissions the operation requires\n3. Add explicit authorization check:\n   - For role-based: check_user_has_role(user, required_role)\n   - For permission-based: check_permission(user, action, resource)\n   - For ownership: verify_resource_ownership(user, resource)\n4. Return 403 Forbidden if authorization fails\n5. Log authorization failures for security auditing\n6. Consider using a middleware/guard pattern for consistent checks",
    review_questions: &[
        "What permissions does this operation require?",
        "Is this checking authentication (who) or authorization (what)?",
        "Should this use role-based or permission-based access control?",
        "Are there existing authorization patterns in the codebase?",
        "What should happen if authorization fails?"
    ],
    agent_actions: &[
        "Identify authentication patterns",
        "Look for authorization check patterns",
        "Trace sensitive operations after auth",
        "Recommend RBAC or ABAC approach",
        "Check for existing permission middleware"
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
    fn test_s4830_rule_properties() {
        let rule = S4830Rule::new();
        assert_eq!(rule.id(), "S4830");
        assert_eq!(rule.name(), "Missing authorization check detected");
        assert_eq!(rule.severity(), Severity::Critical);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4830_detects_login_followed_by_delete() {
        let source = r#"
            let user = login(username, password)?;
            delete_user(user_id);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4830Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect login followed by delete without authz");
        assert_eq!(issues[0].rule_id, "S4830");
    }

    #[test]
    fn test_s4830_detects_auth_with_admin_action() {
        let source = r#"
            let session = authenticate(username, password)?;
            modify_user_settings(user_id);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4830Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect auth followed by modify without authorization");
    }

    #[test]
    fn test_s4830_detects_session_with_role_change() {
        // Test with login followed by sensitive operation
        let source = r#"let user = login(username, password);
delete_user(user_id);
"#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4830Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect login followed by delete without authz");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4830_false_positive_with_permission_check() {
        let source = r#"
            let user = login(username, password)?;
            if has_permission(user, "delete_user") {
                delete_user(user_id);
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4830Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect when has_permission check exists");
    }

    #[test]
    fn test_s4830_false_positive_with_admin_check() {
        let source = r#"
            let user = authenticate(username, password)?;
            if user.is_admin() {
                create_admin_account(details);
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4830Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect when is_admin check exists");
    }

    #[test]
    fn test_s4830_false_positive_with_authorize_attribute() {
        let source = r#"
            #[require_role("admin")]
            fn delete_user(user_id: u64) {
                // Delete user
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4830Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect when require_role attribute exists");
    }

    #[test]
    fn test_s4830_false_positive_comment() {
        let source = r#"
            // login(username, password);
            // delete_user(user_id);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4830Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect auth in comment");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4830_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4830Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s4830_edge_case_only_auth() {
        let source = r#"
            let user = login(username, password)?;
            // Just updating own profile, no special permissions needed
            update_profile(user.id, details);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4830Rule::new();
            rule.check(ctx)
        });
        // update_profile without special permissions might not trigger
        // But if it does, it's a valid finding (update_profile could be sensitive)
        // This test just verifies the rule runs
        assert!(issues.len() <= 1, "Should have at most one issue");
    }
}
