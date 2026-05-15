//! CC_SEC_INP_013: Missing Input Sanitization on Public Interface

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for missing sanitization detection
static MISSING_SANITIZATION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // pub fn with String parameter
        Regex::new(r#"pub\s+fn\s+\w+\s*\([^)]*:\s*String"#).unwrap(),
    ]
});

/// Safe patterns (validation exists)
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // validate function call
        Regex::new(r#"validate_input\s*\("#).unwrap(),
        // Result return type
        Regex::new(r#"\)\s*->\s*Result\s*\("#).unwrap(),
        // Guard clause
        Regex::new(r#"if\s+\w+\.(?:is_empty|len)"#).unwrap(),
    ]
});

/// CC_SEC_INP_013 Rule: Missing Input Sanitization
pub struct MissingInputSanitizationRule;

impl Default for MissingInputSanitizationRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for MissingInputSanitizationRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_013")
    }

    fn name(&self) -> &'static str {
        "Missing Input Sanitization on Public Interface"
    }

    fn description(&self) -> &'static str {
        "Detects function parameters that accept user input without validation before sensitive operations"
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

        // Line-by-line scanning for missing sanitization
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for public function with String parameters
            let has_pub_fn = trimmed.contains("pub fn")
                && trimmed.contains("String");

            if !has_pub_fn {
                continue;
            }

            // Check for patterns
            for pattern in MISSING_SANITIZATION_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Look ahead to check if validation exists
                    let next_lines: String = source.lines()
                        .skip(line_num)
                        .take(10)
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Check if validation exists in function body
                    let has_validation = SAFE_PATTERNS.iter().any(|p| p.is_match(&next_lines));

                    if !has_validation {
                        issues.push(Issue::new(
                            "CC_SEC_INP_013",
                            "Missing Input Sanitization on Public Interface",
                            Severity::Minor,
                            Category::Security,
                            ctx.file_path.to_string_lossy(),
                            line_num + 1,
                            0,
                            "Function parameter appears to be used in sensitive operation without visible validation. \
                             Add input validation at function entry point.".to_string(),
                        ));
                    }
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["pub fn", "user", "input", "validate"])
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
        let rule = MissingInputSanitizationRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_missing_validation() {
        let code = r#"pub fn process_user(data: String) -> Result<()> { db.execute() }"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect missing validation");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_013");
    }

    #[test]
    fn test_safe_with_validation() {
        let code = r#"pub fn process_user(data: String) -> Result<()> { validate_input(&data)?; Ok(()) }"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag with validation");
    }

    #[test]
    fn test_safe_with_result_return() {
        let code = r#"pub fn process(data: String) -> Result<()> { if data.is_empty() { return Err(()) } Ok(()) }"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag with Result return and guard");
    }
}