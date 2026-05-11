//! S1162 — Exception class naming
//!
//! Detects exception classes that don't follow naming conventions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1162"
    name: "Exception class naming convention"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Exception classes should end with 'Error' or 'Exception' to clearly indicate their purpose.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let class_pattern = regex::Regex::new(r"(class\s+\w+)\s*\(\s*Exception\s*\)").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            if let Some(caps) = class_pattern.captures(line) {
                let class_def = caps.get(1).map_or("", |m| m.as_str());
                if let Some(class_name) = class_def.strip_prefix("class ") {
                    let name = class_name.trim();
                    if !name.ends_with("Error") && !name.ends_with("Exception") {
                        issues.push(Issue::new(
                            "PY_S1162",
                            format!("Exception class '{}' should end with 'Error' or 'Exception'", name),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::quick(
                            "Rename the exception class to end with 'Error' or 'Exception'."
                        )));
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
    fn test_s1162_registered() {
        let rule = PY_S1162Rule::new();
        assert_eq!(rule.id(), "PY_S1162");
    }

    #[test]
    fn test_s1162_detects_bad_naming() {
        let rule = PY_S1162Rule::new();
        let smelly = r#"
class DatabaseProblem(Exception):
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect exception without Error/Exception suffix");
        assert_eq!(issues[0].rule_id, "PY_S1162");
    }

    #[test]
    fn test_s1162_allows_proper_naming() {
        let rule = PY_S1162Rule::new();
        let clean = r#"
class DatabaseError(Exception):
    pass

class ProcessingException(Exception):
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow classes ending with Error or Exception");
    }
}
