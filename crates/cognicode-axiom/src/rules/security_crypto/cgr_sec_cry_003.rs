//! CC_SEC_CRY_003: Weak Encryption
//!
//! Detects use of weak encryption algorithms like DES, 3DES, RC2, RC4, or ECB mode.
//!
//! # Problem
//! DES has a 56-bit key space (brute-forceable since 1998). RC4 has exploitable
//! biases. ECB mode reveals patterns in encrypted data.
//!
//! # Fix
//! For encryption: use AES-256-GCM or ChaCha20-Poly1305.
//! For block cipher modes: avoid ECB, use GCM or CBC with proper IV.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for weak encryption detection
static WEAK_CIPHER_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // DES, 3DES, RC4, RC2
        Regex::new(r"(?i)\bDES\b").unwrap(),
        Regex::new(r"(?i)\b3DES\b").unwrap(),
        Regex::new(r"(?i)\bDESede\b").unwrap(),
        Regex::new(r"(?i)\bRC4\b").unwrap(),
        Regex::new(r"(?i)\bRC2\b").unwrap(),
        Regex::new(r"(?i)\bECB\b").unwrap(),
        // Python crypto patterns
        Regex::new(r"(?i)Crypto\.Cipher\.(DES|3DES|RC4|RC2)").unwrap(),
        Regex::new(r"(?i)from\s+Crypto\.Cipher\s+import\s+.*(DES|3DES|RC4|RC2)").unwrap(),
        // JavaScript deprecated cipher
        Regex::new(r"(?i)crypto\.createCipher\s*\(").unwrap(),
        Regex::new(r"(?i)crypto\.createDecipher\s*\(").unwrap(),
        Regex::new(r#"(?i)crypto\.createCipheriv\s*\([^)]*,\s*['"](des|3des|rc4|rc2|ecb)['"]"#).unwrap(),
        // Java patterns
        Regex::new(r#"(?i)Cipher\.getInstance\s*\(\s*['"](DES|DESede|RC[24])['"]"#).unwrap(),
        Regex::new(r"(?i)DESKeySpec\s*\(").unwrap(),
        // Rust patterns
        Regex::new(r"(?i)use\s+.*\b(des|3des|rc4|rc2)\s*::").unwrap(),
        // Go patterns
        Regex::new(r"(?i)crypto/des\b").unwrap(),
        Regex::new(r"(?i)crypto/rc4\b").unwrap(),
    ]
});

/// CC_SEC_CRY_003 Rule: Weak Encryption
pub struct WeakEncryptionRule;

impl Default for WeakEncryptionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for WeakEncryptionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_CRY_003")
    }

    fn name(&self) -> &'static str {
        "Weak Encryption"
    }

    fn description(&self) -> &'static str {
        "Detects weak encryption algorithms (DES, 3DES, RC4, ECB mode) that provide inadequate protection"
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

        // Line-by-line scanning for weak cipher patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*")
                || trimmed.starts_with("<!--") {
                continue;
            }

            // Check each pattern
            for pattern in WEAK_CIPHER_PATTERNS.iter() {
                if pattern.is_match(line) {
                    issues.push(Issue::new(
                        "CC_SEC_CRY_003",
                        "Weak Encryption",
                        Severity::Critical,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Weak encryption detected (DES, 3DES, RC4, RC2, or ECB mode). \
                         These provide inadequate protection. Use AES-256-GCM or ChaCha20-Poly1305 instead.".to_string(),
                    ));
                    break; // One issue per line is enough
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["des", "rc4", "rc2", "3des", "ecb", "cipher", "encrypt", "Crypto"])
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
        let rule = WeakEncryptionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_des_java() {
        let code = r#"
import javax.crypto.Cipher;
public static byte[] encrypt(byte[] data, Key key) throws Exception {
    Cipher cipher = Cipher.getInstance("DES");
    cipher.init(Cipher.ENCRYPT_MODE, key);
    return cipher.doFinal(data);
}
"#;
        let issues = check_rule(code, SrcLanguage::Java);
        assert!(!issues.is_empty(), "Should detect DES in Java");
        assert_eq!(issues[0].rule_id, "CC_SEC_CRY_003");
    }

    #[test]
    fn test_detects_createcipher_js() {
        let code = r#"
const crypto = require('crypto');
const cipher = crypto.createCipher('des', key);
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect createCipher");
    }

    #[test]
    fn test_detects_rc4_python() {
        let code = r#"
from Crypto.Cipher import RC4
cipher = RC4.new(key)
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect RC4 in Python");
    }
}
