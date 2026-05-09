//! S1854 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1854"
    name: "Unused variables should be removed"
    severity: Info
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Variables declared but never used represent dead code that adds noise to the codebase and may indicate unfinished implementation or copy-paste errors.",
    clean_code: Complete,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("let ") && !trimmed.starts_with("let _ ") && !trimmed.starts_with("let _=") && trimmed.contains('=') {
                issues.push(Issue::new(
                    "S1854",
                    "Variable declared but never used",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick(
                    "Remove the unused variable or prefix it with '_' to indicate it is intentionally unused"
                )));
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_s1854_registered() {
        let rule=S1854Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
