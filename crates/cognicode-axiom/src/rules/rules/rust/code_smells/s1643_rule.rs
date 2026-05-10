//! S1643 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1643"
    name: "String concatenation in loops should use collect() or push_str"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "String concatenation with + in loops is inefficient due to repeated allocations; push_str or iterator methods should be used instead.",
    clean_code: Efficient,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let mut in_loop = false;
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("for ") || line.contains("while ") || line.contains("loop {") {
                in_loop = true;
            }
            if in_loop
                && line.contains("+=") && (line.contains("String") || line.contains("\"") || line.contains("to_string") || line.contains("to_owned")) && !line.contains("push_str") {
                    issues.push(Issue::new(
                        "S1643",
                        "String concatenation in loop - use .push_str() or collect()",
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            if line.trim() == "}" {
                in_loop = false;
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_s1643_registered() {
        let rule=S1643Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
