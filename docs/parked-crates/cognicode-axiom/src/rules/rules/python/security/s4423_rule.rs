//! S4423 — Weak TLS configuration
//!
//! Detects weak TLS/SSL configurations that use outdated protocols.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S4423"
    name: "TLS configurations should not use outdated protocols"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Using outdated TLS protocols (SSLv2, SSLv3, TLSv1, TLSv1.0, TLSv1.1) makes the connection vulnerable to attacks like POODLE and BEAST.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect weak SSL/TLS protocols
        let weak_protocols = [
            r"ssl\.PROTOCOL_SSLv2\b",
            r"ssl\.PROTOCOL_SSLv3\b", 
            r"ssl\.PROTOCOL_TLSv1\b",
            r"PROTOCOL_TLSv1_0\b",
            r"PROTOCOL_TLSv1_1\b",
            r"TLSv1\s*=\s*0\s*$",
            r"TLSv1_1\s*=\s*0\s*$",
        ];
        let re = weak_protocols.iter()
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
                        "PY_S4423",
                        format!("Outdated TLS protocol detected - use TLSv1.2 or higher"),
                        Severity::Critical,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::moderate(
                        "Use ssl.PROTOCOL_TLSv1_2 or ssl.PROTOCOL_TLSv1_3 to enforce strong TLS."
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
    fn test_s4423_registered() {
        let rule = PY_S4423Rule::new();
        assert_eq!(rule.id(), "PY_S4423");
    }

    #[test]
    fn test_s4423_detects_weak_tls() {
        let rule = PY_S4423Rule::new();
        let smelly = r#"
import ssl
context = ssl.SSLContext(ssl.PROTOCOL_TLSv1)
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect weak TLS protocol");
        assert_eq!(issues[0].rule_id, "PY_S4423");
    }

    #[test]
    fn test_s4423_allows_strong_tls() {
        let rule = PY_S4423Rule::new();
        let clean = r#"
import ssl
context = ssl.SSLContext(ssl.PROTOCOL_TLSv1_2)
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag TLSv1.2 or higher");
    }
}
