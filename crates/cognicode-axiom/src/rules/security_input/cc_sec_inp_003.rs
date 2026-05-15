//! CC_SEC_INP_003: Path Traversal via User Input in File Operations

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for path traversal detection
static PATH_TRAVERSAL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // File::open with format!
        Regex::new(r#"File::open\s*\(\s*format!\s*\("#).unwrap(),
        // std::fs::read with format!
        Regex::new(r#"std::fs::read\s*\(\s*format!\s*\("#).unwrap(),
        Regex::new(r#"std::fs::read_to_string\s*\(\s*format!\s*\("#).unwrap(),
        // Path::new with format!
        Regex::new(r#"Path::new\s*\(\s*format!\s*\("#).unwrap(),
        // .join() with format!
        Regex::new(r#"\.join\s*\(\s*format!\s*\("#).unwrap(),
    ]
});

/// Safe path patterns
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // canonicalize with starts_with check
        Regex::new(r#"canonicalize\(\)"#).unwrap(),
        Regex::new(r#"starts_with\s*\("#).unwrap(),
        // Constant paths
        Regex::new(r#"File::open\s*\(\s*"[^"]+"\s*\)"#).unwrap(),
    ]
});

/// CC_SEC_INP_003 Rule: Path Traversal via User Input
pub struct PathTraversalRule;

impl Default for PathTraversalRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for PathTraversalRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_003")
    }

    fn name(&self) -> &'static str {
        "Path Traversal via User Input in File Operations"
    }

    fn description(&self) -> &'static str {
        "Detects file path operations where user input is concatenated without validation"
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

        // Line-by-line scanning for path traversal patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for file operation keywords
            let has_file_kw = trimmed.contains("File::open")
                || trimmed.contains("std::fs::read")
                || trimmed.contains("Path::new")
                || trimmed.contains(".join(");

            if !has_file_kw {
                continue;
            }

            // Check for traversal patterns
            for pattern in PATH_TRAVERSAL_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check if canonicalize is used
                    if line.contains("canonicalize()") {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_003",
                        "Path Traversal via User Input in File Operations",
                        Severity::Critical,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible path traversal: user input concatenated into file path without validation. \
                         Use canonicalize() with starts_with() check against base directory.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["open", "read", "write", "Path", "join", "canonicalize", "format"])
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
        let rule = PathTraversalRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_path_traversal_file_open() {
        let code = r#"File::open(format!("{}/{}", dir, name))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect path traversal via format!");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_003");
    }

    #[test]
    fn test_detects_join_with_format() {
        let code = r#"base.join(format!("{}/file.txt", user_input))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect .join with format!");
    }

    #[test]
    fn test_safe_constant_path() {
        let code = r#"std::fs::read_to_string("/etc/config/app.conf")"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag constant paths");
    }
}