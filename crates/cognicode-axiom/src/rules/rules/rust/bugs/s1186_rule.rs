//! S1186 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1186"
    name: "Empty functions should be completed or removed"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Empty function bodies that are not placeholders waste developer time investigating non-functional code and may indicate incomplete implementation.",
    clean_code: Complete,
    impacts: [Maintainability: Medium, Reliability: Low],
    check: => {
        let mut issues = Vec::new();
        let node_type = ctx.language.function_node_type();
        let query_str = format!("({} body: (block) @body) @func", node_type);
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), &query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let node = capture.node;
                    // Check if the block body has no meaningful children (only comment/doc nodes)
                    let named_children = node.named_child_count();
                    if named_children == 0
                        && let Some(name) = ctx.function_name(node.parent().unwrap_or(node)) {
                            let pt = node.start_position();
                            issues.push(Issue::new(
                                "S1186",
                                format!("Function '{}' has an empty body", name),
                                Severity::Major, Category::CodeSmell, ctx.file_path, pt.row + 1,
                            ).with_remediation(Remediation::quick(
                                "Implement the function body or remove it if not needed"
                            )));
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
    fn test_s1186_registered() {
        let rule=S1186Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
