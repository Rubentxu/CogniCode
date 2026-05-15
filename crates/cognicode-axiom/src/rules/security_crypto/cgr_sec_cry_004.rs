//! CC_SEC_CRY_004: Hardcoded Cryptographic Key
//!
//! Detects hardcoded AES, RSA, or other cryptographic keys in source code.
//!
//! # Problem
//! Hardcoded crypto keys can be extracted by anyone with access to the
//! repository, compromising the security of all data protected by those keys.
//!
//! # Fix
//! Use environment variables, AWS KMS, HashiCorp Vault, or HSM storage
//! for cryptographic keys.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Key variable name patterns
static KEY_PATTERNS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(key|secret_key|private_key|public_key|encryption_key|crypto_key|hmac_key|signing_key|aes_key|rsa_key)\s*[=:]\s*["'][^"']{8,}["']"#)
        .unwrap()
});

/// Allowlist patterns
static ALLOWLIST_PATTERNS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(TEST_|EXAMPLE_|MOCK_|PLACEHOLDER_|DEFAULT_|DUMMY_)").unwrap()
});

/// CC_SEC_CRY_004 Rule: Hardcoded Crypto Key
pub struct HardcodedCryptoKeyRule;

impl Default for HardcodedCryptoKeyRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for HardcodedCryptoKeyRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_CRY_004")
    }

    fn name(&self) -> &'static str {
        "Hardcoded Cryptographic Key"
    }

    fn description(&self) -> &'static str {
        "Detects hardcoded cryptographic keys (AES, RSA, HMAC) embedded in source code"
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

        // Line-by-line scanning for key patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*")
                || trimmed.starts_with("<!--") {
                continue;
            }

            // Check key pattern
            if let Some(caps) = KEY_PATTERNS.captures(line) {
                let full_match = caps.get(0).unwrap().as_str();

                // Extract variable name
                let var_name_start = full_match.find(|c: char| c.is_alphabetic()).unwrap_or(0);
                let var_name_end = full_match.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(full_match.len());
                let var_name = &full_match[var_name_start..var_name_end];

                // Check allowlist
                if ALLOWLIST_PATTERNS.is_match(var_name) {
                    continue;
                }

                // Skip environment variable patterns
                if line.contains("env::var") || line.contains("process.env")
                    || line.contains("os.getenv") || line.contains("std::env::var")
                    || line.contains("getenv(") {
                    continue;
                }

                issues.push(Issue::new(
                    "CC_SEC_CRY_004",
                    "Hardcoded Cryptographic Key",
                    Severity::Blocker,
                    Category::Security,
                    ctx.file_path.to_string_lossy(),
                    line_num + 1,
                    0,
                    format!(
                        "Hardcoded cryptographic key detected: '{}' is assigned a literal value. \
                         Cryptographic keys must never be embedded in source code.",
                        var_name
                    ),
                ));
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["key", "secret_key", "private_key", "encryption_key", "aes_key", "rsa_key"])
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
        let rule = HardcodedCryptoKeyRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_hardcoded_aes_key_rust() {
        let code = r#"
fn decrypt(data: &[u8]) -> Vec<u8> {
    let aes_key = "0123456789abcdef0123456789abcdef";
    // ... decryption logic
}
"#;
        let issues = check_rule(code, SrcLanguage::Rust, "crypto.rs");
        assert!(!issues.is_empty(), "Should detect hardcoded AES key");
        assert_eq!(issues[0].rule_id, "CC_SEC_CRY_004");
    }

    #[test]
    fn test_detects_private_key_python() {
        let code = r#"
def sign(data: bytes) -> bytes:
    private_key = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA..."
    return signing_function(data, private_key)
"#;
        let issues = check_rule(code, SrcLanguage::Python, "crypto.py");
        assert!(!issues.is_empty(), "Should detect hardcoded private key");
    }

    #[test]
    fn test_no_false_positive_env_var() {
        let code = r#"
fn decrypt(data: &[u8]) -> Vec<u8> {
    let key = std::env::var("AES_KEY").unwrap();
    // ... decryption logic
}
"#;
        let issues = check_rule(code, SrcLanguage::Rust, "crypto.rs");
        assert!(issues.is_empty(), "Should not flag environment variable");
    }

    #[test]
    fn test_no_false_positive_allowlist() {
        let code = r#"
const TEST_KEY: &str = "0123456789abcdef";
"#;
        let issues = check_rule(code, SrcLanguage::Rust, "crypto.rs");
        assert!(issues.is_empty(), "Should not flag test/placeholder keys");
    }

    #[test]
    fn test_detects_encryption_key_js() {
        let code = r#"
function encrypt(data) {
    const encryption_key = "abcdefghijklmnop";
    return crypto.subtle.encrypt(encryption_key, data);
}
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript, "crypto.js");
        assert!(!issues.is_empty(), "Should detect encryption_key");
    }
}
