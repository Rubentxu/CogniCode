//! S384 — Session Fixation Vulnerability Detection
//! Detects session IDs that are assigned without regeneration, allowing session fixation attacks (CWE-384).
//!
//! Languages: *
//! Severity: Critical
//! Category: Vulnerability
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

/// Rule constant for S384
const RULE_ID: &str = "S384";
const RULE_NAME: &str = "Session fixation vulnerability detected";
const SEVERITY: Severity = Severity::Critical;
const CATEGORY: Category = Category::Vulnerability;

declare_rule! {
    id: "S384"
    name: "Session fixation vulnerability detected"
    severity: Critical
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Session fixation attacks occur when an attacker can set or influence a user's session ID before authentication. If the application doesn't regenerate the session ID after login, the attacker can hijack the authenticated session."
    clean_code: Trustworthy,
    impacts: [Security: High],
    check: => {
        let mut issues = Vec::new();

        // Pattern 1: Direct assignment to session.id or similar without regeneration
        // Matches: session.id = ..., sessionID = ..., SID = ..., etc.
        let session_id_assignment_re = regex::Regex::new(
            r#"(?:\b|_)(?:session[_\-.]?id|SID|session[_\-.]?identifier)\s*[.=]"#
        ).unwrap();

        // Pattern 2: Cookie being set with a known value
        let cookie_set_re = regex::Regex::new(
            r#"(?i)Set-Cookie:\s*\w*[_-]?(?:session|SID|identifier)\w*="#
        ).unwrap();

        // Pattern 3: URL parameters containing session IDs being propagated
        let url_session_re = regex::Regex::new(
            r#"(?i)(?:PHPSESSID|JSESSIONID|ASP\.NET_SessionId|session_id|sessionid)=[^&"\s]+"#
        ).unwrap();

        // Pattern 4: Form hidden fields with session values
        let hidden_session_re = regex::Regex::new(
            r#"<input[^>]+type\s*=\s*["']hidden["'][^>]+(?:name|value)\s*=\s*["'][^"']*(?:session|sid)[^"']*["']"#
        ).unwrap();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("#") {
                continue;
            }

            // Check for session ID assignment
            if session_id_assignment_re.is_match(trimmed) {
                // Check if session.regenerate() or similar is called nearby (within 10 lines after)
                let has_regenerate = (0..10)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i + 1))
                    .take(10)
                    .any(|l| l.contains("regenerate") || l.contains("renew") || l.contains("invalidate"));

                if !has_regenerate {
                    issues.push(Issue::new(
                        RULE_ID,
                        "Session ID assigned without regeneration. Call session.regenerate() or session.renew() after authentication to prevent session fixation attacks.",
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Regenerate the session ID after successful authentication using session.regenerate() or session.renew()"
                    )));
                }
            }

            // Check for Set-Cookie with session ID
            if cookie_set_re.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "Cookie set with session identifier without secure flags. Ensure HttpOnly, Secure, and SameSite attributes are set.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Add HttpOnly, Secure, and SameSite attributes to session cookies"
                )));
            }

            // Check for URL session parameters
            if url_session_re.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "Session ID found in URL, which can be leaked via referrer headers. Avoid passing session IDs in URLs.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Use cookie-based sessions instead of URL parameters for session ID transmission"
                )));
            }

            // Check for hidden form fields with session
            if hidden_session_re.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "Session identifier in hidden form field. This can be intercepted and used for session fixation.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Use server-side session management and avoid embedding session IDs in forms"
                )));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(S384Rule::new())
    }
}

/// Agent semantics for S384 - Session Fixation
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S384_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects session fixation vulnerabilities where session IDs are assigned or set without regeneration, allowing attackers to hijack authenticated sessions",
    fix_playbook: "1. Identify where session IDs are being assigned or set\n2. Find the authentication successful point\n3. Add session.regenerate() or session.renew() call immediately after successful authentication\n4. Ensure old session data is transferred to new session if needed\n5. Verify HttpOnly, Secure, and SameSite flags are set on session cookies",
    review_questions: &[
        "Is the session ID regenerated after successful login?",
        "Are session cookies missing security flags (HttpOnly, Secure, SameSite)?",
        "Could an attacker set or influence the session ID before authentication?",
        "Is session data properly transferred after regeneration?"
    ],
    agent_actions: &[
        "Check if session.regenerate() is called after authentication",
        "Verify session cookie has secure flags",
        "Look for session ID assignment patterns in the codebase",
        "Identify authentication flow and session lifecycle"
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
    fn test_s384_rule_properties() {
        let rule = S384Rule::new();
        assert_eq!(rule.id(), "S384");
        assert_eq!(rule.name(), "Session fixation vulnerability detected");
        assert_eq!(rule.severity(), Severity::Critical);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s384_detects_session_id_assignment() {
        let source = r#"
            session.id = user_session_id;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S384Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect session.id assignment");
        assert_eq!(issues[0].rule_id, "S384");
        assert_eq!(issues[0].line, 2);
    }

    #[test]
    fn test_s384_detects_session_id_variable_assignment() {
        let source = r#"
            let SID = get_session_id();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S384Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect SID assignment");
    }

    #[test]
    fn test_s384_detects_session_identifier_assignment() {
        let source = r#"
            session_identifier = request.session_id();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S384Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect session_identifier assignment");
    }

    #[test]
    fn test_s384_detects_set_cookie_header() {
        let source = r#"
            Set-Cookie: session_id=abc123;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S384Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect Set-Cookie with session");
        assert_eq!(issues[0].rule_id, "S384");
    }

    #[test]
    fn test_s384_detects_url_session_parameter() {
        let source = r#"
            let url = format!("https://example.com?PHPSESSID={}", session_id);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S384Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect PHPSESSID in URL");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s384_false_positive_comment() {
        let source = r#"
            // session.id = user_session_id; // This is fine
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S384Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect session in comment");
    }

    #[test]
    fn test_s384_false_positive_regenerate_after() {
        let source = r#"
            session.id = get_session_id();
            session.regenerate();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S384Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect session assignment with regenerate nearby");
    }

    #[test]
    fn test_s384_false_positive_renew_after() {
        let source = r#"
            session.id = get_session_id();
            session.renew();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S384Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect session assignment with renew nearby");
    }

    #[test]
    fn test_s384_false_positive_doc_comment() {
        let source = r#"
            /// Sets session.id to the new value
            fn login() {}
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S384Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect session in doc comment");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s384_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S384Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s384_edge_case_single_line() {
        let source = "session.id = \"test\";";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S384Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect session.id on single line");
    }
}
