//! S2092 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S2092"
    name: "Cookies should set the Secure flag"
    severity: Minor
    category: SecurityHotspot
    language: "rust"
    params: {}

    explanation: "Cookies without the Secure flag can be transmitted over unencrypted connections, allowing cookie theft through network interception.",
    clean_code: Trustworthy,
    impacts: [Security: Low, Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
        if !trimmed.starts_with("//") && !trimmed.starts_with("/*") && !trimmed.starts_with("*/")
            && (line.contains("Set-Cookie") || line.contains(".cookie("))
            && !line.contains("Secure") && !line.contains("secure") {
                    issues.push(Issue::new(
                        "S2092",
                        "Cookie without Secure flag",
                        Severity::Minor,
                        Category::SecurityHotspot,
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
    fn test_s2092_registered() {
        let rule=S2092Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
