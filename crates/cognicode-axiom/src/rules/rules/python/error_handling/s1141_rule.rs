//! S1141 — Nested try-except (>2)
//!
//! Detects deeply nested try-except blocks (more than 2 levels).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1141"
    name: "Nested try-except blocks"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Deeply nested try-except blocks (>2 levels) make code hard to read and maintain. Consider extracting inner logic into separate functions.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;
        let lines: Vec<&str> = source.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("try:") {
                let try_col = lines[i].len() - lines[i].trim_start().len();
                let mut depth = 1;
                let mut j = i + 1;
                let mut first_try_line = i + 1;

                while j < lines.len() {
                    let next_line = lines[j].trim();
                    let next_col = lines[j].len() - lines[j].trim_start().len();

                    if next_col <= try_col && !next_line.is_empty() && !next_line.starts_with('#') {
                        break;
                    }

                    if next_line.starts_with("try:") {
                        depth += 1;
                        if depth > 2 {
                            issues.push(Issue::new(
                                "PY_S1141",
                                format!("Nested try-except block (depth: {}) at line {}", depth, first_try_line),
                                Severity::Minor,
                                Category::CodeSmell,
                                ctx.file_path,
                                first_try_line,
                            ).with_remediation(Remediation::quick(
                                "Extract inner try-except logic into a separate function to reduce nesting."
                            )));
                            break;
                        }
                    }
                    j += 1;
                }
            }
            i += 1;
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
    fn test_s1141_registered() {
        let rule = PY_S1141Rule::new();
        assert_eq!(rule.id(), "PY_S1141");
    }

    #[test]
    fn test_s1141_detects_deeply_nested() {
        let rule = PY_S1141Rule::new();
        let smelly = r#"
try:
    do_one()
    try:
        do_two()
        try:
            do_three()
        except:
            pass
    except:
        pass
except:
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect deeply nested try-except");
        assert_eq!(issues[0].rule_id, "PY_S1141");
    }

    #[test]
    fn test_s1141_allows_normal_nesting() {
        let rule = PY_S1141Rule::new();
        let clean = r#"
try:
    do_one()
    try:
        do_two()
    except:
        pass
except:
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow up to 2 levels of nesting");
    }
}
