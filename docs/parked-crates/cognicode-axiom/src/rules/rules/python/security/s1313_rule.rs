//! S1313 — Hardcoded IP addresses
//!
//! Detects hardcoded IP addresses in source code, particularly private network ranges.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1313"
    name: "Hardcoded IP addresses should not be used"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Hardcoded IP addresses make code inflexible and can expose internal network configuration. Use configuration files or environment variables instead.",
    clean_code: Clear,
    impacts: [Security: Low, Maintainability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect private IP ranges: 192.168.x.x, 10.x.x.x, 172.16-31.x.x
        let ip_patterns = [
            r"\b192\.168\.\d{1,3}\.\d{1,3}\b",
            r"\b10\.\d{1,3}\.\d{1,3}\.\d{1,3}\b",
            r"\b172\.(1[6-9]|2[0-9]|3[0-1])\.\d{1,3}\.\d{1,3}\b",
        ];
        let re = ip_patterns.iter()
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
                        "PY_S1313",
                        "Hardcoded IP address detected - use configuration instead",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        "Move the IP address to a configuration file or environment variable."
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
    fn test_s1313_registered() {
        let rule = PY_S1313Rule::new();
        assert_eq!(rule.id(), "PY_S1313");
    }

    #[test]
    fn test_s1313_detects_192_168() {
        let rule = PY_S1313Rule::new();
        let smelly = r#"
server = "192.168.1.100"
"#;
        let issues = with_python_context(smelly, "config.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect 192.168.x.x addresses");
        assert_eq!(issues[0].rule_id, "PY_S1313");
    }

    #[test]
    fn test_s1313_detects_10_x() {
        let rule = PY_S1313Rule::new();
        let smelly = r#"
db_host = "10.0.0.50"
"#;
        let issues = with_python_context(smelly, "config.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect 10.x.x.x addresses");
    }

    #[test]
    fn test_s1313_detects_172_16_31() {
        let rule = PY_S1313Rule::new();
        let smelly = r#"
api_endpoint = "172.20.0.1:8080"
"#;
        let issues = with_python_context(smelly, "config.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect 172.16-31.x.x addresses");
    }

    #[test]
    fn test_s1313_allows_public_ips() {
        let rule = PY_S1313Rule::new();
        let clean = r#"
server = "8.8.8.8"
public_api = "93.184.216.34"
"#;
        let issues = with_python_context(clean, "config.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag public IP addresses");
    }
}
