//! CC_SEC_INP_005: Insecure Deserialization of Untrusted Data

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for insecure deserialization detection
static INSECURE_DESERIALIZE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // bincode::deserialize
        Regex::new(r#"bincode::deserialize"#).unwrap(),
        // serde_yaml::from_str
        Regex::new(r#"serde_yaml::from_str"#).unwrap(),
        // serde_yaml::from_reader
        Regex::new(r#"serde_yaml::from_reader"#).unwrap(),
        // rmp_serde::from_read
        Regex::new(r#"rmp_serde::from_read"#).unwrap(),
        // rmp_serde::from_slice
        Regex::new(r#"rmp_serde::from_slice"#).unwrap(),
    ]
});

/// Safe deserialization patterns
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // serde_json
        Regex::new(r#"serde_json::(?:from_str|from_slice|from_reader)\s*\("#).unwrap(),
        // Integrity check
        Regex::new(r#"verify|checksum|signature"#).unwrap(),
        // Trusted source
        Regex::new(r#"include_str!\s*\("#).unwrap(),
    ]
});

/// CC_SEC_INP_005 Rule: Insecure Deserialization
pub struct InsecureDeserializationRule;

impl Default for InsecureDeserializationRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for InsecureDeserializationRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_005")
    }

    fn name(&self) -> &'static str {
        "Insecure Deserialization of Untrusted Data"
    }

    fn description(&self) -> &'static str {
        "Detects deserialization of untrusted data using unsafe formats"
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

        // Line-by-line scanning for insecure deserialization patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for deserialization keywords
            let has_deser_kw = trimmed.contains("bincode")
                || trimmed.contains("serde_yaml")
                || trimmed.contains("rmp")
                || trimmed.contains("rmp_serde");

            if !has_deser_kw {
                continue;
            }

            // Check for insecure patterns
            for pattern in INSECURE_DESERIALIZE_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check if it's actually safe
                    let is_safe = SAFE_PATTERNS.iter().any(|p| p.is_match(line));
                    if is_safe {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_005",
                        "Insecure Deserialization of Untrusted Data",
                        Severity::Critical,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible insecure deserialization: using unsafe format (bincode/MessagePack/YAML) \
                         on untrusted data. Use serde_json or add integrity verification.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["deserialize", "bincode", "yaml", "serde", "MessagePack", "rmp"])
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
        let rule = InsecureDeserializationRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_bincode_deserialize() {
        let code = r#"bincode::deserialize::<Config>(data)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect bincode::deserialize");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_005");
    }

    #[test]
    fn test_detects_yaml_from_str() {
        let code = r#"serde_yaml::from_str::<Config>(input)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect serde_yaml::from_str");
    }

    #[test]
    fn test_safe_json_deserialization() {
        let code = r#"serde_json::from_str::<Value>(input)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag serde_json");
    }

    #[test]
    fn test_safe_with_integrity_check() {
        let code = r#"let hash = sha256(data); verify(&hash)?; bincode::deserialize(data)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag with integrity check");
    }

    #[test]
    fn test_safe_include_str() {
        let code = r#"bincode::deserialize(include_str!("config.bin").as_bytes())"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag trusted source");
    }
}