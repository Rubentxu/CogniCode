//! N9 — Mutable default argument
//!
//! Detects function definitions with mutable default arguments (list, dict, set).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N9"
    name: "Mutable default argument detected"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using mutable default arguments (list, dict, set) can lead to unexpected behavior as the default value is shared across calls. Use None instead.",
    clean_code: Clear,
    impacts: [Reliability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find function definitions with mutable default arguments
        // Pattern: def foo(x=[]) or def foo(x={}) or def foo(x=set())
        let mutable_defaults = [
            (r"def\s+\w+\s*\([^)]*=\s*\[\s*\]", "list"),
            (r"def\s+\w+\s*\([^)]*=\s*\{\s*\}", "dict"),
            (r"def\s+\w+\s*\([^)]*=\s*set\s*\(\s*\)", "set"),
        ];

        for (pattern, mutable_type) in mutable_defaults.iter() {
            let re = regex::Regex::new(pattern).unwrap();
            for cap in re.captures_iter(source) {
                let match_start = cap.get(0).unwrap().start();
                let line_num = source[..match_start].lines().count() + 1;
                issues.push(Issue::new(
                    "PY_N9",
                    format!("Mutable default argument ({}) detected. Use None instead.", mutable_type),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Use None as default and initialize inside the function: def foo(x=None): if x is None: x = []"
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
    fn test_n9_registered() {
        let rule = PY_N9Rule::new();
        assert_eq!(rule.id(), "PY_N9");
    }

    #[test]
    fn test_n9_detects_list_default() {
        let rule = PY_N9Rule::new();
        let smelly = r#"
def foo(x=[]):
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect list default argument");
        assert_eq!(issues[0].rule_id, "PY_N9");
    }

    #[test]
    fn test_n9_detects_dict_default() {
        let rule = PY_N9Rule::new();
        let smelly = r#"
def bar(x={}):
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect dict default argument");
    }

    #[test]
    fn test_n9_detects_set_default() {
        let rule = PY_N9Rule::new();
        let smelly = r#"
def baz(x=set()):
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect set default argument");
    }

    #[test]
    fn test_n9_allows_none_default() {
        let rule = PY_N9Rule::new();
        let clean = r#"
def foo(x=None):
    if x is None:
        x = []
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag None default");
    }
}