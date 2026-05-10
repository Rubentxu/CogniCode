//! S1541 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1541"
    name: "Functions should not have too many branches"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { threshold: usize = 15 }

    explanation: "Functions with high cyclomatic complexity are difficult to test thoroughly and maintain, often indicating the need for refactoring into smaller functions.",
    clean_code: Focused,
    impacts: [Maintainability: Medium, Reliability: Low],
    check: => {
        let mut issues = Vec::new();
        for func_node in ctx.query_functions() {
            let mut branch_count = 0;
            crate::rules::helpers::count_branches_impl(func_node, &mut branch_count);
            if branch_count > self.threshold {
                let pt = func_node.start_position();
                if let Some(name) = ctx.function_name(func_node) {
                    issues.push(Issue::new("S1541", format!("Function '{}' has {} branches", name, branch_count), Severity::Major, Category::CodeSmell, ctx.file_path, pt.row + 1));
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
    fn test_s1541_registered() {
        let rule=S1541Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
