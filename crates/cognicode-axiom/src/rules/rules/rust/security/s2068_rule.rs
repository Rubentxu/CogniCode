//! S2068 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S2068"
    name: "Hard-coded credentials are security sensitive"
    severity: Blocker
    category: SecurityHotspot
    language: "*"
    params: {}

    explanation: "Hard-coded credentials make secrets accessible to anyone with source code access, increasing the risk of credential leakage and unauthorized system access.",
    clean_code: Trustworthy,
    impacts: [Security: High, Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let patterns = [
            (r#"(?i)(password|passwd|pwd)\s*[=:]\s*["'][^"']{8,}["']"#, "password"),
            (r#"(?i)(api[_-]?key|apikey)\s*[=:]\s*["'][^"']{8,}["']"#, "api_key"),
            (r#"(?i)(secret|token)\s*[=:]\s*["'][^"']{8,}["']"#, "secret"),
            (r#"(?i)(bearer\s+token|basic\s+auth)\s*[=:]\s*["'][a-zA-Z0-9_\-]{8,}["']"#, "bearer_token"),
        ];
        let regexes: Vec<_> = patterns.iter().map(|(p, _)| regex::Regex::new(p).unwrap()).collect();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            // Skip comments, docstrings, and empty lines to avoid false positives
            let trimmed = line.trim();
            if trimmed.is_empty() 
            || trimmed.starts_with("//") || trimmed.starts_with("///")
            || trimmed.starts_with("//!") || trimmed.starts_with("/*")
            || trimmed.starts_with("#")
            { continue; }
            
            for re in &regexes {
                if re.is_match(trimmed) {
                    issues.push(Issue::new(
                        "S2068",
                        format!("Hard-coded credential detected on line {}", line_num + 1),
                        Severity::Blocker,
                        Category::SecurityHotspot,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::moderate(
                        "Use environment variables or a secrets manager instead of hard-coded values"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_s2068_registered() {
        let rule=S2068Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
