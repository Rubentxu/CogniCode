//! S1226 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1226"
    name: "Method parameters should not be reassigned"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Reassigning method parameters can confuse callers about whether the original value is used and violates the principle of least surprise.",
    clean_code: Clear,
    impacts: [Maintainability: Medium, Reliability: Low],
    check: => {
        let mut issues = Vec::new();
        // Find function parameters and check if they appear on LHS of assignments
        let query_str = "(function_item parameters: (parameters (parameter pattern: (identifier) @param))) @func";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            let mut params: std::collections::HashSet<String> = std::collections::HashSet::new();
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    if let Ok(name) = capture.node.utf8_text(ctx.source.as_bytes()) {
                        params.insert(name.to_string());
                    }
                }
            }
            // Check for assignment patterns where LHS matches a param name
            let assign_query = "(assignment left: (identifier) @var)";
            if let Ok(query2) = tree_sitter::Query::new(&ctx.language.to_ts_language(), assign_query) {
                let mut cursor2 = tree_sitter::QueryCursor::new();
                let mut matches2 = cursor2.matches(&query2, ctx.tree.root_node(), ctx.source.as_bytes());
                while let Some(m) = matches2.next() {
                    for capture in m.captures {
                        if let Ok(name) = capture.node.utf8_text(ctx.source.as_bytes())
                            && params.contains(name) {
                                let pt = capture.node.start_position();
                                issues.push(Issue::new(
                                    "S1226",
                                    format!("Parameter '{}' should not be reassigned", name),
                                    Severity::Major, Category::CodeSmell, ctx.file_path, pt.row + 1,
                                ).with_remediation(Remediation::moderate(
                                    "Use a new local variable instead of reassigning the parameter"
                                )));
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
    fn test_s1226_registered() {
        let rule=S1226Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
