//! CC_SEC_INP_007: XPath Injection via Expression Concatenation

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for XPath injection detection
static XPATH_INJECTION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // format! in XPath evaluate
        Regex::new(r#"xpath\.evaluate\s*\(\s*format!\s*\("#).unwrap(),
        // .select with format!
        Regex::new(r#"\.(?:select|query)\s*\(\s*format!\s*\("#).unwrap(),
    ]
});

/// Safe XPath patterns
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Prepared statements with bind
        Regex::new(r#"prepare.*bind"#).unwrap(),
    ]
});

/// CC_SEC_INP_007 Rule: XPath Injection
pub struct XpathInjectionRule;

impl Default for XpathInjectionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for XpathInjectionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_007")
    }

    fn name(&self) -> &'static str {
        "XPath Injection via Expression Concatenation"
    }

    fn description(&self) -> &'static str {
        "Detects XPath expression construction where user input is concatenated without parameterization"
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

        // Line-by-line scanning for XPath injection patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for XPath-related keywords
            let has_xpath_kw = trimmed.contains("xpath") || trimmed.contains("evaluate");

            if !has_xpath_kw {
                continue;
            }

            // Check for injection patterns
            for pattern in XPATH_INJECTION_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check if it's actually safe
                    if SAFE_PATTERNS.iter().any(|p| p.is_match(line)) {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_007",
                        "XPath Injection via Expression Concatenation",
                        Severity::Major,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible XPath injection: user input concatenated into XPath expression. \
                         Use prepared statements with bind() or validate/escape input.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["xpath", "evaluate", "select", "query"])
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
        let rule = XpathInjectionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_xpath_injection() {
        let code = r#"xpath.evaluate(format!("//user[@name='{}']", name), doc)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect XPath injection");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_007");
    }

    #[test]
    fn test_safe_prepared_statement() {
        let code = r#"xpath.prepare("//user[@name=$1]")?.bind(&[name])?"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag prepared statements");
    }
}