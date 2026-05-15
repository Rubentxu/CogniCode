//! CC_SEC_INP_004: XML External Entity (XXE) Injection

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for XXE vulnerability detection
static XXE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // quick_xml Reader::new without features
        Regex::new(r#"Reader::new\s*\(\s*\w+\s*\)"#).unwrap(),
        Regex::new(r#"Reader::from_str\s*\(\s*\w+\s*\)"#).unwrap(),
        // xml parser creation
        Regex::new(r#"XmlReader::new\s*\("#).unwrap(),
    ]
});

/// Safe XXE patterns (feature disabled)
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // with_feature disabling external entities
        Regex::new(r#"with_feature\s*\("#).unwrap(),
        // Using serde_json instead
        Regex::new(r#"serde_json::from"#).unwrap(),
    ]
});

/// CC_SEC_INP_004 Rule: XML External Entity (XXE) Injection
pub struct XxeInjectionRule;

impl Default for XxeInjectionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for XxeInjectionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_004")
    }

    fn name(&self) -> &'static str {
        "XML External Entity (XXE) Injection"
    }

    fn description(&self) -> &'static str {
        "Detects XML parser configuration that enables external entity expansion"
    }

    fn category(&self) -> Category {
        Category::Security
    }

    fn severity(&self) -> Severity {
        Severity::Critical
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

        // Line-by-line scanning for XXE patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for XML parser creation
            let has_xml_parser = trimmed.contains("Reader::new")
                || trimmed.contains("Reader::from_str")
                || trimmed.contains("XmlReader::new");

            if !has_xml_parser {
                continue;
            }

            // Check for XXE patterns
            for pattern in XXE_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check if features are disabled
                    if line.contains("with_feature(") {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_004",
                        "XML External Entity (XXE) Injection",
                        Severity::Critical,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible XXE vulnerability: XML parser created without disabling external entities. \
                         Use with_feature() to disable external entities or use serde_json for untrusted data.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["XmlReader", "Reader", "quick_xml", "DTD", "Entity", "external", "feature"])
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
        let rule = XxeInjectionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_xxe_quick_xml() {
        let code = r#"let mut reader = Reader::new(input);"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect XXE vulnerability in quick_xml");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_004");
    }

    #[test]
    fn test_detects_xxe_from_str() {
        let code = r#"let reader = Reader::from_str(data);"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect XXE via from_str");
    }

    #[test]
    fn test_safe_with_feature_disable() {
        let code = r#"reader.with_feature("external", false)?"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag when features are disabled");
    }

    #[test]
    fn test_safe_using_json() {
        let code = r#"serde_json::from_str(data)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag JSON parsing");
    }
}