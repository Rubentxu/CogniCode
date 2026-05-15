//! CC_SEC_INP_002: OS Command Injection via Shell Execution
//!
//! Detects process command construction where user input is concatenated
//! into shell commands, allowing arbitrary command execution.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for command injection detection
static CMD_INJECTION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Command::new with format!
        Regex::new(r#"Command::new\s*\(\s*format!\s*\("#).unwrap(),
        // std::process::Command with format! in new
        Regex::new(r#"std::process::Command::new\s*\(\s*format!\s*\("#).unwrap(),
        // .arg with format!
        Regex::new(r#"\.arg\s*\(\s*format!\s*\("#).unwrap(),
        // system() with format
        Regex::new(r#"system\s*\(\s*format!\s*\("#).unwrap(),
        // Shell execution with format
        Regex::new(r#"sh["']?\s*-c["']?\s*.*format!\s*\("#).unwrap(),
    ]
});

/// Safe command patterns
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Individual .arg() calls without format!
        Regex::new(r#"\.arg\s*\(\s*\w+\s*\)"#).unwrap(),
        // .args with iterator
        Regex::new(r#"\.args\s*\(&?\w+)"#).unwrap(),
    ]
});

/// CC_SEC_INP_002 Rule: OS Command Injection via Shell Execution
pub struct CommandInjectionRule;

impl Default for CommandInjectionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for CommandInjectionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_002")
    }

    fn name(&self) -> &'static str {
        "OS Command Injection via Shell Execution"
    }

    fn description(&self) -> &'static str {
        "Detects process command construction where user input is concatenated into shell commands"
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

        // Line-by-line scanning for command injection patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for command-related keywords
            let has_cmd_kw = trimmed.contains("Command::new")
                || trimmed.contains("std::process::Command")
                || trimmed.contains("system(");

            if !has_cmd_kw {
                continue;
            }

            // Check for injection patterns
            for pattern in CMD_INJECTION_PATTERNS.iter() {
                if pattern.is_match(line) {
                    issues.push(Issue::new(
                        "CC_SEC_INP_002",
                        "OS Command Injection via Shell Execution",
                        Severity::Critical,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible command injection: user input concatenated into command without validation. \
                         Use individual .arg() calls or validate input against whitelist.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["Command", "new", "exec", "system", "format"])
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
        let rule = CommandInjectionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_command_injection_format() {
        let code = r#"Command::new(format!("rm -rf {}", path))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect command injection via format!");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_002");
    }

    #[test]
    fn test_detects_shell_command_injection() {
        let code = r#"std::process::Command::new("sh").arg(format!("ls {}", dir))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect shell command injection");
    }

    #[test]
    fn test_safe_individual_args() {
        let code = r#"Command::new("ls").arg(path)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag individual .arg() calls");
    }

    #[test]
    fn test_safe_no_format() {
        let code = r#"Command::new("ls").arg("-la")"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag Command without format");
    }
}