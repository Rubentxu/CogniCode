//! S1481 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S1481"
    name: "Unused local variables should be removed"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Variables prefixed with underscore but actually used indicate the developer intended to suppress warnings but used the wrong prefix.",
    clean_code: Focused,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"let\s+(?:mut\s+)?_\b(\w+)\b\s*(?::\s*\S+)?\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                let remaining: String = ctx.source.lines().skip(idx + 1).collect::<Vec<_>>().join("\n");
                let is_used = remaining.contains(&format!(" {} ", name))
                    || remaining.contains(&format!("({}", name))
                    || remaining.contains(&format!(",{}", name))
                    || remaining.contains(&format!("{})", name))
                    || remaining.contains(&format!(".{}", name))
                    || remaining.contains(&format!("[]{}", name));
                if !is_used {
                    issues.push(Issue::new(
                        "S1481",
                        format!("Unused variable '_{}' - remove it entirely", name),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
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
    fn test_s1481_registered() {
        let rule=S1481Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
