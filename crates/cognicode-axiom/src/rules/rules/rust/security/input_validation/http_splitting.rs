//! S2004 — HTTP Response Splitting (CRLF Injection) Detection
//! Detects user input in HTTP headers without proper sanitization (CWE-93).
//!
//! Languages: Rust
//! Severity: Major
//! Category: Vulnerability
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S2004
const RULE_ID: &str = "S2004";
const RULE_NAME: &str = "HTTP Response Splitting (CRLF injection) detected";
const SEVERITY: Severity = Severity::Major;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Pattern for detecting header insertion with potential user input
static HEADER_INSERT_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)(?:insert_header|set_header|append_header|header)\s*\("#).unwrap()
});

/// Pattern for HttpResponse in actix-web and similar frameworks
static HTTP_RESPONSE_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)HttpResponse[^{]*\{[^}]*\.insert_header\s*\("#).unwrap()
});

/// Pattern for response headers in various frameworks
static RESPONSE_HEADER_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)(?:response|Response)[^;]*(?:set_header|insert_header|add_header)"#).unwrap()
});

/// Pattern for dangerous newline characters in user-controlled input context
static NEWLINE_IN_HEADER_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"\\[rn]|%0[dD]|%0[aA]|#0[dD]|#0[aA]"#).unwrap()
});

/// Pattern to detect user-controlled variables in header values
static USER_INPUT_HEADER_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)(?:user|url|query|param|input|cookie|request|body)[^;]*insert_header\s*\("#).unwrap()
});

