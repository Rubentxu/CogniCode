//! P13 — Repeated regex compile in loop
//!
//! Detects regex compilation inside loops which should be done once outside.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P13"
    name: "Compile regex outside of loops"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Compiling regex inside a loop is inefficient. Compile the regex once and reuse it.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Detect loop keywords
            if trimmed.starts_with("for ") || trimmed.starts_with("while ") {
                let mut indent_level = line.len() - line.trim_start().len();

                // Check subsequent lines for re.compile
                for (check_line_num, check_line) in ctx.source.lines().enumerate().skip(line_num + 1) {
                    let check_trimmed = check_line.trim();
                    if check_trimmed.is_empty() {
                        continue;
                    }
                    let check_indent = check_line.len() - check_line.trim_start().len();
                    if check_indent <= indent_level && !check_trimmed.is_empty() {
                        break;
                    }
                    // Check for re.compile pattern
                    if check_trimmed.contains("re.compile") || check_trimmed.contains("regex.compile") {
                        issues.push(Issue::new(
                            "PY_P13",
                            format!("Regex compile in loop detected at line {}", check_line_num + 1),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            check_line_num + 1,
                        ).with_remediation(Remediation::quick(
                            "Move regex compilation outside the loop: pattern = re.compile(...)"
                        )));
                        break;
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
    fn test_p13_registered() {
        let rule = PY_P13Rule::new();
        assert_eq!(rule.id(), "PY_P13");
    }

    #[test]
    fn test_p13_detects_regex_in_loop() {
        let rule = PY_P13Rule::new();
        let smelly = r#"
for line in lines:
    pattern = re.compile(r'\d+')
    match = pattern.search(line)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect regex compile in loop");
        assert_eq!(issues[0].rule_id, "PY_P13");
    }

    #[test]
    fn test_p13_allows_compiled_outside() {
        let rule = PY_P13Rule::new();
        let clean = r#"
pattern = re.compile(r'\d+')
for line in lines:
    match = pattern.search(line)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag compiled regex outside loop");
    }
}
