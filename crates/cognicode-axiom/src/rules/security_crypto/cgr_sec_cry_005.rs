//! CC_SEC_CRY_005: Insecure TLS Configuration
//!
//! Detects insecure TLS/SSL configuration that disables certificate verification.
//!
//! # Problem
//! Disabling TLS certificate verification allows man-in-the-middle attacks,
//! enabling attackers to intercept and modify encrypted communications.
//!
//! # Fix
//! Enable certificate verification and configure proper CA bundles.
//! For development with self-signed certs: use devcert or mkcert.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for insecure TLS verification disable
static INSECURE_TLS_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // verify: false or verify: 0 or verify: False (case insensitive)
        Regex::new(r"(?i)verify\s*[:=]\s*(false|0)\b").unwrap(),
        Regex::new(r"(?i)\bNoVerify\b").unwrap(),
        Regex::new(r"(?i)ALLOW_SELF_SIGNED\s*[:=]").unwrap(),
        // accept_invalid_certs(true) or acceptInvalidCerts: true
        Regex::new(r"accept_invalid_certs?\s*\(\s*true\s*\)").unwrap(),
        Regex::new(r"acceptInvalidCerts\s*[:\s]+\s*true\b").unwrap(),
        Regex::new(r"danger_accept_invalid_certs\s*\(\s*true\s*\)").unwrap(),
        // ssl_verify: false
        Regex::new(r"ssl_verify\s*[:=]\s*false").unwrap(),
        Regex::new(r"tls_verify\s*[:=]\s*false").unwrap(),
        // insecure: true
        Regex::new(r"insecure\s*[:=]\s*true").unwrap(),
        // request({ ... verify: false })
        Regex::new(r"request\s*\(\s*\{[^}]*verify\s*:\s*false").unwrap(),
        // set_openssl_verify(0)
        Regex::new(r"set_openssl_verify\s*\(\s*0\s*\)").unwrap(),
    ]
});

/// CC_SEC_CRY_005 Rule: Insecure TLS Configuration
pub struct InsecureTlsConfigRule;

impl Default for InsecureTlsConfigRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for InsecureTlsConfigRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_CRY_005")
    }

    fn name(&self) -> &'static str {
        "Insecure TLS Configuration"
    }

    fn description(&self) -> &'static str {
        "Detects TLS/SSL configuration that disables certificate verification"
    }

    fn category(&self) -> Category {
        Category::Security
    }

    fn severity(&self) -> Severity {
        Severity::Critical
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

        // Line-by-line scanning for insecure TLS patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*")
                || trimmed.starts_with("<!--") {
                continue;
            }

            // Check each pattern
            for pattern in INSECURE_TLS_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check for test/dev context indicators in surrounding lines
                    let context_start = std::cmp::max(0, line_num.saturating_sub(5));
                    let context_end = std::cmp::min(source.lines().count(), line_num + 6);
                    let context: String = source
                        .lines()
                        .skip(context_start)
                        .take(context_end - context_start)
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Skip if clearly in test context
                    if context.contains("#[cfg(test)]") || context.contains("#[cfg(dev)]")
                        || context.contains("NODE_ENV=test") || context.contains("test_mode")
                        || context.contains("MOCK") || context.contains("MOCK_") {
                        continue;
                    }

                    // Check if pattern is only in a comment
                    let code_part = if let Some(idx) = line.find("//") {
                        &line[..idx]
                    } else if let Some(idx) = line.find('#') {
                        &line[..idx]
                    } else {
                        line
                    };

                    let mut found_in_code = false;
                    for p in INSECURE_TLS_PATTERNS.iter() {
                        if p.is_match(code_part) {
                            found_in_code = true;
                            break;
                        }
                    }
                    if !found_in_code {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_CRY_005",
                        "Insecure TLS Configuration",
                        Severity::Critical,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Insecure TLS configuration detected: certificate verification appears \
                         to be disabled. This allows man-in-the-middle attacks.".to_string(),
                    ));
                    break; // One issue per line is enough
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["verify", "ssl", "tls", "cert", "https", "insecure"])
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
        let rule = InsecureTlsConfigRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_verify_false_python() {
        let code = r#"
import requests
response = requests.get(url, verify=False)
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect verify=False");
        assert_eq!(issues[0].rule_id, "CC_SEC_CRY_005");
    }

    #[test]
    fn test_detects_accept_invalid_certs() {
        let code = r#"
fetch(url, { acceptInvalidCerts: true })
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect acceptInvalidCerts");
    }

    #[test]
    fn test_detects_ssl_verify_false() {
        let code = r#"
requests.get(url, ssl_verify=False)
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect ssl_verify=False");
    }

    #[test]
    fn test_no_false_positive_comment() {
        let code = r#"
// This is fine: verify=False is only for testing
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(issues.is_empty(), "Should not flag comment-only mentions");
    }

    #[test]
    fn test_no_false_positive_test_context() {
        let code = r#"
#[cfg(test)]
mod tests {
    fn mock_request() {
        let response = client.get(url, verify: false);
    }
}
"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag test context");
    }
}
