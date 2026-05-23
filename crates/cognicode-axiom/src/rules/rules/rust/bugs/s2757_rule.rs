//! S2757 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

declare_rule! {
    id: "S2757"
    name: "Unexpected assignment operators in conditions"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Pattern matches in conditions that look like assignments can confuse developers and lead to unintended behavior due to the difference between = and ==.",
    clean_code: Logical,
    impacts: [Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"if\s+let\s+[[:alpha:]]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue; // Skip comments and disabled code
            }
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S2757",
                    "Potentially unintended pattern match in condition",
                    Severity::Major,
                    Category::Bug,
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
    fn test_s2757_registered() {
        let rule = S2757Rule::new();
        assert_eq!(rule.id(), "S2757");
        assert!(rule.name().len() > 0);
    }
}
