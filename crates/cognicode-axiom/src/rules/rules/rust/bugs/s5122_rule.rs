//! S5122 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

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
        let sql_keyword_patterns: Vec<_> = sql_keywords.iter()
            .map(|keyword| {
                let pattern = format!(r"(?i)(?<![a-zA-Z_]){}(?![a-zA-Z_])", regex::escape(keyword));
                regex::Regex::new(&pattern)
            })
            .collect();
        
        let query = match tree_sitter::Query::new(
            &ctx.language.to_ts_language(),
            "(macro_invocation (identifier) @macro_name (token_tree) @args)"
        ) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                if cap.node.kind() == "identifier"
                    && let Ok(macro_name) = cap.node.utf8_text(ctx.source.as_bytes())
                        && (macro_name == "format" || macro_name == "format_args")
                            && let Some(args_node) = m.captures.iter().find(|c| c.node.kind() == "token_tree")
                                && let Ok(args_text) = args_node.node.utf8_text(ctx.source.as_bytes()) {
                                    let args_upper = args_text.to_uppercase();
                                    let format_arg_count = args_text.matches("{}").count();
                                    for (keyword, pattern) in sql_keywords.iter().zip(sql_keyword_patterns.iter()) {
                                        if pattern
                                            .as_ref()
                                            .and_then(|re| Ok(re.is_match(args_text)))
                                            .unwrap_or(false)
                                            && format_arg_count >= 1 {
                                            let pt = cap.node.start_position();
                                            issues.push(Issue::new(
                                                "S5122",
                                                format!(
                                                    "Potential SQL injection: SQL keyword '{}' found in format! string with {} dynamic argument(s)",
                                                    keyword,
                                                    format_arg_count
                                                ),
                                                Severity::Blocker,
                                                Category::Vulnerability,
                                                ctx.file_path,
                                                pt.row + 1,
                                            ).with_column(pt.column + 1)
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
        let rule=S5122Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
