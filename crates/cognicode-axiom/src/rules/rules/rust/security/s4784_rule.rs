//! Auto-generated from KB: S4784
//! Languages: rust
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use regex;

declare_rule! {
    id: "S4784"
    name: "RegExp Injection"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "Avoid eval with user input in regex"

    impacts: [Security: Medium]

    check: => {
        let mut issues = Vec::new();
        for node in ctx.query_nodes("(call_expression (identifier) @call (#eq? @call \"eval\"))") {
            let node_text = node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
            if true {
                let start = node.start_position();
                issues.push(Issue::new(
                    self.id(),
                    "Security issue detected",
                    self.severity(),
                    self.category(),
                    ctx.file_path,
                    start.row + 1,
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
    fn test_S4784_registered() {
        let rule = S4784Rule::new();
        assert_eq!(rule.id(), "S4784");
        assert!(!rule.name().is_empty());
    }
}
