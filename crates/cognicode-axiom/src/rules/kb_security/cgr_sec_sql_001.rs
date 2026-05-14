//! CGR_SEC_SQL_001 — SQL Injection via String Interpolation/Concatenation
//! Detects SQL queries built by interpolating or concatenating strings, allowing attackers
//! to manipulate query logic (CWE-89).
//!
//! Languages: python, java, rust
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_SEC_SQL_001"
    name: "SQL injection vulnerability — string interpolation or concatenation"
    severity: Critical
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "SQL query constructed by interpolating or concatenating strings that may contain user-controlled data. Attackers can modify query logic or access unauthorized data."

    clean_code: Trustworthy,
    impacts: [Security: High, Reliability: Medium],

    check: => {
        let mut issues = Vec::new();

        // Python: cursor.execute(f"SELECT * FROM .. WHERE id={user_id}")
        // Also: "SELECT * FROM .. WHERE id=" + user_id
        for qm in ctx.query_captures(
            "(call_expression \
              function: (attribute \
                object: (identifier) @obj \
                attr: (identifier) @method) \
              arguments: (arguments (string) @sql_str) @call)"
        ) {
            let method_text = qm.get("method")
                .map(|m| m.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let sql_text = qm.get("sql_str")
                .map(|s| s.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            // Check if it's a SQL execution method
            let is_sql_exec = method_text == "execute"
                || method_text == "executemany"
                || method_text == "executescript"
                || method_text == "query"
                || method_text == "raw";

            if !is_sql_exec {
                continue;
            }

            // Check if string contains interpolation or concatenation indicators
            let has_interpolation = sql_text.contains("${")
                || sql_text.contains("{")
                || sql_text.contains("%(")
                || sql_text.contains("+")
                || sql_text.contains("'\"'"); // suspicious quote mixing

            if has_interpolation {
                let start = qm.get("call")
                    .map(|n| n.start_position())
                    .unwrap_or_default();
                issues.push(Issue::from_node(
                    "CGR_SEC_SQL_001",
                    format!("SQL query built with string interpolation/concatenation. User input may reach SQL execution."),
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    qm.get("call").unwrap_or_else(|| qm.get("sql_str").unwrap()),
                ).with_remediation(Remediation::moderate(
                    "Use parameterized queries: cursor.execute(\"SELECT * FROM users WHERE id=%s\", (user_id,))"
                )).with_bad_example(
                    "cursor.execute(f\"SELECT * FROM users WHERE id={user_id}\")"
                ).with_good_example(
                    "cursor.execute(\"SELECT * FROM users WHERE id=%s\", (user_id,))"
                ));
            }
        }

        // Also detect string concatenation patterns: "SELECT ... " + var
        for node in ctx.query_nodes("(binary_expression)") {
            let bin_text = node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
            // Binary expression with + where one side looks like SQL keyword
            if bin_text.contains("SELECT")
                || bin_text.contains("INSERT")
                || bin_text.contains("UPDATE")
                || bin_text.contains("DELETE")
                || bin_text.contains("DROP")
            {
                let start = node.start_position();
                issues.push(Issue::new(
                    "CGR_SEC_SQL_001",
                    "SQL query built by concatenating strings. User input may be present.",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    start.row + 1,
                ).with_remediation(Remediation::moderate(
                    "Use parameterized queries instead of string concatenation."
                )));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_SEC_SQL_001Rule::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_sec_sql_001_registered() {
        let rule = CGR_SEC_SQL_001Rule::new();
        assert_eq!(rule.id(), "CGR_SEC_SQL_001");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Critical);
        assert_eq!(rule.category(), Category::Vulnerability);
    }
}