declare_rule! {
    id: "S2004"
    name: "HTTP Response Splitting (CRLF injection) detected"
    severity: Major
    category: Vulnerability
    language: "Rust"
    params: {}

    explanation: "HTTP Response Splitting occurs when user input is included in HTTP headers without proper sanitization. Attackers can inject CRLF characters (%0D%0A) to manipulate HTTP responses, leading to header injection, response splitting, and cache poisoning attacks."
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

            // Check for header insertion patterns
            if HEADER_INSERT_PATTERN.is_match(trimmed) {
                // Check if there's user input context (variables, function params)
                let has_user_input = USER_INPUT_HEADER_PATTERN.is_match(trimmed);
                let has_newlines = NEWLINE_IN_HEADER_PATTERN.is_match(trimmed);

                // Look ahead to get more context about the header value
                let header_context: String = (0..3)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n");

                // Check for potential CRLF injection
                if has_user_input || has_newlines {
                    // Check if the value contains or could contain newlines
                    let value_part = header_context.lines()
                        .skip(line_idx.saturating_sub(0))
                        .take(2)
                        .collect::<Vec<_>>()
                        .join("\n");

                    if has_newlines || value_part.contains("\\r") || value_part.contains("\\n")
                       || value_part.contains("%0d") || value_part.contains("%0a")
                       || value_part.to_lowercase().contains("user")
                       || value_part.to_lowercase().contains("input")
                       || value_part.to_lowercase().contains("param")
                       || value_part.to_lowercase().contains("query")
                       || value_part.to_lowercase().contains("url")
                       || value_part.to_lowercase().contains("cookie")
                       || value_part.to_lowercase().contains("request")
                       || value_part.to_lowercase().contains("body") {

                        // Avoid false positives: check for sanitization
                        let has_sanitize = value_part.to_lowercase().contains("sanitize")
                            || value_part.to_lowercase().contains("escape")
                            || value_part.to_lowercase().contains("replace")
                            || value_part.to_lowercase().contains("strip")
                            || value_part.to_lowercase().contains("validate");

                        if !has_sanitize {
                            issues.push(Issue::new(
                                RULE_ID,
                                "Potential HTTP Response Splitting: user input may be included in HTTP headers without CRLF sanitization.".to_string(),
                                SEVERITY,
                                CATEGORY,
                                ctx.file_path,
                                line_idx + 1,
                            ).with_remediation(Remediation::substantial(
                                "Sanitize user input before including in headers. Remove or encode CRLF characters (%0D%0A). Use libraries that handle header encoding safely."
                            )));
                        }
                    }
                }
            }

            // Check for HttpResponse with header insertion
            if HTTP_RESPONSE_PATTERN.is_match(trimmed) {
                // Look for potentially dangerous patterns
                let response_context: String = (0..5)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                    .take(6)
                    .collect::<Vec<_>>()
                    .join("\n");

                // Check for user-controlled values in insert_header
                if response_context.to_lowercase().contains("user")
                    || response_context.to_lowercase().contains("input")
                    || response_context.to_lowercase().contains("param")
                    || response_context.to_lowercase().contains("query")
                    || response_context.to_lowercase().contains("url")
                    || response_context.to_lowercase().contains("cookie")
                    || response_context.to_lowercase().contains("request")
                    || response_context.to_lowercase().contains("body") {

                    // Check for newlines or their encoded forms
                    if response_context.contains("\\r") || response_context.contains("\\n")
                        || response_context.to_lowercase().contains("%0d")
                        || response_context.to_lowercase().contains("%0a")
                        || response_context.to_lowercase().contains("\\u{0d}")
                        || response_context.to_lowercase().contains("\\u{0a}") {

                        issues.push(Issue::new(
                            RULE_ID,
                            "HTTP Response Splitting: detected user-controlled value with potential CRLF characters in header.".to_string(),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Validate and sanitize all user input before adding to HTTP headers. Remove CRLF sequences or use proper encoding."
                        )));
                    }
                }
            }

            // Check for response header manipulation
            if RESPONSE_HEADER_PATTERN.is_match(trimmed) && !trimmed.contains("//") {
                let context: String = (0..3)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n");

                // Check for user input patterns
                if context.to_lowercase().contains("user")
                    || context.to_lowercase().contains("input")
                    || context.to_lowercase().contains("param")
                    || context.to_lowercase().contains("query")
                    || context.to_lowercase().contains("url")
                    || context.to_lowercase().contains("cookie")
                    || context.to_lowercase().contains("request")
                    || context.to_lowercase().contains("body") {

                    // Check for potential sanitization
                    let has_sanitize = context.to_lowercase().contains("sanitize")
                        || context.to_lowercase().contains("escape")
                        || context.to_lowercase().contains("replace")
                        || context.to_lowercase().contains("strip")
                        || context.to_lowercase().contains("validate");

                    if !has_sanitize {
                        issues.push(Issue::new(
                            RULE_ID,
                            "HTTP Response Splitting: user input in response header without visible sanitization.".to_string(),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Sanitize user input before using in response headers"
                        )));
                    }
                }
            }
        }

        issues
    }
}


