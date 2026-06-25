//! S3649 — SQL via string concatenation
//!
//! Detects SQL queries built using string concatenation which can enable SQL injection.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S3649"
    name: "SQL queries should not be built using string concatenation"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Building SQL queries with string concatenation allows attackers to inject malicious SQL code, potentially leading to data breaches or data loss.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect string concatenation with + in SQL context
        // Looking for patterns like "string" + variable
        let sql_concat_re = regex::Regex::new(r#"["']\s*\+"#).unwrap();
        let sql_keywords = ["SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "FROM", "WHERE"];
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            let upper_line = line.to_uppercase();
            // Check if line has SQL keywords
            let has_sql = sql_keywords.iter().any(|kw| upper_line.contains(kw));
            // Check if line has string concatenation (quote followed by +)
            if has_sql && sql_concat_re.is_match(line) {
                issues.push(Issue::new(
                    "PY_S3649",
                    format!("SQL query built using string concatenation - potential SQL injection"),
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Use parameterized queries or an ORM instead of string concatenation for SQL queries."
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
    fn test_s3649_registered() {
        let rule = PY_S3649Rule::new();
        assert_eq!(rule.id(), "PY_S3649");
    }

    #[test]
    fn test_s3649_detects_sql_concat() {
        let rule = PY_S3649Rule::new();
        let smelly = r#"
query = "SELECT * FROM users WHERE id=" + user_id
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect SQL string concatenation");
        assert_eq!(issues[0].rule_id, "PY_S3649");
    }

    #[test]
    fn test_s3649_allows_parameterized() {
        let rule = PY_S3649Rule::new();
        let clean = r#"
cursor.execute("SELECT * FROM users WHERE id = ?", (user_id,))
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag parameterized queries");
    }
}
