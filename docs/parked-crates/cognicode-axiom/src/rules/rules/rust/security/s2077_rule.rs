//! S2077 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

declare_rule! {
    id: "S2077"
    name: "SQL queries should not be built with string interpolation"
    severity: Blocker
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "SQL queries built with string interpolation are vulnerable to injection attacks when user input is included without proper sanitization.",
    clean_code: Trustworthy,
    impacts: [Security: High, Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let sql_keywords = ["SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER"];
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("///")
            || trimmed.starts_with("//!") || trimmed.starts_with("/*") || trimmed.starts_with("*")
            || trimmed.starts_with("#") { continue; }
            let has_sql = sql_keywords.iter().any(|kw| line.contains(kw));
            let has_format = line.contains("format!");
            if has_sql && has_format && !line.contains("bind(") && !line.contains("$") && !line.contains("?") && !line.contains("prepared") {
                issues.push(Issue::new(
                    "S2077",
                    "SQL query built with string interpolation - use parameterized queries",
                    Severity::Blocker,
                    Category::Vulnerability,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate("Use prepared statements or an ORM with parameter binding")));
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s2077_registered() {
        let rule = S2077Rule::new();
        assert_eq!(rule.id(), "S2077");
        assert!(rule.name().len() > 0);
    }
}
