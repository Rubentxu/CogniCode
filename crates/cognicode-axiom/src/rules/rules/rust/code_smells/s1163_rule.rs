//! S1163 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1163"
    name: "Redundant else after return, break, or continue"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Redundant else blocks after return/break/continue add unnecessary nesting and reduce code clarity.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for i in 0..lines.len().saturating_sub(1) {
            let prev = lines[i].trim();
            let next = lines[i+1].trim();
            if (prev.ends_with("return;") || prev.ends_with("break;") || prev.ends_with("continue;")) && next.starts_with("else ") && !prev.is_empty() && !prev.starts_with("//") {
                issues.push(Issue::new(
                    "S1163",
                    "Redundant else after control flow statement",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    i + 2,
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
    fn test_s1163_registered() {
        let rule=S1163Rule::new();
        assert_eq!(rule.id(), "S1163");
        assert!(rule.name().len()>0);
    }
}
