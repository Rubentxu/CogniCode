//! S100 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S100"
    name: "Function names should follow snake_case convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Function names not following snake_case violate Rust naming conventions and reduce readability",
    clean_code: Efficient,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fn\s+(?![a-z][a-z0-9_]*_$)([A-Z][a-zA-Z0-9_]*|[a-z][a-zA-Z0-9_]*[A-Z][a-zA-Z0-9_]*)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line)
                && let Some(name) = cap.get(1) {
                    let name_str = name.as_str();
                    // Skip test functions by naming convention
                    if name_str.starts_with("test_") || name_str.contains("_test_") {
                        continue;
                    }
                    // Skip test functions marked with various test attributes
                    if idx > 0 {
                        let prev_line = ctx.source.lines().nth(idx - 1).unwrap_or("").trim();
                        if prev_line.contains("#[test]") || prev_line.contains("#[cfg(test)]") {
                            continue;
                        }
                    }
                    // Skip getter/setter functions (getFoo, setFoo patterns are conventional)
                    if (name_str.starts_with("get_") || name_str.starts_with("set_") || name_str.starts_with("is_"))
                        && name_str.chars().skip(4).all(|c| c.is_ascii_lowercase() || c == '_') {
                        continue;
                    }
                    // Skip closure parameters (|x| syntax is not a function declaration)
                    if name_str.starts_with('|') {
                        continue;
                    }
                    // Skip underscore-prefixed private helper functions
                    if name_str.starts_with('_') {
                        continue;
                    }
                    issues.push(Issue::new(
                        "S100",
                        format!("Function '{}' should use snake_case", name_str),
                        Severity::Minor,
                        Category::CodeSmell,
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
    fn test_s100_registered() {
        let rule=S100Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
