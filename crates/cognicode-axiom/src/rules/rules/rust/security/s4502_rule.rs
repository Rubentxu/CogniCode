//! Auto-generated from KB: S4502
//! Languages: rust
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use regex;

declare_rule! {
    id: "S4502"
    name: "CSRF Token Check"
    severity: Major
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "Validate CSRF tokens on state-changing requests"

    impacts: [Security: High]

    check: => {
        let mut issues = Vec::new();
        if let Some(re) = regex::Regex::new(r##"(?i)(csrf|csrf_token|xsrf|anti_csrf)"##).ok() {
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
    fn test_S4502_registered() {
        let rule = S4502Rule::new();
        assert_eq!(rule.id(), "S4502");
        assert!(!rule.name().is_empty());
    }
}
