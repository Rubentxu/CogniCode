//! S5542 — Weak cryptographic hash (MD5, SHA1)
//!
//! Detects usage of weak cryptographic hash functions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S5542"
    name: "Strong cryptographic hash functions should be used"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "MD5 and SHA1 are weak cryptographic hash functions susceptible to collision attacks. They should not be used for security purposes like digital signatures or password hashing.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect weak hash algorithms
        let weak_hashes = [
            r"hashlib\.md5\s*\(",
            r"hashlib\.sha1\s*\(",
            r#"['"].*MD5.*['"]"#,
            r#"['"].*SHA1.*['"]"#,
            r#"['"].*SHA-1.*['"]"#,
        ];
        let re = weak_hashes.iter()
            .map(|p| regex::Regex::new(p).unwrap())
            .collect::<Vec<_>>();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            for regex in &re {
                if regex.is_match(line) {
                    issues.push(Issue::new(
                        "PY_S5542",
                        format!("Weak cryptographic hash detected (MD5 or SHA1)"),
                        Severity::Critical,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::moderate(
                        "Use a strong hash function like SHA-256 or SHA-3 from the hashlib module."
                    )));
                    break;
                }
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;

    fn with_python_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Python.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Python,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_s5542_registered() {
        let rule = PY_S5542Rule::new();
        assert_eq!(rule.id(), "PY_S5542");
    }

    #[test]
    fn test_s5542_detects_md5() {
        let rule = PY_S5542Rule::new();
        let smelly = r#"
import hashlib
hashlib.md5(password.encode())
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect MD5");
        assert_eq!(issues[0].rule_id, "PY_S5542");
    }

    #[test]
    fn test_s5542_detects_sha1() {
        let rule = PY_S5542Rule::new();
        let smelly = r#"
hashlib.sha1(data)
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect SHA1");
    }

    #[test]
    fn test_s5542_allows_sha256() {
        let rule = PY_S5542Rule::new();
        let clean = r#"
hashlib.sha256(data)
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag SHA-256");
    }
}
