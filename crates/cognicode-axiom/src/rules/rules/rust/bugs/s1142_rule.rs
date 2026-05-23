//! S1142 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1142"
    name: "Functions should not have too many return points"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { max_returns: usize = 3 }

    explanation: "Functions with too many return points are harder to understand and trace through, increasing the risk of logic errors during maintenance.",
    clean_code: Clear,
    impacts: [Maintainability: Medium, Reliability: Low],
    check: => {
        let mut issues = Vec::new();
        for func_node in ctx.query_functions() {
            let text = func_node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
            let return_count = text.matches("return ").count() + text.matches("return;").count();
            if return_count > self.max_returns {
                let pt = func_node.start_position();
                if let Some(name) = ctx.function_name(func_node) {
                    issues.push(Issue::new(
                        "S1142",
                        format!("Function '{}' has {} return statements", name, return_count),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        pt.row + 1,
                    ));
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
    fn test_s1142_registered() {
        let rule=S1142Rule::new();
        assert_eq!(rule.id(), "S1142");
        assert!(rule.name().len()>0);
    }
}
