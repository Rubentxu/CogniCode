//! S5122 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

declare_rule! {
    id: "S5122"
    name: "SQL injection vulnerabilities should be prevented"
    severity: Blocker
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "SQL injection allows attackers to manipulate database queries through unsanitized input, potentially leading to data theft, corruption, or unauthorized system access.",
    clean_code: Trustworthy,
    impacts: [Security: High, Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let sql_keywords = ["SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "EXEC", "EXECUTE", "UNION", "INTO", "OUTFILE", "INFILE", "LOAD_FILE", "BENCHMARK", "SLEEP"];

        // Look for format! macro invocations containing SQL keywords
        // Pattern: format! followed by a string literal that contains SQL keywords
        // Only flag if used in database context
        let db_keywords = ["query", "execute", "sql", "prepare", "run", "raw_query", "execute_query"];
        
        for (line_idx, line) in ctx.source.lines().enumerate() {
            // Check if this line has a format! macro
            if line.contains("format!") || line.contains("format_args!") {
                // Check if this or nearby lines contain database-related function calls
                let has_db_context = (0..=2.min(line_idx))
                    .any(|i| ctx.source.lines().nth(line_idx.saturating_sub(i))
                        .map(|l| db_keywords.iter().any(|kw| l.to_lowercase().contains(kw)))
                        .unwrap_or(false));
                
                if !has_db_context {
                    continue;
                }
                // Simple approach: find format!(" or format_args!(" and extract until the closing quote
                let line_upper = line.to_uppercase();
                for keyword in &sql_keywords {
                    let kw_upper = keyword.to_uppercase();
                    // Check if the line contains the SQL keyword
                    if line_upper.contains(&kw_upper) {
                        // Found SQL keyword - report it
                        // We report at the position of the keyword in the original line
                        if let Some(kw_pos) = line_upper.find(&kw_upper) {
                            issues.push(Issue::new(
                                "S5122",
                                format!(
                                    "Potential SQL injection: SQL keyword '{}' found in format! string",
                                    keyword
                                ),
                                Severity::Blocker,
                                Category::Vulnerability,
                                ctx.file_path,
                                line_idx + 1,
                            ).with_column(kw_pos + 1)
                            .with_remediation(Remediation::substantial(
                                "Use parameterized queries instead of string interpolation"
                            )));
                            break;
                        }
                    }
                }
            }
        }

        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s5122_registered() {
        let rule = S5122Rule::new();
        assert_eq!(rule.id(), "S5122");
        assert!(rule.name().len() > 0);
    }
}
