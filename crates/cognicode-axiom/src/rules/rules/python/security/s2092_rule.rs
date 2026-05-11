//! S2092 — Cookie without Secure
//!
//! Detects cookies created without the Secure flag, allowing transmission over non-HTTPS connections.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2092"
    name: "Cookies should be created with Secure flag"
    severity: Major
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Cookies without the Secure flag can be transmitted over unencrypted HTTP connections, allowing attackers to intercept them via man-in-the-middle attacks.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let cookie_call = regex::Regex::new(r"\bset_cookie\s*\(").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if cookie_call.is_match(line) && !line.contains("secure=True") && !line.contains("secure = True") {
                issues.push(Issue::new(
                    "PY_S2092",
                    "Cookie created without Secure flag - can be sent over HTTP",
                    Severity::Major,
                    Category::Vulnerability,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Add secure=True to the set_cookie() call to ensure cookies are only sent over HTTPS."
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
    fn test_s2092_registered() {
        let rule = PY_S2092Rule::new();
        assert_eq!(rule.id(), "PY_S2092");
    }

    #[test]
    fn test_s2092_detects_cookie_without_secure() {
        let rule = PY_S2092Rule::new();
        let smelly = r#"
response.set_cookie("session_id", token)
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect cookie without Secure flag");
        assert_eq!(issues[0].rule_id, "PY_S2092");
    }

    #[test]
    fn test_s2092_allows_cookie_with_secure() {
        let rule = PY_S2092Rule::new();
        let clean = r#"
response.set_cookie("session_id", token, secure=True)
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag cookie with Secure flag");
    }
}
