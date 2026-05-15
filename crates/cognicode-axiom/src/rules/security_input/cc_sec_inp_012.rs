//! CC_SEC_INP_012: Integer Overflow in Size Calculation

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for integer overflow in size calculation
static OVERFLOW_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // vec![...] with multiplication
        Regex::new(r#"vec!\s*\[\s*\w+\s*;\s*\w+\s*\*\s*\w+\]"#).unwrap(),
        // Vec::with_capacity with user input arithmetic
        Regex::new(r#"Vec::with_capacity\s*\(\s*\w+\s*[\+\*]\s*\w+\s*\)"#).unwrap(),
        // Size calculation with user input
        Regex::new(r#"(?:len|size|capacity)\s*\(\s*\)\s*\*\s*\w+"#).unwrap(),
    ]
});

/// Safe patterns
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // checked_add
        Regex::new(r#"checked_add\s*\("#).unwrap(),
        // checked_mul
        Regex::new(r#"checked_mul\s*\("#).unwrap(),
        // saturating_add
        Regex::new(r#"saturating_add\s*\("#).unwrap(),
        // saturating_mul
        Regex::new(r#"saturating_mul\s*\("#).unwrap(),
    ]
});

/// CC_SEC_INP_012 Rule: Integer Overflow in Size Calculation
pub struct IntegerOverflowRule;

impl Default for IntegerOverflowRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for IntegerOverflowRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_012")
    }

    fn name(&self) -> &'static str {
        "Integer Overflow in Size Calculation"
    }

    fn description(&self) -> &'static str {
        "Detects arithmetic on size/length values where user input could cause overflow"
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

        // Line-by-line scanning for overflow patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for size-related keywords
            let has_size_kw = trimmed.contains(".len()")
                || trimmed.contains("size()")
                || trimmed.contains("capacity()")
                || trimmed.contains("vec![")
                || trimmed.contains("Vec::with_capacity");

            if !has_size_kw {
                continue;
            }

            // Check for overflow patterns
            for pattern in OVERFLOW_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check if it's actually safe
                    if SAFE_PATTERNS.iter().any(|p| p.is_match(line)) {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_012",
                        "Integer Overflow in Size Calculation",
                        Severity::Major,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible integer overflow: size calculation with user input without overflow checking. \
                         Use checked_add/checked_mul or validate input range.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["len", "size", "capacity", "Vec", "vec", "with_capacity"])
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
        let rule = IntegerOverflowRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_vec_overflow() {
        let code = r#"vec![0u8; a.len() * b.len()]"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect integer overflow");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_012");
    }

    #[test]
    fn test_detects_unsafe_capacity() {
        let code = r#"Vec::with_capacity(size * 2)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect unsafe capacity calculation");
    }

    #[test]
    fn test_safe_checked_arithmetic() {
        let code = r#"size.checked_mul(2)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag checked arithmetic");
    }

    #[test]
    fn test_safe_saturating() {
        let code = r#"size.saturating_mul(2)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag saturating arithmetic");
    }
}