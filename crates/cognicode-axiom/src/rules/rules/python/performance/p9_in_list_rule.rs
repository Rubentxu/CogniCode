//! P9 — in on list instead of set
//!
//! Detects using 'in' operator on a list when a set would be more efficient.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P9"
    name: "Use set for membership testing instead of list"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using 'in' operator on a list is O(n) while on a set it's O(1). If checking membership multiple times, use a set.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        // Pattern for checking 'in list' where list is defined elsewhere
        // Match patterns like "x in list_name[" or "x in list_name[:"
        let in_list_pattern = regex::Regex::new(r"\bin\s+\w+\s*\[\s*").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if in_list_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_P9",
                    format!("'in' on list detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Consider using a set or frozenset for membership testing: 'x in my_set' is O(1)."
                )));
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
    fn test_p9_registered() {
        let rule = PY_P9Rule::new();
        assert_eq!(rule.id(), "PY_P9");
    }

    #[test]
    fn test_p9_detects_in_list() {
        let rule = PY_P9Rule::new();
        let smelly = r#"
if x in items[:]:
    print("found")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect 'in list[:]' pattern");
        assert_eq!(issues[0].rule_id, "PY_P9");
    }

    #[test]
    fn test_p9_allows_in_set() {
        let rule = PY_P9Rule::new();
        let clean = r#"
if x in my_set:
    print("found")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag 'in set'");
    }

    #[test]
    fn test_p9_allows_in_dict() {
        let rule = PY_P9Rule::new();
        let clean = r#"
if key in my_dict:
    print("found")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag 'in dict' (dict keys)");
    }
}
