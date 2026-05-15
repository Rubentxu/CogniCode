//! CC_SEC_CRY_001: Hardcoded Credentials
//!
//! Detects hardcoded passwords, secrets, tokens, and API keys in source code.
//!
//! # Problem
//! Hardcoded credentials in source code can be extracted by anyone with access
//! to the repository, leading to unauthorized access.
//!
//! # Fix
//! Use environment variables or secrets management services:
//! - AWS Secrets Manager, HashiCorp Vault, GCP Secret Manager

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Credential assignment patterns (variable name followed by string literal)
/// Matches patterns like: password = "...", api_key: "...", etc.
static CREDENTIAL_ASSIGNMENT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Pattern: identifier = "value" or identifier:"value" with credential name
        Regex::new(r#"(?i)\b(password|secret|token|api_key|apikey|credential|auth|passwd|pwd)\s*[=:]\s*["'][^"']{4,}["']"#).unwrap(),
        // Pattern: const identifier = "value" with credential name
        Regex::new(r#"(?i)\b(const\s+)?(password|secret|token|api_key|apikey|credential|auth|passwd|pwd)\s*[=:]\s*["'][^"']{4,}["']"#).unwrap(),
    ]
});

/// Allowlist patterns
static ALLOWLIST_PATTERNS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(DEFAULT_|EXAMPLE_|TEST_|PLACEHOLDER_|DUMMY_)").unwrap()
});

/// CC_SEC_CRY_001 Rule: Hardcoded Credentials
pub struct HardcodedCredentialsRule;

impl Default for HardcodedCredentialsRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for HardcodedCredentialsRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_CRY_001")
    }

    fn name(&self) -> &'static str {
        "Hardcoded Credentials"
    }

    fn description(&self) -> &'static str {
        "Detects hardcoded passwords, secrets, tokens, and API keys in source code"
    }

    fn category(&self) -> Category {
        Category::Security
    }

    fn severity(&self) -> Severity {
        Severity::Blocker
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[
            SrcLanguage::Rust,
            SrcLanguage::Python,
            SrcLanguage::JavaScript,
            SrcLanguage::TypeScript,
            SrcLanguage::Go,
            SrcLanguage::Java,
        ]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Skip test files
        let path_str = ctx.file_path.to_string_lossy();
        if path_str.contains("_test.") || path_str.contains("test_") || path_str.contains("/tests/") {
            return issues;
        }

        // Line-by-line scanning for credential patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*")
                || trimmed.starts_with("<!--") {
                continue;
            }

            // Check each pattern
            for pattern in CREDENTIAL_ASSIGNMENT_PATTERNS.iter() {
                if let Some(caps) = pattern.captures(line) {
                    let full_match = caps.get(0).unwrap().as_str();

                    // Extract variable name for allowlist check
                    let var_name_start = full_match.find(|c: char| c.is_alphabetic()).unwrap_or(0);
                    let var_name_end = full_match.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(full_match.len());
                    let var_name = &full_match[var_name_start..var_name_end];

                    // Check allowlist
                    if ALLOWLIST_PATTERNS.is_match(var_name) {
                        continue;
                    }

                    // Skip environment variable access patterns
                    if line.contains("env::var") || line.contains("process.env")
                        || line.contains("os.getenv") || line.contains("std::env::var")
                        || line.contains("getenv(") || line.contains("process.env") {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_CRY_001",
                        "Hardcoded Credentials",
                        Severity::Blocker,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        format!(
                            "Hardcoded credential detected: '{}' appears to contain a secret value. \
                             Credentials should never be embedded in source code.",
                            var_name
                        ),
                    ));
                    break; // One issue per line is enough
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["password", "secret", "token", "api_key", "credential"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_rule(code: &str, language: SrcLanguage, file_path: &str) -> Vec<Issue> {
        let lang = language.to_ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(code, None).unwrap();
        let source = code.to_string();
        let metrics = crate::types::FileMetrics::default();
        let ctx = RuleContext::new(
            &tree,
            &source,
            std::path::Path::new(file_path),
            &language,
            &metrics,
        );
        let rule = HardcodedCredentialsRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_hardcoded_password_rust() {
        let code = r#"
fn authenticate() {
    let password = "super_secret_123";
    println!("{}", password);
}
"#;
        let issues = check_rule(code, SrcLanguage::Rust, "auth.rs");
        assert!(!issues.is_empty(), "Should detect hardcoded password");
        assert_eq!(issues[0].rule_id, "CC_SEC_CRY_001");
    }

    #[test]
    fn test_detects_api_key_python() {
        let code = r#"
def get_data():
    api_key = "sk-1234567890abcdef"
    return api_key
"#;
        let issues = check_rule(code, SrcLanguage::Python, "api.py");
        assert!(!issues.is_empty(), "Should detect hardcoded API key");
    }

    #[test]
    fn test_no_false_positive_env_variable() {
        let code = r#"
fn authenticate() {
    let password = std::env::var("PASSWORD").unwrap();
}
"#;
        let issues = check_rule(code, SrcLanguage::Rust, "auth.rs");
        assert!(issues.is_empty(), "Should not flag environment variable access");
    }

    #[test]
    fn test_no_false_positive_allowlist() {
        let code = r#"
const DEFAULT_PASSWORD: &str = "placeholder";
"#;
        let issues = check_rule(code, SrcLanguage::Rust, "constants.rs");
        assert!(issues.is_empty(), "Should not flag allowlisted variable names");
    }

    #[test]
    fn test_detects_hardcoded_token_js() {
        let code = r#"
function authenticate() {
    const token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
    return token;
}
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript, "auth.js");
        assert!(!issues.is_empty(), "Should detect hardcoded token");
    }

    #[test]
    fn test_no_false_positive_test_file() {
        let code = r#"
fn test_auth() {
    let password = "test_password";
}
"#;
        let issues = check_rule(code, SrcLanguage::Rust, "auth_test.rs");
        assert!(issues.is_empty(), "Should skip test files");
    }
}
