//! S115 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S115"
    name: "Constant names should follow UPPER_CASE convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Constant names not following UPPER_CASE convention reduce code readability and make it harder to distinguish constants from variables.",
    clean_code: Efficient,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"const\s+([a-z][A-Za-z0-9_]*)\s*(?::|=)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                if name != name.to_uppercase() {
                    issues.push(Issue::new("S115", format!("Constant '{}' should be UPPER_CASE", name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
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
    fn test_s115_registered() {
        let rule=S115Rule::new();
        assert_eq!(rule.id(), "S115");
        assert!(rule.name().len()>0);
    }
}
