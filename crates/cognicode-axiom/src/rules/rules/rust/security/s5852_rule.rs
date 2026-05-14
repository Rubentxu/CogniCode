//! Auto-generated from KB: S5852
//! Languages: rust
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use regex;

declare_rule! {
    id: "S5852"
    name: "Regex Catastrophic Backtracking"
    severity: Blocker
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "Use possessive quantifiers or atomic groups"

    impacts: [Security: Medium]

    check: => {
        let mut issues = Vec::new();
        if let Some(re) = regex::Regex::new(r##"\.\*(\.\+|\.\*)\*|\+\+|\?\?|\{.*?\}\{.*?\}"##).ok() {
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
    fn test_S5852_registered() {
        let rule = S5852Rule::new();
        assert_eq!(rule.id(), "S5852");
        assert!(!rule.name().is_empty());
    }
}