/// Agent semantics for S2004 - HTTP Response Splitting
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S2004_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects HTTP Response Splitting (CRLF injection) where user input in HTTP headers lacks proper sanitization, allowing attackers to inject newline characters to manipulate HTTP responses",
    fix_playbook: "1. Identify all locations where user input is added to HTTP headers\n2. Remove or encode CRLF characters (%0D%0A or \\r\\n) from user input before header insertion\n3. Use parameterized header APIs when available\n4. Implement input validation to reject requests with suspicious characters\n5. Consider using security-focused HTTP libraries that handle encoding automatically\n6. For actix-web: use HttpResponse::Ok().insert_header((key, value)) with validated values only",
    review_questions: &[
        "Is the header value derived from user input?",
        "Are CRLF characters (%0D%0A, \\r\\n) being filtered or encoded?",
        "Is there input validation that rejects newlines in header values?",
        "Are you using a framework that handles header encoding safely?"
    ],
    agent_actions: &[
        "Identify header insertion with user-controlled values",
        "Check for presence of CRLF sanitization",
        "Verify input validation for newline characters",
        "Suggest proper encoding or use of safe header APIs",
        "Recommend rejecting requests with suspicious header values"
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
    fn test_s2004_rule_properties() {
        let rule = HTTP_RESPONSE_SPLITTINGRule::new();
        assert_eq!(rule.id(), "S2004");
        assert_eq!(rule.name(), "HTTP Response Splitting (CRLF injection) detected");
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2004_detects_user_input_in_header() {
        let source = r#"
            let user_input = get_user_value();
            response.insert_header(("X-Custom", user_input));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect user input in header insertion");
        assert_eq!(issues[0].rule_id, "S2004");
    }

    #[test]
    fn test_s2004_detects_url_in_header() {
        let source = r#"
            let url_value = request.query("redirect_url");
            response.insert_header(("Location", url_value));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect URL query parameter in header");
        assert_eq!(issues[0].rule_id, "S2004");
    }

    #[test]
    fn test_s2004_detects_cookie_in_header() {
        let source = r#"
            let cookie_val = cookie.get("user_pref");
            response.set_header("X-User-Preference", cookie_val);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect cookie value in header");
        assert_eq!(issues[0].rule_id, "S2004");
    }

    #[test]
    fn test_s2004_detects_newline_in_header_value() {
        let source = r#"
            let malicious = "value\\r\\nX-Injected: evil";
            response.insert_header(("X-Custom", malicious));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect CRLF characters in header value");
        assert_eq!(issues[0].rule_id, "S2004");
    }

    #[test]
    fn test_s2004_detects_encoded_crlf() {
        let source = r#"
            let encoded = "%0D%0AX-Injected: header";
            response.insert_header(("X-Custom", encoded));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect URL-encoded CRLF in header value");
        assert_eq!(issues[0].rule_id, "S2004");
    }

    #[test]
    fn test_s2004_detects_http_response_header() {
        let source = r#"
            HttpResponse::Ok()
                .insert_header(("X-User-Header", user_input))
                .finish()
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect user input in HttpResponse header");
        assert_eq!(issues[0].rule_id, "S2004");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2004_false_positive_static_header() {
        let source = r#"
            response.insert_header(("X-Custom", "static-value"));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static header value");
    }

    #[test]
    fn test_s2004_false_positive_sanitized_input() {
        let source = r#"
            let sanitized = sanitize_input(user_input);
            response.insert_header(("X-Custom", sanitized));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect sanitized input");
    }

    #[test]
    fn test_s2004_false_positive_escaped_input() {
        let source = r#"
            let escaped = user_input.replace("\\r", "").replace("\\n", "");
            response.insert_header(("X-Custom", escaped));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect escaped newlines");
    }

    #[test]
    fn test_s2004_false_positive_validated_input() {
        let source = r#"
            let validated = validate_header_value(user_input)?;
            response.insert_header(("X-Custom", validated));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect validated input");
    }

    #[test]
    fn test_s2004_false_positive_comment() {
        let source = r#"
            // response.insert_header(("X-Custom", user_input));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect header insertion in comment");
    }

    #[test]
    fn test_s2004_false_positive_stripped_newlines() {
        let source = r#"
            let stripped = user_input.trim().strip_prefix(|c| c == '\\r' || c == '\\n');
            response.insert_header(("X-Custom", stripped));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect stripped newlines");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2004_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s2004_edge_case_multiple_headers() {
        let source = r#"
            let safe = "static-value";
            let user_val = get_user_input();
            response.insert_header(("X-Safe", safe));
            response.insert_header(("X-User", user_val));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        // Should detect only the user input header, not the static one
        assert!(!issues.is_empty(), "Should detect user input header");
    }

    #[test]
    fn test_s2004_edge_case_body_in_header() {
        let source = r#"
            let body = request.body();
            response.insert_header(("X-Body-Hash", body));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = HTTP_RESPONSE_SPLITTINGRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect body in header");
    }
}
