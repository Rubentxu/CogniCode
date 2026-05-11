//! S1135 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1135"
    name: "TODO tags should be completed or removed"
    severity: Minor
    category: CodeSmell
    language: "*"
    params: {
    tags: Vec<String> = vec![
        "TODO".to_string(),
        "FIXME".to_string(),
        "HACK".to_string(),
        "XXX".to_string()
    ]
}

    explanation: "TODO and FIXME tags indicate incomplete work that should be tracked and completed to avoid leaving technical debt or forgotten tasks in the codebase.",
    clean_code: Complete,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Pre-compile regex once - pattern is constant
        let re = regex::Regex::new(r"(?i)\b(TODO|FIXME|HACK|XXX)\b(?![a-zA-Z0-9_])").unwrap();
        for (line_num, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1135",
                    format!("TODO/FIXME/HACK/XXX tag found: {}", line.trim()),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
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
    fn test_s1135_registered() {
        let rule=S1135Rule::new();
        assert_eq!(rule.id(), "S1135");
        assert!(rule.name().len()>0);
    }
}
