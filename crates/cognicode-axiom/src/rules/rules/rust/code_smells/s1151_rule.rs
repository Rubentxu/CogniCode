//! S1151 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1151"
    name: "Match arms should not be too long"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: { max_lines: usize = 10 }

    explanation: "Match arms that span many lines indicate complex branching logic that could be extracted into separate functions for better readability.",
    clean_code: Focused,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let query_str = "(match_arm) @arm";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let lines = ctx.line_count(capture.node);
                    if lines > self.max_lines {
                        let pt = capture.node.start_position();
                        issues.push(Issue::new(
                            "S1151",
                            format!("Match arm is {} lines - consider extracting to function", lines),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            pt.row + 1,
                        ));
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
    fn test_s1151_registered() {
        let rule=S1151Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
