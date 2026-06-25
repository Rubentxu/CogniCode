//! S5332 — Clear-text HTTP
//!
//! Detects usage of HTTP (not HTTPS) URLs which transmit data in clear text.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S5332"
    name: "Clear-text HTTP URLs should not be used"
    severity: Major
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Using HTTP instead of HTTPS transmits data in clear text, allowing attackers to intercept sensitive information like credentials, tokens, and personal data.",
    clean_code: Focused,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            // Check if line contains http:// but NOT https://
            let has_http = line.contains("\"http://") || line.contains("'http://");
            let has_https = line.contains("\"https://") || line.contains("'https://");
            
            if has_http && !has_https {
                // Check if it's localhost (allowed)
                let is_localhost = line.contains("localhost") || 
                                   line.contains("127.0.0.1") || 
                                   line.contains("0.0.0.0");
                if !is_localhost {
                    issues.push(Issue::new(
                        "PY_S5332",
                        format!("Clear-text HTTP URL detected - use HTTPS instead"),
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        "Replace http:// with https:// to encrypt data in transit."
                    )));
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
    fn test_s5332_registered() {
        let rule = PY_S5332Rule::new();
        assert_eq!(rule.id(), "PY_S5332");
    }

    #[test]
    fn test_s5332_detects_http_url() {
        let rule = PY_S5332Rule::new();
        let smelly = r#"
requests.get("http://api.example.com/data")
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect HTTP URL");
        assert_eq!(issues[0].rule_id, "PY_S5332");
    }

    #[test]
    fn test_s5332_allows_https() {
        let rule = PY_S5332Rule::new();
        let clean = r#"
requests.get("https://api.example.com/data")
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag HTTPS URLs");
    }

    #[test]
    fn test_s5332_allows_localhost() {
        let rule = PY_S5332Rule::new();
        let clean = r#"
requests.get("http://localhost:8080/api")
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag localhost URLs");
    }
}
