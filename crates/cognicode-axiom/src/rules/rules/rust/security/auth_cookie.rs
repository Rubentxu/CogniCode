//! S1004 — Insecure Cookie Flags Detection
//! Detects cookies set without HttpOnly, Secure, or SameSite flags (CWE-1004).
//!
//! Languages: *
//! Severity: Major
//! Category: Vulnerability
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S1004
const RULE_ID: &str = "S1004";
const RULE_NAME: &str = "Insecure cookie flags detected";
const SEVERITY: Severity = Severity::Major;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

static SET_COOKIE_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)Set-Cookie:\s*[^;]+"#).unwrap()
});

static COOKIE_NEW_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Rust cookie patterns - FIXED: removed (?:name|value) requirement for positional args
        regex::Regex::new(r#"(?i)Cookie::(?:new|from)\s*\("#).unwrap(),
        // JavaScript document.cookie
        regex::Regex::new(r#"(?i)document\.cookie\s*="#).unwrap(),
        // Python cookie creation
        regex::Regex::new(r#"(?i)(?:Cookie|SimpleCookie|CookieJar)\s*\([^)]*\)\s*(?:\.set|\.update|\[)"#).unwrap(),
        // Java response.setCookie
        regex::Regex::new(r#"(?i)response\.addCookie\s*\([^)]+\)"#).unwrap(),
        // .cookie or Set-Cookie in various frameworks
        regex::Regex::new(r#"(?i)\.cookie\s*[=:]\s*["'][^"']+["']"#).unwrap(),
    ]
});

static HEADER_COOKIE_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)(?:header|Header|response)[^;]*(?:Set-Cookie|set-cookie)"#).unwrap()
});

declare_rule! {
    id: "S1004"
    name: "Insecure cookie flags detected"
    severity: Major
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Cookies set without security flags are vulnerable to attacks. Without HttpOnly, JavaScript can access the cookie (XSS). Without Secure, the cookie is sent over HTTP. Without SameSite, the cookie can be sent in cross-site requests (CSRF)."
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

            // Check for Set-Cookie header
            if SET_COOKIE_PATTERN.is_match(trimmed) {
                let has_httponly = trimmed.to_lowercase().contains("httponly");
                let has_secure = trimmed.to_lowercase().contains("secure");
                let has_samesite = trimmed.to_lowercase().contains("samesite");

                let missing_flags = if !has_httponly && !has_secure && !has_samesite {
                    "HttpOnly, Secure, and SameSite"
                } else if !has_httponly && !has_secure {
                    "HttpOnly and Secure"
                } else if !has_httponly && !has_samesite {
                    "HttpOnly and SameSite"
                } else if !has_secure && !has_samesite {
                    "Secure and SameSite"
                } else if !has_httponly {
                    "HttpOnly"
                } else if !has_secure {
                    "Secure"
                } else {
                    "SameSite"
                };

                if !has_httponly || !has_secure || !has_samesite {
                    issues.push(Issue::new(
                        RULE_ID,
                        format!("Cookie set without security flags. Missing: {}.", missing_flags),
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Add HttpOnly, Secure, and SameSite attributes to cookie: 'HttpOnly; Secure; SameSite=Strict'"
                    )));
                }
            }

            // Check for cookie constructor patterns using cached regexes
            for re in COOKIE_NEW_PATTERNS.iter() {
                if re.is_match(trimmed) {
                    // Look ahead to see if security flags are set
                    let cookie_context: String = (0..5)
                        .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                        .take(6)
                        .collect::<Vec<_>>()
                        .join("\n");

                    let has_secure = cookie_context.to_lowercase().contains("secure");
                    let has_httponly = cookie_context.to_lowercase().contains("httponly");
                    let has_samesite = cookie_context.to_lowercase().contains("samesite");

                    if !has_secure || !has_httponly || !has_samesite {
                        issues.push(Issue::new(
                            RULE_ID,
                            "Cookie created without security flags. Ensure HttpOnly, Secure, and SameSite are set.",
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::moderate(
                            "Add security flags when setting cookie: Cookie::build().secure().http_only().same_site()"
                        )));
                    }
                    break;
                }
            }

            // Check header manipulation
            if HEADER_COOKIE_PATTERN.is_match(trimmed) && !trimmed.to_lowercase().contains("httponly") {
                issues.push(Issue::new(
                    RULE_ID,
                    "Cookie header set without security flags.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Add security flags to cookie header"
                )));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(S1004Rule::new())
    }
}

