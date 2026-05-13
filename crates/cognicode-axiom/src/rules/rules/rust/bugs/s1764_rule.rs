//! S1764 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

declare_rule! {
    id: "S1764"
    name: "Identical expressions should not be compared"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Comparisons with identical operands always produce the same result, indicating dead code or logical errors in the expression.",
    clean_code: Logical,
    impacts: [Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Check for comparison operators with identical operands (avoid backreferences)
        let comparison_ops = ["==", "!=", ">=", "<=", ">", "<"];
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("// allow") {
                continue;
            }
            for op in &comparison_ops {
                if let Some(pos) = line.find(op)
                    && pos > 0 {
                        let before = line[..pos].trim();
                        let after = line[pos + op.len()..].trim();
                        if before == after && !before.is_empty() && !before.contains('"') && !after.contains('"') {
                            issues.push(Issue::new("S1764", "Identical operands in comparison - always true/false", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                            break;
                        }
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
    fn test_s1764_registered() {
        let rule = S1764Rule::new();
        assert_eq!(rule.id(), "S1764");
        assert!(rule.name().len() > 0);
    }
}
