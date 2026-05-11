//! S1700 — Mutable default argument
//!
//! Detects mutable default arguments (list/dict as default) which can cause unexpected behavior.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1700"
    name: "Mutable default arguments should not be used"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Mutable default arguments are evaluated once at function definition time, not at each call. This can cause unexpected behavior. Use None and initialize inside the function.",
    clean_code: Clear,
    impacts: [Maintainability: High],
    check: => {
        let mut issues = Vec::new();
        let mutable_pattern = regex::Regex::new(r"=\s*(\[\s*\]|\{\s*\}|\[\s*,\s*\]|\{\s*:\s*\})").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                if mutable_pattern.is_match(line) {
                    issues.push(Issue::new(
                        "PY_S1700",
                        format!("Mutable default argument detected at line {}", line_num + 1),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        "Use None as default and initialize mutable objects inside the function."
                    )));
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
    fn test_s1700_registered() {
        let rule = PY_S1700Rule::new();
        assert_eq!(rule.id(), "PY_S1700");
    }

    #[test]
    fn test_s1700_detects_list_default() {
        let rule = PY_S1700Rule::new();
        let smelly = r#"
def append_to(element, target=[]):
    target.append(element)
    return target
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect mutable list default");
        assert_eq!(issues[0].rule_id, "PY_S1700");
    }

    #[test]
    fn test_s1700_detects_dict_default() {
        let rule = PY_S1700Rule::new();
        let smelly = r#"
def set_value(key, value, data={}):
    data[key] = value
    return data
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect mutable dict default");
        assert_eq!(issues[0].rule_id, "PY_S1700");
    }

    #[test]
    fn test_s1700_allows_none_default() {
        let rule = PY_S1700Rule::new();
        let clean = r#"
def append_to(element, target=None):
    if target is None:
        target = []
    target.append(element)
    return target
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag None default");
    }
}
