//! Auto-generated from KB: S5730
//! Languages: rust
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use regex;

declare_rule! {
    id: "S5730"
    name: "Mixed Content"
    severity: Major
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "Use HTTPS for all resources"

    impacts: [Security: Medium]

    check: => {
        let mut issues = Vec::new();
        if let Some(re) = regex::Regex::new(r##"(?i)http://(?!127\.0\.0\.1|localhost)"##).ok() {
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
    fn test_S5730_registered() {
        let rule = S5730Rule::new();
        assert_eq!(rule.id(), "S5730");
        assert!(!rule.name().is_empty());
    }
}
