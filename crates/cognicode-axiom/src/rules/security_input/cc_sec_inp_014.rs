//! CC_SEC_INP_014: Unvalidated URL Scheme Allows Dangerous Protocols

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for unvalidated URL scheme detection
static UNVALIDATED_URL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Url::parse with user input
        Regex::new(r#"Url::parse\s*\(\s*&?\w+\s*\)"#).unwrap(),
        // reqwest::get with user URL
        Regex::new(r#"reqwest::get\s*\(\s*&?\w+\s*\)"#).unwrap(),
    ]
});

/// Safe URL patterns
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // scheme validation
        Regex::new(r#"scheme\s*\(\s*\)\s*=="#).unwrap(),
        // require_https
        Regex::new(r#"require_https\s*\("#).unwrap(),
    ]
});

/// CC_SEC_INP_014 Rule: Unvalidated URL Scheme
pub struct UnvalidatedUrlSchemeRule;

impl Default for UnvalidatedUrlSchemeRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for UnvalidatedUrlSchemeRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_014")
    }

    fn name(&self) -> &'static str {
        "Unvalidated URL Scheme Allows Dangerous Protocols"
    }

    fn description(&self) -> &'static str {
        "Detects URL construction where the scheme is not validated"
    }

    fn category(&self) -> Category {
        Category::Security
    }

    fn severity(&self) -> Severity {
        Severity::Minor
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

        // Line-by-line scanning for unvalidated URL schemes
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for URL-related keywords
            let has_url_kw = trimmed.contains("Url::parse")
                || trimmed.contains("reqwest::get")
                || trimmed.contains("Client::new");

            if !has_url_kw {
                continue;
            }

            // Check for unvalidated URL patterns
            for pattern in UNVALIDATED_URL_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Look ahead for scheme validation
                    let next_lines: String = source.lines()
                        .skip(line_num)
                        .take(5)
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Check if validation exists
                    if SAFE_PATTERNS.iter().any(|p| p.is_match(&next_lines)) {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_014",
                        "Unvalidated URL Scheme Allows Dangerous Protocols",
                        Severity::Minor,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible unvalidated URL scheme: URL parsed without checking scheme. \
                         Validate scheme is 'https' before use.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["Url", "parse", "reqwest", "Client", "url", "scheme"])
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
        let rule = UnvalidatedUrlSchemeRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_unvalidated_url() {
        let code = r#"let url = Url::parse(&user_url)?"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect unvalidated URL");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_014");
    }

    #[test]
    fn test_safe_with_scheme_validation() {
        let code = r#"let url = Url::parse(&user_url)?;
if url.scheme() == "https" { Ok(()) }"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag with scheme validation");
    }

    #[test]
    fn test_safe_require_https() {
        let code = r#"let url = Url::parse(&user_url)?;
url.require_https()?"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag with require_https");
    }
}