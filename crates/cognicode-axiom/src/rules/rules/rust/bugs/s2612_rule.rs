//! S2612 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S2612"
    name: "File permissions should not be too permissive"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "Overly permissive file permissions (0o777/777) allow unauthorized users to read or modify sensitive files, creating security vulnerabilities. Also consider 0o666 for world-writable files.",
    clean_code: Trustworthy,
    impacts: [Security: High, Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"0o?777|0o666|chmod\s+777").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S2612",
                    "Overly permissive file permissions (0777)",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Use 0o644 for files and 0o755 for directories")));
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_s2612_registered() {
        let rule=S2612Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
