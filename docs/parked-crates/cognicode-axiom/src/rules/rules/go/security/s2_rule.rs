//! S2 — SQL injection via fmt.Sprintf
//!
//! Detects potential SQL injection vulnerabilities via fmt.Sprintf.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S2077"
    name: "SQL injection vulnerability via fmt.Sprintf"
    severity: Critical
    category: Vulnerability
    language: "Go"
    params: {}

    explanation: "Using fmt.Sprintf to build SQL queries allows SQL injection attacks. Use parameterized queries instead.",
    clean_code: Clear,
    impacts: [Security: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find fmt.Sprintf with SQL keywords
        let sql_pattern = regex::Regex::new(r#"fmt\.Sprintf\(["'][^"']*(?:SELECT|INSERT|UPDATE|DELETE|FROM|WHERE|INSERT INTO|UPDATE SET|DELETE FROM)[^"']*["']"#).unwrap();

        for cap in sql_pattern.captures_iter(source) {
            let match_start = cap.get(0).unwrap().start();
            let line_num = source[..match_start].lines().count() + 1;
            issues.push(Issue::new(
                "GO_S2077",
                format!("Potential SQL injection: fmt.Sprintf used with SQL query"),
                Severity::Critical,
                Category::Vulnerability,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Use parameterized queries or a SQL builder library instead of string formatting"
            )));
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;

    fn with_go_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Go.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Go,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_s2_registered() {
        let rule = GO_S2077Rule::new();
        assert_eq!(rule.id(), "GO_S2077");
    }

    #[test]
    fn test_s2_detects_sql_injection() {
        let rule = GO_S2077Rule::new();
        let smelly = r#"
query := fmt.Sprintf("SELECT * FROM users WHERE id = '%s'", userId)
"#;
        let issues = with_go_context(smelly, "db.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect SQL injection via fmt.Sprintf");
        assert_eq!(issues[0].rule_id, "GO_S2077");
    }

    #[test]
    fn test_s2_allows_safe_query() {
        let rule = GO_S2077Rule::new();
        let clean = r#"
query := "SELECT * FROM users WHERE id = ?"
db.Query(query, userId)
"#;
        let issues = with_go_context(clean, "db.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag parameterized queries");
    }
}
