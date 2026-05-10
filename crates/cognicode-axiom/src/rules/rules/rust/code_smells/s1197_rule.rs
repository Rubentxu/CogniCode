//! S1197 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1197"
    name: "Magic numbers should be replaced by named constants"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Magic numbers without context make code harder to understand and maintain, as their meaning and origin are not immediately clear.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"[=<>!]\s*\d{4,}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("const") && !line.contains("test") && !line.contains("\"") {
                issues.push(Issue::new(
                    "S1197",
                    "Magic number detected - use a named constant",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_s1197_registered() {
        let rule=S1197Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
