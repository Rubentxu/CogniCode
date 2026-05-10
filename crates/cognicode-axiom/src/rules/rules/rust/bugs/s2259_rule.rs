//! S2259 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S2259"
    name: "Null pointer dereferences should be avoided"
    severity: Blocker
    category: Bug
    language: "rust"
    params: {}

    explanation: "Raw pointer dereferences without verification can cause undefined behavior including crashes, memory corruption, or security vulnerabilities.",
    clean_code: Logical,
    impacts: [Reliability: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\*\(?(\w+)\)?\s*\.\s*\w+").unwrap();
        let mut unsafe_depth = 0;
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("unsafe {") { unsafe_depth += 1; }
            if unsafe_depth > 0 && re.is_match(line) {
                issues.push(Issue::new("S2259", "Raw pointer dereference in unsafe block - verify non-null", Severity::Blocker, Category::Bug, ctx.file_path, idx + 1));
            }
            if line.trim() == "}" && unsafe_depth > 0 { unsafe_depth -= 1; }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s2259_registered() {
        let rule = S2259Rule::new();
        assert_eq!(rule.id(), "S2259");
        assert!(rule.name().len() > 0);
    }
}
