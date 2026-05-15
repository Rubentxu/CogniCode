//! CC_SEC_INP_011: HTTP Response Splitting via Header Injection

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for HTTP response splitting detection
static HTTP_SPLITTING_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // insert_header with format!
        Regex::new(r#"insert_header\s*\([^)]*format!"#).unwrap(),
        // append_header with format!
        Regex::new(r#"append_header\s*\([^)]*format!"#).unwrap(),
    ]
});

/// Safe header patterns
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Newlines stripped
        Regex::new(r#"replace\s*\(\s*['\"][\r\n]"#).unwrap(),
        // Typed header builders
        Regex::new(r#"ContentType::"#).unwrap(),
    ]
});

/// CC_SEC_INP_011 Rule: HTTP Response Splitting
pub struct HttpResponseSplittingRule;

impl Default for HttpResponseSplittingRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for HttpResponseSplittingRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_011")
    }

    fn name(&self) -> &'static str {
        "HTTP Response Splitting via Header Injection"
    }

    fn description(&self) -> &'static str {
        "Detects HTTP header construction where user input could inject CRLF characters"
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

        // Line-by-line scanning for HTTP response splitting patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for header-related keywords
            let has_header_kw = trimmed.contains("insert_header")
                || trimmed.contains("append_header");

            if !has_header_kw {
                continue;
            }

            // Check for splitting patterns
            for pattern in HTTP_SPLITTING_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check if it's actually safe
                    if SAFE_PATTERNS.iter().any(|p| p.is_match(line)) {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_011",
                        "HTTP Response Splitting via Header Injection",
                        Severity::Major,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible HTTP response splitting: header value may contain CRLF characters. \
                         Strip newlines before inserting headers.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["insert_header", "append_header", "header"])
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
        let rule = HttpResponseSplittingRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_header_injection() {
        let code = r#"insert_header(("X-Greeting", format!("Hello {}", name)))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect header injection");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_011");
    }

    #[test]
    fn test_safe_newline_strip() {
        let code = r#"insert_header(("X-Value", name.replace('\n', "")))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag with newline stripping");
    }

    #[test]
    fn test_safe_typed_header() {
        let code = r#"insert_header(ContentType::json())"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag typed headers");
    }
}