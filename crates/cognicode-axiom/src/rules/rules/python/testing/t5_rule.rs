//! T5 — unittest.skip without reason
//!
//! Detects unittest.skip decorators without a provided reason.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T5"
    name: "unittest.skip without reason"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using @skip without a reason makes it unclear why the test is skipped and when it should be re-enabled.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Match @skip without reason: @skip or @skip()
        let skip_no_reason = regex::Regex::new(r"@unittest\.skip\s*\(\s*\)").unwrap();
        // Match @skipIf/@skipUnless without reason
        let skip_cond_no_reason = regex::Regex::new(r"@(?:skipIf|skipUnless)\s*\(\s*\)").unwrap();

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if skip_no_reason.is_match(trimmed) || skip_cond_no_reason.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_T5",
                    format!("@skip decorator without reason at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Add a reason explaining why the test is skipped (e.g., @skip('Bug #123: reason'))"
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
    use cognicode_core::infrastructure::parser::Language;

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
    fn test_t5_registered() {
        let rule = PY_T5Rule::new();
        assert_eq!(rule.id(), "PY_T5");
    }

    #[test]
    fn test_t5_detects_skip_without_reason() {
        let rule = PY_T5Rule::new();
        let smelly = r#"
@unittest.skip()
def test_old():
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect @skip without reason");
        assert_eq!(issues[0].rule_id, "PY_T5");
    }

    #[test]
    fn test_t5_allows_skip_with_reason() {
        let rule = PY_T5Rule::new();
        let clean = r#"
@unittest.skip('Bug #123: waiting for fix')
def test_old():
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag @skip with reason");
    }
}
