//! CC_SEC_INP_009: Path Equivalence Without Verification

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for path equivalence issues
static PATH_EQUIV_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // canonicalize without starts_with check
        Regex::new(r#"\.canonicalize\(\)\s*\?;"#).unwrap(),
        // Path::new without validation
        Regex::new(r#"Path::new\s*\(\s*\w+\s*\)"#).unwrap(),
    ]
});

/// Safe path patterns
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // starts_with check
        Regex::new(r#"starts_with\s*\("#).unwrap(),
    ]
});

/// CC_SEC_INP_009 Rule: Path Equivalence Without Verification
pub struct PathEquivalenceRule;

impl Default for PathEquivalenceRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for PathEquivalenceRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_009")
    }

    fn name(&self) -> &'static str {
        "Path Equivalence Without Verification"
    }

    fn description(&self) -> &'static str {
        "Detects path normalization operations that don't verify boundaries"
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

        // Line-by-line scanning for path equivalence issues
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for path-related keywords
            let has_path_kw = trimmed.contains("canonicalize")
                || trimmed.contains("Path::new");

            if !has_path_kw {
                continue;
            }

            // Check for issues
            for pattern in PATH_EQUIV_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check if it's actually safe
                    if SAFE_PATTERNS.iter().any(|p| p.is_match(line)) {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_009",
                        "Path Equivalence Without Verification",
                        Severity::Major,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Path normalization may not prevent access to files outside intended directory. \
                         Add starts_with() check against base directory after canonicalize().".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["canonicalize", "Path::new", "normalize"])
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
        let rule = PathEquivalenceRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_unsafe_canonicalize() {
        let code = r#"let path = Path::new(&filename).canonicalize()?;"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect canonicalize without check");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_009");
    }

    #[test]
    fn test_safe_with_starts_with_check() {
        let code = r#"if path.starts_with(base) { Ok(()) }"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag with starts_with check");
    }
}