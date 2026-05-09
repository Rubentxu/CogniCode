//! S107 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S107"
    name: "Functions should not have too many parameters"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { threshold: usize = 4 }

    explanation: "Functions with too many parameters are difficult to call, test, and remember, often indicating the need for parameter grouping into structs or configuration objects.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        // Query for function definitions to count parameters
        let query_str = "(function_item parameters: (parameters) @params)";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    // Count named children (parameters) inside the parameters node
                    let params_node = capture.node;
                    let param_count = params_node.named_child_count();
                    if param_count > self.threshold {
                        let pt = params_node.start_position();
                        // Try to get the function name from the parent node
                        let func_name = params_node.parent()
                            .and_then(|p| ctx.function_name(p))
                            .unwrap_or("anonymous");
                        issues.push(Issue::new(
                            "S107",
                            format!("Function '{}' has {} parameters exceeding threshold {}", func_name, param_count, self.threshold),
                            Severity::Major,
                            Category::CodeSmell,
                            ctx.file_path,
                            pt.row + 1,
                        ).with_column(pt.column)
                        .with_remediation(Remediation::moderate(
                            "Consider grouping related parameters into a struct"
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
    fn test_s107_registered() {
        let rule=S107Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
