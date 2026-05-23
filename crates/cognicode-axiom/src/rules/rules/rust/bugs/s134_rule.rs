//! S134 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S134"
    name: "Control flow statements should not be nested too deeply"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { threshold: usize = 3 }

    explanation: "Deeply nested control flow structures reduce code readability and maintainability, making it harder to understand program logic and increasing the risk of introducing bugs during modifications.",
    clean_code: Focused,
    impacts: [Maintainability: Medium, Reliability: Low],
    check: => {
        let mut issues = Vec::new();
        let func_nodes = ctx.query_functions();
        for node in func_nodes {
            let depth = ctx.nesting_depth(node);
            if depth >= self.threshold {
                let pt = node.start_position();
                if let Some(name) = ctx.function_name(node) {
                    issues.push(Issue::new(
                        "S134",
                        format!("Function '{}' has nesting depth {} exceeding threshold {}", name, depth, self.threshold),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        pt.row + 1,
                    ).with_column(pt.column)
                    .with_remediation(Remediation::moderate(
                        "Extract nested logic into separate functions or use early returns"
                    )));
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
    fn test_s134_registered() {
        let rule = S134Rule::new();
        assert_eq!(rule.id(), "S134");
        assert!(rule.name().len() > 0);
    }
}
