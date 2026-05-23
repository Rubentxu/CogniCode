//! S5696 — Open Redirect Detection
//! Detects URL redirection based on user input without validation (CWE-601).
//!
//! Languages: *
//! Severity: Major
//! Category: Vulnerability
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S5696
const RULE_ID: &str = "S5696";
const RULE_NAME: &str = "Open redirect vulnerability detected";
const SEVERITY: Severity = Severity::Major;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Pattern for redirect calls with user-controlled URL
static REDIRECT_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Rust actix-web redirects
        regex::Regex::new(r#"(?i)HttpResponse::(?:found|see_other|temporary_redirect|permanent_redirect)"#).unwrap(),
        regex::Regex::new(r#"(?i)\.insert_header\s*\(\s*["']Location["']\s*,"#).unwrap(),
        regex::Regex::new(r#"(?i)Redirect::(?:to|found|permanent|to_any)"#).unwrap(),
        // Rust rocket redirects
        regex::Regex::new(r#"(?i)Response::redirect"#).unwrap(),
        regex::Regex::new(r#"(?i)redirect\s*\("#).unwrap(),
        // Rust warp redirects
        regex::Regex::new(r#"(?i)warp::redirect"#).unwrap(),
        // axum redirects
        regex::Regex::new(r#"(?i)axum::(?:redirect|Response::redirect)"#).unwrap(),
        // JavaScript/TypeScript redirects
        regex::Regex::new(r#"(?i)window\.location\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)window\.location\.href\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)document\.location\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)location\.href\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)location\.replace\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)history\.pushState\s*\([^)]*\)"#).unwrap(),
        regex::Regex::new(r#"(?i)history\.replaceState\s*\([^)]*\)"#).unwrap(),
        // Python web frameworks
        regex::Regex::new(r#"(?i)redirect\s*\(\s*(?:request\.|flask\.|ctx\.)"#).unwrap(),
        regex::Regex::new(r#"(?i)HttpResponseRedirect"#).unwrap(),
        regex::Regex::new(r#"(?i)redirect_to\s*\("#).unwrap(),
        // Java
        regex::Regex::new(r#"(?i)response\.sendRedirect\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)response\.setStatus\s*\(\s*302\s*\)"#).unwrap(),
        regex::Regex::new(r#"(?i)Response::(?:found|temporaryRedirect|permanentRedirect)"#).unwrap(),
    ]
});

/// Pattern for Location header
static LOCATION_HEADER_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)["']Location["']|Location\s*:"#).unwrap()
});

/// Pattern for user input sources that might be used in redirects
static USER_INPUT_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Rust
        regex::Regex::new(r#"(?i)(?:request\.|query|params|form)\s*\["#).unwrap(),
        regex::Regex::new(r#"(?i)(?:request\.uri\(\)|request\.query\(\))"#).unwrap(),
        regex::Regex::new(r#"(?i)header\s*\(\s*["']Referer["']\s*\)"#).unwrap(),
        regex::Regex::new(r#"(?i)header\s*\(\s*["']Origin["']\s*\)"#).unwrap(),
        // JavaScript
        regex::Regex::new(r#"(?i)(?:window\.)?location\.(?:search|hash|pathname)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:URL|document|window)\.searchParams"#).unwrap(),
        regex::Regex::new(r#"(?i)new\s+URL\s*\([^)]*\)\.search"#).unwrap(),
        regex::Regex::new(r#"(?i)atob\s*\([^)]*\)"#).unwrap(),  // base64 decoded user input
    ]
});

/// Pattern for dangerous URL schemes
static DANGEROUS_URL_SCHEME_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)^(?:javascript:|data:|blob:|vbscript:)"#).unwrap()
});

/// Pattern for external domain patterns
/// Uses fancy_regex for lookahead support
static EXTERNAL_DOMAIN_PATTERN: LazyLock<fancy_regex::Regex> = LazyLock::new(|| {
    fancy_regex::Regex::new(r#"(?i)^https?://(?!localhost|127\.0\.0\.1|\[::1\])(?!/)(?![\w-]+/)"#).unwrap()
});

declare_rule! {
    id: "S5696"
    name: "Open redirect vulnerability detected"
    severity: Major
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "URL redirection based on user input without validation allows attackers to redirect users to malicious sites (phishing, malware distribution). Always validate and whitelist redirect targets."
    clean_code: Clear,
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

            // Check for redirect patterns
            let has_redirect = REDIRECT_PATTERNS.iter().any(|p| p.is_match(trimmed));
            let has_location_header = LOCATION_HEADER_PATTERN.is_match(trimmed);

            if has_redirect || has_location_header {
                // Check if user input is being used in the redirect
                let has_user_input = USER_INPUT_PATTERNS.iter().any(|p| p.is_match(trimmed));

                // Check for dangerous URL schemes
                let has_dangerous_scheme = DANGEROUS_URL_SCHEME_PATTERN.is_match(trimmed);
                let has_external_domain = EXTERNAL_DOMAIN_PATTERN.is_match(trimmed);

                // Look at surrounding context for user input
                let context: String = (0..5)
                    .filter_map(|i| ctx.source.lines().nth(line_idx.saturating_sub(2) + i))
                    .take(6)
                    .collect::<Vec<_>>()
                    .join("\n");

                let context_has_user_input = USER_INPUT_PATTERNS.iter().any(|p| p.is_match(&context));

                if has_dangerous_scheme {
                    issues.push(Issue::new(
                        RULE_ID,
                        "Dangerous URL scheme (javascript:, data:, etc.) detected in redirect. This is a common XSS and open redirect attack vector.",
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Never allow user input to contain URL schemes. Use a whitelist of allowed redirect targets."
                    )));
                } else if has_external_domain {
                    issues.push(Issue::new(
                        RULE_ID,
                        "External domain detected in redirect. This could be an open redirect attack.",
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Validate redirect targets against a whitelist of allowed domains. Never allow arbitrary external URLs."
                    )));
                } else if has_user_input || context_has_user_input {
                    issues.push(Issue::new(
                        RULE_ID,
                        "Potential open redirect: user-controlled input used in redirect without validation.",
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Validate redirect targets against a whitelist. Check that the redirect URL is on an allowed list of domains/paths."
                    )));
                }
            }

            // Check for direct header setting with user input
            if trimmed.to_lowercase().contains("header") && trimmed.to_lowercase().contains("location") {
                if USER_INPUT_PATTERNS.iter().any(|p| p.is_match(trimmed)) {
                    issues.push(Issue::new(
                        RULE_ID,
                        "User-controlled header value used in Location header.",
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Validate Location header values against a whitelist."
                    )));
                }
            }
        }

        issues
    }
}


/// Agent semantics for S5696 - Open Redirect
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S5696_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects open redirect vulnerabilities where user input controls URL redirection target without proper validation, enabling phishing and malware attacks",
    fix_playbook: "1. Identify all redirect operations (HttpResponse::found(), redirect(), Location header)\n2. Trace where the redirect URL comes from (request params, headers, query strings)\n3. Check if user input is validated against a whitelist\n4. If not validated:\n   - Create a whitelist of allowed redirect domains/paths\n   - Validate redirect target against whitelist\n   - Reject or safe-default on invalid targets\n5. Block dangerous URL schemes (javascript:, data:, vbscript:)\n6. Consider using a safe URL type that enforces validation",
    review_questions: &[
        "Where does the redirect URL originate (query, header, form)?",
        "Is there a whitelist of allowed redirect targets?",
        "What is the risk if redirect is manipulated (phishing, malware)?",
        "Is Referer/Origin header validation sufficient for this case?",
    ],
    agent_actions: &[
        "Identify redirect operations in code",
        "Trace source of redirect URLs",
        "Check for whitelist validation",
        "Detect dangerous URL schemes",
        "Verify domain allowlist exists and is used",
        "Suggest safe redirect patterns",
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
    fn test_s5696_rule_properties() {
        let rule = S5696Rule::new();
        assert_eq!(rule.id(), "S5696");
        assert_eq!(rule.name(), "Open redirect vulnerability detected");
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5696_detects_actix_redirect_with_user_url() {
        let source = r#"
            HttpResponse::found().insert_header(("Location", &user_url))
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect actix redirect with user URL");
        assert_eq!(issues[0].rule_id, "S5696");
    }

    #[test]
    fn test_s5696_detects_redirect_from_request_uri() {
        let source = r#"
            redirect(&request.uri())
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect redirect from request.uri()");
    }

    #[test]
    fn test_s5696_detects_query_param_redirect() {
        let source = r#"
            redirect(&request.query()["redirect"])
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect redirect from query parameter");
    }

    #[test]
    fn test_s5696_detects_javascript_scheme() {
        let source = r#"
            window.location.href = "javascript:alert('xss')";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect javascript: scheme");
    }

    #[test]
    fn test_s5696_detects_external_domain_redirect() {
        let source = r#"
            HttpResponse::found().insert_header(("Location", user_url))
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        // With external domain pattern, this should be flagged
        assert!(!issues.is_empty(), "Expected to detect potential external redirect");
    }

    #[test]
    fn test_s5696_detects_warp_redirect() {
        let source = r#"
            warp::redirect()
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect warp redirect");
    }

    #[test]
    fn test_s5696_detects_data_scheme() {
        let source = r#"
            window.location = "data:text/html,<script>alert(1)</script>";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect data: scheme");
    }

    #[test]
    fn test_s5696_detects_location_header_manipulation() {
        let source = r#"
            header("Location", user_input)
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect Location header with user input");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5696_false_positive_static_redirect() {
        let source = r#"
            HttpResponse::found().insert_header(("Location", "/static/page"))
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static redirect path");
    }

    #[test]
    fn test_s5696_false_positive_whitelisted_redirect() {
        let source = r#"
            if is_allowed_redirect(&target) {
                HttpResponse::found().insert_header(("Location", &target))
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        // With explicit whitelist check, should not trigger
        assert!(issues.is_empty(), "Should NOT detect whitelisted redirect");
    }

    #[test]
    fn test_s5696_false_positive_comment() {
        let source = r#"
            // redirect(&request.uri()) - this is safe
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect redirect in comment");
    }

    #[test]
    fn test_s5696_false_positive_constant_path() {
        let source = r#"
            const SAFE_REDIRECT: &str = "/dashboard";
            redirect(SAFE_REDIRECT);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect constant path redirect");
    }

    #[test]
    fn test_s5696_false_positive_localhost_redirect() {
        let source = r#"
            HttpResponse::found().insert_header(("Location", "http://localhost/dashboard"))
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect localhost redirect");
    }

    #[test]
    fn test_s5696_false_positive_no_redirect() {
        let source = r#"
            fn home() -> HttpResponse {
                HttpResponse::Ok().body("Welcome")
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect regular HTTP response");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5696_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s5696_edge_case_multiple_redirects() {
        let source = r#"
            // Dashboard redirect
            HttpResponse::found().insert_header(("Location", "/dashboard"));
            // External redirect
            HttpResponse::found().insert_header(("Location", user_url));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect at least one redirect issue");
    }

    #[test]
    fn test_s5696_edge_case_mixed_case_schemes() {
        let source = r#"
            window.location.href = "JAVASCRIPT:alert(1)";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect case-insensitive scheme match");
    }

    #[test]
    fn test_s5696_edge_case_vbscript_scheme() {
        let source = r#"
            window.location.href = "vbscript:msgbox('xss')";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5696Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect vbscript: scheme");
    }
}