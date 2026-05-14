//! Auto-generated from KB: S5736
//! Languages: rust
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use regex;

declare_rule! {
    id: "S5736"
    name: "X-Content-Type-Options Missing"
    severity: Minor
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "Add X-Content-Type-Options: nosniff"

    impacts: [Security: Medium]

    check: => {
        let mut issues = Vec::new();
        if let Some(re) = regex::Regex::new(r##"(?i)x-content-type-options"##).ok() {
            for m in re.find_iter(ctx.source) {
                let line_number = ctx.source[..m.start()].lines().count() + 1;
                issues.push(Issue::new(
                    self.id(),
                    "Security issue detected",
                    self.severity(),
                    self.category(),
                    ctx.file_path,
                    line_number,
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
    fn test_S5736_registered() {
        let rule = S5736Rule::new();
        assert_eq!(rule.id(), "S5736");
        assert!(!rule.name().is_empty());
    }
}
