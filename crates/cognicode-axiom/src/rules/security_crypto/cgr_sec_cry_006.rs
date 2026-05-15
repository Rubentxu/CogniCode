//! CC_SEC_CRY_006: Deprecated Cryptographic Functions
//!
//! Detects use of deprecated cryptographic APIs and functions.
//!
//! # Problem
//! Deprecated cryptographic functions may have known vulnerabilities or
//! are being phased out in favor of more secure alternatives.
//!
//! # Fix
//! Use modern cryptographic APIs per language best practices:
//! - Node.js: crypto.subtle or updated Node.js crypto module
//! - Python: hashlib with modern algorithms
//! - Java: KeyGenerator with AES/GCM
//! - Rust: ring or aes-gcm crates

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for deprecated crypto functions
static DEPRECATED_CRYPTO_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Node.js deprecated crypto methods
        Regex::new(r"crypto\.createCipher\s*\(").unwrap(),
        Regex::new(r"crypto\.createDecipher\s*\(").unwrap(),
        Regex::new(r"crypto\.createCipheriv\s*\(").unwrap(),
        Regex::new(r"crypto\.createDecipheriv\s*\(").unwrap(),
        // Python hashlib deprecated
        Regex::new(r"hashlib\.md5\s*\(").unwrap(),
        Regex::new(r"hashlib\.sha1\s*\(").unwrap(),
        // Java deprecated classes
        Regex::new(r"DESKeySpec\s*\(").unwrap(),
        Regex::new(r"DESedeKeySpec\s*\(").unwrap(),
        Regex::new(r"RC2ParameterSpec\s*\(").unwrap(),
        // Java Cipher with deprecated algorithms
        Regex::new(r#"Cipher\.getInstance\s*\(\s*['"]DES\b"#).unwrap(),
        Regex::new(r#"Cipher\.getInstance\s*\(\s*['"]DESede\b"#).unwrap(),
        // Rust deprecated crypto
        Regex::new(r"use\s+.*\bmd5\s*::").unwrap(),
        Regex::new(r"use\s+.*\bsha1\s*::").unwrap(),
        // Go deprecated
        Regex::new(r"crypto/md5\b").unwrap(),
        Regex::new(r"crypto/sha1\b").unwrap(),
    ]
});

/// CC_SEC_CRY_006 Rule: Deprecated Cryptographic Functions
pub struct DeprecatedCryptoFunctionsRule;

impl Default for DeprecatedCryptoFunctionsRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for DeprecatedCryptoFunctionsRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_CRY_006")
    }

    fn name(&self) -> &'static str {
        "Deprecated Cryptographic Functions"
    }

    fn description(&self) -> &'static str {
        "Detects use of deprecated cryptographic APIs that have known weaknesses or are being phased out"
    }

    fn category(&self) -> Category {
        Category::Security
    }

    fn severity(&self) -> Severity {
        Severity::Major
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

        // Line-by-line scanning for deprecated crypto patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*")
                || trimmed.starts_with("<!--") {
                continue;
            }

            // Check each pattern
            for pattern in DEPRECATED_CRYPTO_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check for migration comments (allow if migration is documented)
                    let context_start = std::cmp::max(0, line_num.saturating_sub(2));
                    let context_end = std::cmp::min(source.lines().count(), line_num + 3);
                    let context_lower: String = source
                        .lines()
                        .skip(context_start)
                        .take(context_end - context_start)
                        .collect::<Vec<_>>()
                        .join("\n")
                        .to_lowercase();

                    if context_lower.contains("migration") || context_lower.contains("migrate")
                        || (context_lower.contains("deprecated") && context_lower.contains("instead")) {
                        continue; // Has migration documentation
                    }

                    issues.push(Issue::new(
                        "CC_SEC_CRY_006",
                        "Deprecated Cryptographic Functions",
                        Severity::Major,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Deprecated cryptographic function detected. This API has known weaknesses \
                         or is being phased out. Consult language-specific crypto best practices.".to_string(),
                    ));
                    break; // One issue per line is enough
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["crypto", "cipher", "decipher", "hashlib", "subtle", "md5", "sha1", "des"])
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
            std::path::Path::new("test.js"),
            &language,
            &metrics,
        );
        let rule = DeprecatedCryptoFunctionsRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_createcipher_js() {
        let code = r#"
const crypto = require('crypto');
const cipher = crypto.createCipher('aes-256-cbc', key);
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect createCipher");
        assert_eq!(issues[0].rule_id, "CC_SEC_CRY_006");
    }

    #[test]
    fn test_detects_createdecipher_js() {
        let code = r#"
const decipher = crypto.createDecipher('aes-256-cbc', key);
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect createDecipher");
    }

    #[test]
    fn test_detects_md5_python() {
        let code = r#"
import hashlib
h = hashlib.md5()
h.update(b"data")
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect hashlib.md5");
    }

    #[test]
    fn test_detects_des_java() {
        let code = r#"
import javax.crypto.spec.DESKeySpec;
DESKeySpec keySpec = new DESKeySpec(keyBytes);
"#;
        let issues = check_rule(code, SrcLanguage::Java);
        assert!(!issues.is_empty(), "Should detect DESKeySpec");
    }

    #[test]
    fn test_no_false_positive_with_migration_comment() {
        let code = r#"
// Deprecated: use crypto.subtle instead
// Migration in progress to AES-GCM
const cipher = crypto.createCipher('aes-256-cbc', key);
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(issues.is_empty(), "Should not flag with migration comment");
    }
}
