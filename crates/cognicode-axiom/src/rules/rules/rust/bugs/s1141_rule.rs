//! S1141 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1141"
    name: "Error handling should not be deeply nested"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Deeply nested error handling with multiple match arms or Result types indicates complex control flow that could be simplified using the ? operator.",
    clean_code: Clear,
    impacts: [Maintainability: Low, Reliability: Low],
    check: => {
        let mut issues = Vec::new();
        let query_str = "(match_expression pattern: (identifier) @pat (#any-of? @pat \"Err\" \"Ok\" \"Some\" \"None\")) @match";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let depth = ctx.nesting_depth(capture.node);
                    if depth > 5 {
                        let pt = capture.node.start_position();
                        issues.push(Issue::new(
                            "S1141",
                            "Deeply nested error handling - consider using ? operator or extracting to function",
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
    fn test_s1141_registered() {
        let rule=S1141Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
