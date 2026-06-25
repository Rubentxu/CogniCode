//! P1 — range(len(x)) instead of enumerate
//!
//! Detects inefficient use of range(len(x)) when enumerate would be more appropriate.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P1"
    name: "Use enumerate() instead of range(len(x))"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using range(len(x)) to iterate over indices is less efficient and readable than using enumerate().",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let range_len_pattern = regex::Regex::new(r"range\s*\(\s*len\s*\(").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if range_len_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_P1",
                    format!("Inefficient range(len(x)) pattern detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Use enumerate() instead: for i, item in enumerate(items):"
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
    fn test_p1_registered() {
        let rule = PY_P1Rule::new();
        assert_eq!(rule.id(), "PY_P1");
    }

    #[test]
    fn test_p1_detects_range_len() {
        let rule = PY_P1Rule::new();
        let smelly = r#"
for i in range(len(items)):
    print(items[i])
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect range(len()) pattern");
        assert_eq!(issues[0].rule_id, "PY_P1");
    }

    #[test]
    fn test_p1_allows_enumerate() {
        let rule = PY_P1Rule::new();
        let clean = r#"
for i, item in enumerate(items):
    print(item)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag enumerate() usage");
    }

    #[test]
    fn test_p1_detects_complex_case() {
        let rule = PY_P1Rule::new();
        let smelly = r#"
for i in range(len(my_list)):
    result.append(my_list[i] * 2)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect range(len()) in more complex case");
    }
}
