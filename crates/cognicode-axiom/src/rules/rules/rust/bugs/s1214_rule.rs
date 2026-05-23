//! S1214 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1214"
    name: "Mutable static variables should not be used"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "static mut is inherently unsafe in Rust as it allows data races; interior mutability patterns like OnceCell or Mutex should be used instead.",
    clean_code: Logical,
    impacts: [Reliability: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with("//") && !trimmed.starts_with("/*") && line.contains("static mut") {
                issues.push(Issue::new(
                    "S1214",
                    "static mut is unsafe - use OnceCell, Lazy, or interior mutability",
                    Severity::Critical,
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
    fn test_s1214_registered() {
        let rule=S1214Rule::new();
        assert_eq!(rule.id(), "S1214");
        assert!(rule.name().len()>0);
    }
}
