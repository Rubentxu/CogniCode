//! CC_SEC_INP_006: LDAP Injection via Filter String Concatenation

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use regex::Regex;
use std::sync::LazyLock;

/// Patterns for LDAP injection detection
static LDAP_INJECTION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // format! in LDAP search
        Regex::new(r#"format!\s*\([^)]*uid="#).unwrap(),
        // simple_search with format!
        Regex::new(r#"simple_search\s*\("#).unwrap(),
    ]
});

/// Safe LDAP patterns
static SAFE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // filter_escape
        Regex::new(r#"filter_escape\s*\("#).unwrap(),
        // Filter builder
        Regex::new(r#"Filter::"#).unwrap(),
    ]
});

/// CC_SEC_INP_006 Rule: LDAP Injection
pub struct LdapInjectionRule;

impl Default for LdapInjectionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for LdapInjectionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_SEC_INP_006")
    }

    fn name(&self) -> &'static str {
        "LDAP Injection via Filter String Concatenation"
    }

    fn description(&self) -> &'static str {
        "Detects LDAP query construction where user input is concatenated without proper escaping"
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

        // Line-by-line scanning for LDAP injection patterns
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("#")
                || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Check for LDAP-related keywords
            let has_ldap_kw = trimmed.contains("ldap")
                || trimmed.contains(".search")
                || trimmed.contains("simple_search");

            if !has_ldap_kw {
                continue;
            }

            // Check for injection patterns
            for pattern in LDAP_INJECTION_PATTERNS.iter() {
                if pattern.is_match(line) {
                    // Check if it's actually safe
                    if SAFE_PATTERNS.iter().any(|p| p.is_match(line)) {
                        continue;
                    }

                    issues.push(Issue::new(
                        "CC_SEC_INP_006",
                        "LDAP Injection via Filter String Concatenation",
                        Severity::Major,
                        Category::Security,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "Possible LDAP injection: user input concatenated into LDAP filter without escaping. \
                         Use filter_escape() or Filter builder methods.".to_string(),
                    ));
                    break;
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["ldap", "search", "filter", "ldap3"])
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
        let rule = LdapInjectionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_ldap_injection() {
        let code = r#"conn.search(&format!("(uid={})", uid))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(!issues.is_empty(), "Should detect LDAP injection");
        assert_eq!(issues[0].rule_id, "CC_SEC_INP_006");
    }

    #[test]
    fn test_safe_filter_escape() {
        let code = r#"let filter = format!("(uid={})", filter_escape(&uid))"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag with filter_escape");
    }

    #[test]
    fn test_safe_filter_builder() {
        let code = r#"Filter::eq("uid", &uid)"#;
        let issues = check_rule(code, SrcLanguage::Rust);
        assert!(issues.is_empty(), "Should not flag Filter builder");
    }
}