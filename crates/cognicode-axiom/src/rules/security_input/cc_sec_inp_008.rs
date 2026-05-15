//! CC_SEC_INP_008: Cross-Site Scripting (XSS) via Unsanitized Output

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for XSS detection
static XSS_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // format! with HTML and user input
        Regex::new(r#"format!\s*\(\s*"[^"]*<[^"]*\{\}"#).unwrap(),
        // HttpResponse with format! body
        Regex::new(r#"HttpResponse::.*body\s*\(\s*format!\s*\("#).unwrap(),
    ]
});

/// Safe XSS patterns
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // escape_html
        Regex::new(r#"escape_html\s*\("#).unwrap(),
        // Template engine auto-escape
        Regex::new(r#"(?:askama|maud|handlebars)::"#).unwrap(),
        // JSON response
        Regex::new(r#"\.json\s*\("#).unwrap(),
    ]
});

/// CC_SEC_INP_008 Rule: Cross-Site Scripting
pub struct CrossSiteScriptingRule;

impl Default for CrossSiteScriptingRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for CrossSiteScriptingRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_008")
    }

    fn name(&self) -> &'static str {
        "Cross-Site Scripting (XSS) via Unsanitized Output"
    }

    fn description(&self) -> &'static str {
        "Detects HTML/JS output where user input is rendered without sanitization"
    }

    fn category(&self) -> Category {
        Category::Security
    }

    fn severity(&self) -> Severity {
        Severity::Major
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Rust]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Skip test files
        let path_str = ctx.file_path.to_string_lossy();
        if path_str.contains("_test.") || path_str.contains("test_") || path_str.contains("/tests/") {
            return issues;
        }

        // Line-by-line scanning for XSS patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for HTML/response-related keywords
            let has_html_kw = trimmed.contains("HttpResponse")
                || trimmed.contains("format!");

            if !has_html_kw {
                continue;
            }

            // Check for XSS patterns
            for pattern in XSS_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check if it's actually safe
                    if SAFE_PATTERNS.iter().any(|p| p.is_match(line)) {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_008",
                        "Cross-Site Scripting (XSS) via Unsanitized Output",
                        Severity::Major,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible XSS vulnerability: user input rendered in HTML without escaping. \
                         Use escape_html(), template engines with auto-escape, or JSON responses.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["HttpResponse", "body", "format", "render"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_rule(code: &str, language: SrcLanguage) -> Vec<Issue> {
        let lang = language.to_ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(code, None).unwrap();
        let source = code.to_string();
        let metrics = crate::types::FileMetrics::default();
        let ctx = RuleContext::new(
            &tree,
            &source,
            std::path::Path::new("test.rs"),
            &language,
            &metrics,
        );
        let rule = CrossSiteScriptingRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_xss_http_response() {
        let code = r#"HttpResponse::Ok().body(format!("<div>{}</div>", name))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect XSS in HttpResponse");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_008");
    }

    #[test]
    fn test_detects_xss_script_tag() {
        let code = r#"format!("<script>alert('{}')</script>", msg)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect XSS via script tag");
    }

    #[test]
    fn test_safe_with_escape() {
        let code = r#"format!("<div>{}</div>", escape_html(&name))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag with escape_html");
    }

    #[test]
    fn test_safe_json_response() {
        let code = r#"HttpResponse::Ok().json(&user)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag JSON responses");
    }
}