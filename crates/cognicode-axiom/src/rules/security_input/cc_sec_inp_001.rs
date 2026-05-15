//! CC_SEC_INP_001: SQL Injection via String Concatenation
//!
//! Detects SQL queries constructed using string concatenation or interpolation
//! with user-controlled input, allowing attackers to manipulate query logic.
//!
//! # Problem
//! When SQL queries are built by concatenating user input, attackers can
//! inject malicious SQL code to manipulate database operations.
//!
//! # Fix
//! Use parameterized queries or prepared statements:
//! - query!("SELECT * FROM users WHERE id = $1", user_id)
//! - sqlx::query("SELECT * FROM users WHERE id = ?").bind(user_id)

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for SQL injection detection
static SQL_INJECTION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Pattern: format!("SELECT ... {}", user_input)
        Regex::new(r#"format!\s*\([^)]*SELECT[^)]*\{\}"#).unwrap(),
        Regex::new(r#"format!\s*\([^)]*INSERT[^)]*\{\}"#).unwrap(),
        Regex::new(r#"format!\s*\([^)]*UPDATE[^)]*\{\}"#).unwrap(),
        Regex::new(r#"format!\s*\([^)]*DELETE[^)]*\{\}"#).unwrap(),
        Regex::new(r#"format!\s*\([^)]*DROP[^)]*\{\}"#).unwrap(),
        // Pattern: "SELECT ... " + user_input
        Regex::new(r#""[^"]*SELECT[^"]*"\s*\+"#).unwrap(),
        Regex::new(r#""[^"]*INSERT[^"]*"\s*\+"#).unwrap(),
        Regex::new(r#""[^"]*FROM[^"]*"\s*\+"#).unwrap(),
    ]
});

/// Safe query patterns (parameterized)
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Parameterized placeholders
        Regex::new(r#"\$\d+|\?"#).unwrap(),
        // Safe query builders
        Regex::new(r#"query!\s*\("#).unwrap(),
        Regex::new(r#"sqlx::query\("#).unwrap(),
        // Prepared statements
        Regex::new(r#"prepare\s*\("#).unwrap(),
    ]
});

/// CC_SEC_INP_001 Rule: SQL Injection via String Concatenation
pub struct SqlInjectionRule;

impl Default for SqlInjectionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for SqlInjectionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_001")
    }

    fn name(&self) -> &'static str {
        "SQL Injection via String Concatenation"
    }

    fn description(&self) -> &'static str {
        "Detects SQL queries constructed using string concatenation with user input"
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

        // Line-by-line scanning for SQL injection patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for injection patterns
            let has_sql_kw = trimmed.contains("SELECT")
                || trimmed.contains("INSERT")
                || trimmed.contains("UPDATE")
                || trimmed.contains("DELETE")
                || trimmed.contains("DROP")
                || trimmed.contains("WHERE");

            if !has_sql_kw {
                continue;
            }

            for pattern in SQL_INJECTION_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check if it's actually safe
                    let is_safe = SAFE_PATTERNS.iter().any(|p| p.is_match(line));
                    if is_safe {
                        continue;
                    }

                    // Check for common sanitization functions
                    if line.contains("sql_escape")
                        || line.contains("quote(")
                        || line.contains("bind(")
                        || line.contains("$1")
                        || line.contains("?") {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_001",
                        "SQL Injection via String Concatenation",
                        Severity::Critical,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible SQL injection: user input concatenated into SQL query without parameterization. \
                         Use parameterized queries (?, $1) or query builders.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["query", "execute", "sql", "format!", "diesel", "sqlx", "rusqlite"])
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
        let rule = SqlInjectionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_sql_injection_format() {
        let code = r#"let query = format!("SELECT * FROM users WHERE id = {}", user_id);"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect SQL injection via format!");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_001");
    }

    #[test]
    fn test_detects_string_concatenation() {
        let code = r#"let sql = "SELECT * FROM users WHERE name = '" + &username + "'";"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect SQL injection via string concat");
    }

    #[test]
    fn test_safe_parameterized_query() {
        let code = r#"sqlx::query("SELECT * FROM users WHERE id = $1").bind(user_id)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag parameterized queries");
    }

    #[test]
    fn test_safe_query_macro() {
        let code = r#"sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", user_id)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag query! macro");
    }

    #[test]
    fn test_no_false_positive_constant() {
        let code = r#"let query = "SELECT * FROM users WHERE active = true";"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag constant SQL strings");
    }
}