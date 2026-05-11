//! S1845 — Dead store
//!
//! Detects variable assignments that are immediately overwritten without being used.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1845"
    name: "Variable should not be assigned and then immediately overwritten"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "A variable is assigned a value and then immediately reassigned without the original value being used. This is a dead store.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Look for consecutive assignments to same variable with no use
        let assignment = regex::Regex::new(r"^([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*").unwrap();
        
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut pending_vars: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            
            // Check if this line uses any pending variable
            for var in pending_vars.keys().cloned().collect::<Vec<_>>() {
                if trimmed.contains(&format!("{}", var)) && !trimmed.starts_with(&format!("{} =", var)) {
                    pending_vars.remove(var);
                }
            }
            
            if let Some(caps) = assignment.captures(trimmed) {
                let var_name = caps.get(1).map(|m| m.as_str()).unwrap();
                if pending_vars.contains_key(var_name) {
                    // Found reassignment without use - but only flag if on consecutive-ish lines
                    if let Some(first_line) = pending_vars.get(var_name) {
                        if line_num - first_line < 5 {
                            issues.push(Issue::new(
                                "PY_S1845",
                                &format!("Dead store - variable '{}' assigned but never used before reassignment", var_name),
                                Severity::Minor,
                                Category::Bug,
                                ctx.file_path,
                                line_num + 1,
                            ).with_remediation(Remediation::quick(
                                &format!("Remove the first assignment to '{}' or use its value.", var_name)
                            )));
                        }
                    }
                }
                pending_vars.insert(var_name, line_num);
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;

    fn with_python_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Python.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Python,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_s1845_registered() {
        let rule = PY_S1845Rule::new();
        assert_eq!(rule.id(), "PY_S1845");
    }

    #[test]
    fn test_s1845_detects_dead_store() {
        let rule = PY_S1845Rule::new();
        let smelly = r#"
x = 1
x = 2
print("done")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect dead store");
        assert_eq!(issues[0].rule_id, "PY_S1845");
    }

    #[test]
    fn test_s1845_allows_proper_use() {
        let rule = PY_S1845Rule::new();
        let clean = r#"
x = 1
print(x)
x = 2
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag when first assignment is used");
    }
}
