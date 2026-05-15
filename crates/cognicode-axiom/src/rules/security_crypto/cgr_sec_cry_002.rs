//! CC_SEC_CRY_002: Weak Cryptographic Hash
//!
//! Detects use of MD5 or SHA1 for security purposes.
//!
//! # Problem
//! MD5 and SHA1 have known collision attacks and should not be used for
//! security purposes. MD5 has been broken since 2004, SHA1 since 2017.
//!
//! # Fix
//! For password hashing: use bcrypt, Argon2, or scrypt.
//! For integrity verification: use SHA-256 or SHA-3 minimum.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for weak hash detection
static WEAK_HASH_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // MD5/SHA1 in various contexts
        Regex::new(r"(?i)\bmd5\s*\(").unwrap(),
        Regex::new(r"(?i)\bsha1\s*\(").unwrap(),
        Regex::new(r"(?i)hashlib\.md5\s*\(").unwrap(),
        Regex::new(r"(?i)hashlib\.sha1\s*\(").unwrap(),
        Regex::new(r#"(?i)crypto\.createHash\s*\(\s*['"]md5['"]"#).unwrap(),
        Regex::new(r#"(?i)crypto\.createHash\s*\(\s*['"]sha1['"]"#).unwrap(),
        Regex::new(r#"(?i)MessageDigest\.getInstance\s*\(\s*['"]MD5['"]"#).unwrap(),
        Regex::new(r#"(?i)MessageDigest\.getInstance\s*\(\s*['"]SHA-?1['"]"#).unwrap(),
        Regex::new(r#"(?i)import\s+.*\bmd5\b"#).unwrap(),
        Regex::new(r#"(?i)import\s+.*\bsha1\b"#).unwrap(),
        Regex::new(r"(?i)use\s+.*md5\s*::").unwrap(),
        Regex::new(r"(?i)use\s+.*sha1\s*::").unwrap(),
        Regex::new(r"(?i)from\s+crypto\s+import\s+.*md5").unwrap(),
        Regex::new(r"(?i)from\s+crypto\s+import\s+.*sha1").unwrap(),
    ]
});

/// CC_SEC_CRY_002 Rule: Weak Cryptographic Hash
pub struct WeakCryptoHashRule;

impl Default for WeakCryptoHashRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for WeakCryptoHashRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_CRY_002")
    }

    fn name(&self) -> &'static str {
        "Weak Cryptographic Hash"
    }

    fn description(&self) -> &'static str {
        "Detects use of MD5 or SHA1 for security purposes - these algorithms are broken"
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

        // Line-by-line scanning for weak hash patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*")
                || trimmed.starts_with("<!--") {
                continue;
            }

            // Check each pattern
            for pattern in WEAK_HASH_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check for git context (SHA1 for commit IDs is acceptable)
                    if (line.contains("git") || line.contains("commit")) && line.contains("sha1") {
                        continue;
                    }

                    // Check for non-security context (checksum)
                    if line.contains("checksum") && line.contains("sha1") {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_CRY_002",
                        "Weak Cryptographic Hash",
                        Severity::Critical,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Weak cryptographic hash detected (MD5/SHA1). These are broken for security \
                         purposes. Use SHA-256, SHA-3, bcrypt, or Argon2 instead.".to_string(),
                    ));
                    break; // One issue per line is enough
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["md5", "sha1", "hashlib", "crypto", "MessageDigest"])
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
            std::path::Path::new("test.py"),
            &language,
            &metrics,
        );
        let rule = WeakCryptoHashRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_md5_python() {
        let code = r#"
import hashlib
def hash_password(password):
    return hashlib.md5(password.encode()).hexdigest()
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect md5 in Python");
        assert_eq!(issues[0].rule_id, "CC_SEC_CRY_002");
    }

    #[test]
    fn test_detects_sha1_javascript() {
        let code = r#"
const crypto = require('crypto');
const hash = crypto.createHash('sha1').update(data).digest('hex');
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect sha1 in JavaScript");
    }

    #[test]
    fn test_detects_md5_java() {
        let code = r#"
import java.security.MessageDigest;
public static String hash(String input) throws Exception {
    MessageDigest md = MessageDigest.getInstance("MD5");
    return bytesToHex(md.digest(input.getBytes()));
}
"#;
        let issues = check_rule(code, SrcLanguage::Java);
        assert!(!issues.is_empty(), "Should detect MD5 in Java");
    }

    #[test]
    fn test_no_false_positive_git_commit() {
        let code = r#"
// Git commit: a1b2c3d4e5f6abcdef1234567890abcdef123456
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(issues.is_empty(), "Should not flag git commit IDs");
    }
}
