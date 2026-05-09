//! S1134 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1134"
    name: "Deprecated code should not be used"
    severity: Info
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Using deprecated code can lead to compatibility issues, security vulnerabilities, and difficulties in future maintenance as deprecated APIs may be removed.",
    clean_code: Complete,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let deprecated_pattern = regex::Regex::new(r#"(?i)^\s*#\s*\[deprecated\b)"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") 
            || trimmed.starts_with("///") || trimmed.starts_with("//!")
            || trimmed.starts_with("/*")
            { continue; }
            
            if deprecated_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "S1134",
                    "Deprecated attribute detected",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Replace deprecated API with the recommended alternative"
                )));
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_s1134_registered() {
        let rule=S1134Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
