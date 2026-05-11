//! S1135 — TODO/FIXME tags
//!
//! Detects TODO and FIXME comments that indicate incomplete work.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1135"
    name: "TODO/FIXME comments should be completed"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "TODO and FIXME comments indicate incomplete work and should be addressed before shipping.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let todo_pattern = regex::Regex::new(r"(?i)\b(TODO|FIXME|HACK|XXX)\b").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') && todo_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_S1135",
                    format!("Incomplete work found: {}", trimmed),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Complete the work indicated by this comment or create a tracking issue."
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
    fn test_s1135_registered() {
        let rule = PY_S1135Rule::new();
        assert_eq!(rule.id(), "PY_S1135");
    }

    #[test]
    fn test_s1135_detects_todo() {
        let rule = PY_S1135Rule::new();
        let smelly = r#"
def incomplete():
    # TODO: implement this
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect TODO comment");
        assert_eq!(issues[0].rule_id, "PY_S1135");
    }

    #[test]
    fn test_s1135_allows_no_todo() {
        let rule = PY_S1135Rule::new();
        let clean = r#"
def complete():
    # This is a normal comment
    return True
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal comments");
    }
}
