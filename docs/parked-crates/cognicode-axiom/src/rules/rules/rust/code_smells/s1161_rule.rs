//! S1161 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1161"
    name: "Deprecated code should not be used"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "#[allow(deprecated)] suppresses warnings about using deprecated APIs, preventing developers from migrating to supported alternatives.",
    clean_code: Complete,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("#[allow(deprecated)]") {
                issues.push(Issue::new(
                    "S1161",
                    "#[allow(deprecated)] suppresses useful warnings about deprecated API usage",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Remove the allow(deprecated) attribute and update deprecated code")));
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_s1161_registered() {
        let rule=S1161Rule::new();
        assert_eq!(rule.id(), "S1161");
        assert!(rule.name().len()>0);
    }
}
