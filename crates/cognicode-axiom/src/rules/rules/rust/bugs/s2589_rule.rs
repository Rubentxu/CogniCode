//! S2589 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S2589"
    name: "Boolean expressions should not be constant"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Constant boolean expressions in conditions always evaluate to the same result, indicating dead code that should be removed or replaced with meaningful logic.",
    clean_code: Logical,
    impacts: [Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            let const_bool_re = regex::Regex::new(r"(if|while)\s*\(?\s*(true|false)\s*\)?").unwrap();
        if const_bool_re.is_match(trimmed) {
                issues.push(Issue::new(
                    "S2589",
                    format!("Constant boolean expression at line {}", idx + 1),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Remove the redundant condition or use a meaningful expression")));
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_s2589_registered() {
        let rule=S2589Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
