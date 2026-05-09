//! S3330 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S3330"
    name: "Cookies should set the HttpOnly flag"
    severity: Minor
    category: SecurityHotspot
    language: "rust"
    params: {}

    explanation: "Cookies without HttpOnly flag can be accessed by JavaScript, making them vulnerable to cross-site scripting (XSS) theft.",
    clean_code: Trustworthy,
    impacts: [Security: Low, Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains(".cookie(") || line.contains("Set-Cookie"))
                && !line.to_lowercase().contains("httponly") && !line.to_lowercase().contains("http_only") {
                    issues.push(Issue::new(
                        "S3330",
                        "Cookie without HttpOnly flag - vulnerable to XSS",
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
    fn test_s3330_registered() {
        let rule=S3330Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
