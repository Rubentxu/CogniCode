//! S1871 — Duplicate branches
//!
//! Detects duplicate if/elif branches in the same function.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;
use std::collections::HashMap;

declare_rule! {
    id: "PY_S1871"
    name: "Duplicate branches should not exist"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Duplicate branches indicate redundant code that is harder to maintain.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();

        // Simple detection: look for patterns like "if x: return y" followed by "elif z: return y"
        let return_pattern = regex::Regex::new(r":\s*return\s+").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for i in 0..lines.len().saturating_sub(1) {
            let current = lines[i].trim();
            let next = lines.get(i + 1).map(|l| l.trim()).unwrap_or("");

            // Skip empty/comment lines
            if current.is_empty() || current.starts_with('#') ||
               next.is_empty() || next.starts_with('#') {
                continue;
            }

            // Check if both lines have return statements with same return value
            if (current.starts_with("if ") || current.starts_with("elif ")) &&
               (next.starts_with("if ") || next.starts_with("elif ") || next.starts_with("else:")) {
                if let Some(current_return) = return_pattern.find(current) {
                    if let Some(next_return) = return_pattern.find(next) {
                        // Extract the return value
                        let current_val = &current[current_return.end()..];
                        let next_val = &next[next_return.end()..];

                        if current_val.trim() == next_val.trim() && !current_val.trim().is_empty() {
                            issues.push(Issue::new(
                                "PY_S1871",
                                "Duplicate branch return values detected",
                                Severity::Major,
                                Category::CodeSmell,
                                ctx.file_path,
                                i + 1,
                            ).with_remediation(Remediation::quick(
                                "Merge duplicate branches with same return value."
                            )));
                            break;
                        }
                    }
                }
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
    fn test_s1871_registered() {
        let rule = PY_S1871Rule::new();
        assert_eq!(rule.id(), "PY_S1871");
    }

    #[test]
    fn test_s1871_detects_duplicate_branches() {
        let rule = PY_S1871Rule::new();
        // Consecutive branches with identical body - use simple same-line approach
        let smelly = r#"def check(x):
    if x > 0: return 1
    elif x == 0: return 1
    else: return 0
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect duplicate branches");
        assert_eq!(issues[0].rule_id, "PY_S1871");
    }

    #[test]
    fn test_s1871_allows_unique_branches() {
        let rule = PY_S1871Rule::new();
        let clean = r#"
def check_status(status):
    if status == 1:
        return "one"
    elif status == 2:
        return "two"
    elif status == 3:
        return "three"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag unique branches");
    }
}
