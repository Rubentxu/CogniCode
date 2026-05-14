//! Auto-generated from KB: S6245
//! Languages: rust
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use regex;

declare_rule! {
    id: "S6245"
    name: "JWT None Algorithm"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "Use HMAC or RSA with proper algorithm, not 'none'"

    impacts: [Security: Medium]

    check: => {
        let mut issues = Vec::new();
        if let Some(re) = regex::Regex::new(r##"(?i)(jwt|json.web.token).*algorithm.*none"##).ok() {
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
    fn test_S6245_registered() {
        let rule = S6245Rule::new();
        assert_eq!(rule.id(), "S6245");
        assert!(!rule.name().is_empty());
    }
}
