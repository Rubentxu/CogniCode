//! Auto-generated from KB: S4817
//! Languages: rust
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use regex;

declare_rule! {
    id: "S4817"
    name: "XPath Injection"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "Sanitize user input before XPath queries"

    impacts: [Security: Medium]

    check: => {
        let mut issues = Vec::new();
        if let Some(re) = regex::Regex::new(r##"(?i)(xpath|xml\.etree|elementtree).*(user|input|req)"##).ok() {
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
    fn test_S4817_registered() {
        let rule = S4817Rule::new();
        assert_eq!(rule.id(), "S4817");
        assert!(!rule.name().is_empty());
    }
}
