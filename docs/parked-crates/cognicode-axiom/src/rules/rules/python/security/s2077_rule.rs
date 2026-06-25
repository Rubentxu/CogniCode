//! S2077 — SQL injection via f-strings
//!
//! Detects SQL queries built using f-strings which can enable SQL injection attacks.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2077"
    name: "SQL queries should not be built using string formatting"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Building SQL queries with f-strings or string concatenation allows attackers to inject malicious SQL code, potentially leading to data breaches or data loss.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect f-strings containing SQL keywords (case-insensitive)
        let re = regex::Regex::new(r#"(?i)f["'][^"']*(SELECT|INSERT|UPDATE|DELETE|DROP|ALTER|CREATE|GRANT|REVOKE)"#).unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if re.is_match(line) {
                issues.push(Issue::new(
                    "PY_S2077",
                    format!("Possible SQL injection: f-string contains SQL keyword"),
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Use parameterized queries or an ORM instead of string formatting for SQL queries."
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
    fn test_s2077_registered() {
        let rule = PY_S2077Rule::new();
        assert_eq!(rule.id(), "PY_S2077");
    }

    #[test]
    fn test_s2077_detects_sql_injection() {
        let rule = PY_S2077Rule::new();
        let smelly = r#"
query = f"SELECT * FROM users WHERE name = '{username}'"
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect SQL injection via f-string");
        assert_eq!(issues[0].rule_id, "PY_S2077");
    }

    #[test]
    fn test_s2077_allows_safe_code() {
        let rule = PY_S2077Rule::new();
        let clean = r#"
# Safe parameterized query
cursor.execute("SELECT * FROM users WHERE id = ?", (user_id,))
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag parameterized queries");
    }
}
