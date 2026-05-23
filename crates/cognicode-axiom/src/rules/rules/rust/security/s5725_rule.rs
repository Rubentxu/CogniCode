//! Auto-generated from KB: S5725
//! Languages: rust
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use regex;

declare_rule! {
    id: "S5725"
    name: "CSP Header Missing"
    severity: Major
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "Define Content-Security-Policy header"

    impacts: [Security: Medium]

    check: => {
        let mut issues = Vec::new();
        if let Some(re) = regex::Regex::new(r##"(?i)content-security-policy"##).ok() {
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
    fn test_S5725_registered() {
        let rule = S5725Rule::new();
        assert_eq!(rule.id(), "S5725");
        assert!(!rule.name().is_empty());
    }
}
