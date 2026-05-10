//! S4144 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S4144"
    name: "Test methods should not be duplicated"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Duplicated test functions waste build time and indicate possible copy-paste testing",
    clean_code: Distinct,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let mut test_names: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
        let re = regex::Regex::new(r"fn\s+(test_[a-zA-Z0-9]+|[a-zA-Z0-9]+_test)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str().to_string();
                test_names.entry(name).or_default().push(idx + 1);
            }
        }
        for (name, lines) in test_names {
            let count = lines.len();
            if count > 1 {
                for &line in &lines {
                    issues.push(Issue::new(
                        "S4144",
                        format!("Duplicated test function '{}' - appears {} times", name, count),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        line,
                    ).with_remediation(Remediation::moderate("Remove duplicate test or merge them")));
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
    fn test_s4144_registered() {
        let rule=S4144Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
