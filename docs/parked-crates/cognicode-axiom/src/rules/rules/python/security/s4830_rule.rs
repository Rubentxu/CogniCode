//! S4830 — SSL verification disabled
//!
//! Detects requests with SSL verification disabled (verify=False).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S4830"
    name: "SSL certificate verification should not be disabled"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Disabling SSL certificate verification removes the protection against man-in-the-middle attacks, allowing attackers to intercept and modify encrypted traffic.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect verify=False in requests calls
        let re = regex::Regex::new(r"verify\s*=\s*False").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if re.is_match(line) {
                issues.push(Issue::new(
                    "PY_S4830",
                    format!("SSL verification disabled - vulnerable to MITM attacks"),
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Set verify=True or provide the path to the CA bundle certificate."
                )));
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
    fn test_s4830_registered() {
        let rule = PY_S4830Rule::new();
        assert_eq!(rule.id(), "PY_S4830");
    }

    #[test]
    fn test_s4830_detects_verify_false() {
        let rule = PY_S4830Rule::new();
        let smelly = r#"
requests.get("https://api.example.com", verify=False)
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect verify=False");
        assert_eq!(issues[0].rule_id, "PY_S4830");
    }

    #[test]
    fn test_s4830_allows_verified_requests() {
        let rule = PY_S4830Rule::new();
        let clean = r#"
response = requests.get("https://api.example.com", verify=True)
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag verified requests");
    }
}