/// Agent semantics for S1004 - Insecure Cookie Flags
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S1004_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects cookies set without HttpOnly, Secure, or SameSite flags, making them vulnerable to XSS and CSRF attacks",
    fix_playbook: "1. Identify where cookies are being set\n2. Add HttpOnly flag to prevent JavaScript access\n3. Add Secure flag to ensure HTTPS-only transmission\n4. Add SameSite flag to prevent cross-site requests\n5. Use appropriate SameSite value: Strict (most secure) or Lax (balanced)\n6. For authentication cookies, SameSite=Lax or SameSite=Strict is recommended",
    review_questions: &[
        "Is this cookie for authentication or tracking?",
        "Does the cookie need to be accessible via JavaScript?",
        "Is the application HTTPS-only?",
        "What SameSite value is appropriate for this use case?"
    ],
    agent_actions: &[
        "Identify cookie setting locations",
        "Check for HttpOnly, Secure, and SameSite flags",
        "Verify cookie purpose and appropriate settings",
        "Suggest correct SameSite value"
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
    fn test_s1004_rule_properties() {
        let rule = S1004Rule::new();
        assert_eq!(rule.id(), "S1004");
        assert_eq!(rule.name(), "Insecure cookie flags detected");
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s1004_detects_set_cookie_header_without_flags() {
        let source = r#"
            Set-Cookie: session=abc123;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect Set-Cookie without flags");
        assert_eq!(issues[0].rule_id, "S1004");
    }

    #[test]
    fn test_s1004_detects_cookie_only_secure() {
        let source = r#"
            Set-Cookie: session=abc123; Secure;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect missing HttpOnly and SameSite");
    }

    #[test]
    fn test_s1004_detects_cookie_only_httponly() {
        let source = r#"
            Set-Cookie: session=abc123; HttpOnly;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect missing Secure and SameSite");
    }

    #[test]
    fn test_s1004_detects_document_cookie() {
        let source = r#"
            document.cookie = "session=abc123";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect document.cookie assignment");
    }

    #[test]
    fn test_s1004_detects_cookie_new() {
        let source = r#"
            let cookie = Cookie::new("session", "abc123");
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect Cookie::new without flags");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s1004_false_positive_comment() {
        let source = r#"
            // Set-Cookie: session=abc123; HttpOnly; Secure;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect cookie in comment");
    }

    #[test]
    fn test_s1004_false_positive_full_flags() {
        let source = r#"
            Set-Cookie: session=abc123; HttpOnly; Secure; SameSite=Strict;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect cookie with all flags");
    }

    #[test]
    fn test_s1004_false_positive_case_insensitive() {
        let source = r#"
            set-cookie: session=abc123; HTTPONLY; SECURE; SAMESITE=LAX;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect cookie with case-insensitive flags");
    }

    #[test]
    fn test_s1004_false_positive_doc_comment() {
        let source = r#"
            /// Sets a cookie with HttpOnly and Secure flags
            fn set_session() {}
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect cookie in doc comment");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s1004_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s1004_edge_case_single_line() {
        let source = "Set-Cookie: test=value";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect cookie on single line");
    }

    #[test]
    fn test_s1004_edge_case_multiple_cookies() {
        let source = r#"
            Set-Cookie: first=1;
            Set-Cookie: second=2; HttpOnly;
            Set-Cookie: third=3; HttpOnly; Secure;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S1004Rule::new();
            rule.check(ctx)
        });
        // Should detect first cookie (missing all flags)
        // May also detect second cookie (missing Secure and SameSite)
        assert!(!issues.is_empty(), "Should detect multiple cookies without all flags");
    }
}
