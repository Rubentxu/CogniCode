//! S1313 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1313"
    name: "IP addresses should not be hardcoded"
    severity: Minor
    category: SecurityHotspot
    language: "*"
    params: {}

    explanation: "Hardcoded IP addresses make applications inflexible and difficult to deploy in different environments, reducing configurability and portability.",
    clean_code: Focused,
    impacts: [Security: Low, Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#""([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])"(?![0-9a-zA-Z])"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") 
            || trimmed.starts_with("///") || trimmed.starts_with("//!")
            || trimmed.starts_with("/*") || trimmed.starts_with("*")
            || trimmed.starts_with("#")
            || trimmed.contains("version") || trimmed.contains("Version")
            || trimmed.contains("coordinate") || trimmed.contains("Coordinate")
            || trimmed.contains("specification") || trimmed.contains("Specification")
            || trimmed.contains("example") || trimmed.contains("Example")
            { continue; }
            
            if let Some(m) = re.find(trimmed) {
                issues.push(Issue::new(
                    "S1313",
                    format!("Hardcoded IP address: {}", m.as_str()),
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
    fn test_s1313_registered() {
        let rule=S1313Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
