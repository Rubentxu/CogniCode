//! S1244 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1244"
    name: "Floating point equality should not be used"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Floating point equality comparisons can fail due to precision issues, producing unexpected results in numeric comparisons.",
    clean_code: Logical,
    impacts: [Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(f32|f64)\b.*\s*==\s*").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1244",
                    "Floating point equality comparison - may not behave as expected",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Use epsilon comparison: (a - b).abs() < EPSILON")));
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_s1244_registered() {
        let rule=S1244Rule::new();
        assert_eq!(rule.id(), "S1244");
        assert!(rule.name().len()>0);
    }
}
