//! CC_SEC_INP_010: Open Redirect via Unvalidated URL

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for open redirect detection
static OPEN_REDIRECT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // redirect with format!
        Regex::new(r#"redirect\s*\([^)]*format!"#).unwrap(),
        // Location header with format!
        Regex::new(r#"insert_header\s*\(\s*\(?"["']?Location["']?"#).unwrap(),
    ]
});

/// Safe redirect patterns
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Whitelist validation
        Regex::new(r#"whitelist|allowlist|is_allowed_redirect"#).unwrap(),
        // Named route
        Regex::new(r#"Redirect::to_named"#).unwrap(),
        // Constant URL
        Regex::new(r#"redirect\s*\(\s*"[^"]+"\s*\)"#).unwrap(),
    ]
});

/// CC_SEC_INP_010 Rule: Open Redirect
pub struct OpenRedirectRule;

impl Default for OpenRedirectRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for OpenRedirectRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_010")
    }

    fn name(&self) -> &'static str {
        "Open Redirect via Unvalidated URL"
    }

    fn description(&self) -> &'static str {
        "Detects HTTP redirects where destination URL is constructed from user input without validation"
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

        // Line-by-line scanning for open redirect patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for redirect-related keywords
            let has_redirect_kw = trimmed.contains("redirect")
                || trimmed.contains("Location")
                || trimmed.contains("Moved")
                || trimmed.contains("HttpResponse");

            if !has_redirect_kw {
                continue;
            }

            // Check for open redirect patterns
            for pattern in OPEN_REDIRECT_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check if it's actually safe
                    if SAFE_PATTERNS.iter().any(|p| p.is_match(line)) {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_010",
                        "Open Redirect via Unvalidated URL",
                        Severity::Major,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible open redirect: URL constructed from user input without validation. \
                         Use whitelist validation or named route redirects.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["redirect", "Location", "HttpResponse", "Moved"])
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
        let rule = OpenRedirectRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_open_redirect() {
        let code = r#"redirect(&format!("{}/dashboard", url))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect open redirect");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_010");
    }

    #[test]
    fn test_detects_location_header_injection() {
        let code = r#"insert_header(("Location", format!("https://{}/path", host)))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect Location header injection");
    }

    #[test]
    fn test_safe_named_route() {
        let code = r#"Redirect::to_named("dashboard")"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag named route redirects");
    }

    #[test]
    fn test_safe_whitelist_validation() {
        let code = r#"if is_allowed_redirect(&url) { redirect(&url) }"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag with whitelist validation");
    }
}