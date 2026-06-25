//! N10 — print() in library code
//!
//! Detects print() calls outside of __main__ blocks in library code.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N10"
    name: "print() call detected in library code"
    severity: Info
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "print() calls in library code should be avoided. Use logging module or move print statements to __main__ block.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Check if there's a __main__ block
        let has_main_block = source.contains("if __name__ == '__main__':") || source.contains("if __name__ == \"__main__\":");

        if !has_main_block {
            // Find all print() calls
            let print_pattern = regex::Regex::new(r"\bprint\s*\(").unwrap();

            for cap in print_pattern.captures_iter(source) {
                let match_start = cap.get(0).unwrap().start();
                let line_num = source[..match_start].lines().count() + 1;
                issues.push(Issue::new(
                    "PY_N10",
                    "print() call detected in library code. Use logging module instead.",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Use the logging module for output, or move print statements to 'if __name__ == \"__main__\":' block"
                )));
            }
        } else {
            // If there is a __main__ block, only flag print() outside of it
            let print_pattern = regex::Regex::new(r"\bprint\s*\(").unwrap();
            let main_block_pattern = regex::Regex::new(r#"if __name__\s*==\s*['"]__main__['"]\s*:"#).unwrap();

            let mut in_main_block = false;
            let mut main_block_indent = 0;

            for (line_num, line) in source.lines().enumerate() {
                let trimmed = line.trim();

                if main_block_pattern.is_match(trimmed) {
                    in_main_block = true;
                    main_block_indent = line.len() - line.trim_start().len();
                    continue;
                }

                if in_main_block {
                    let current_indent = line.len() - line.trim_start().len();
                    if current_indent <= main_block_indent && !trimmed.is_empty() {
                        in_main_block = false;
                    }
                }

                if !in_main_block && print_pattern.is_match(trimmed) {
                    issues.push(Issue::new(
                        "PY_N10",
                        "print() call detected outside __main__ block. Use logging module instead.",
                        Severity::Info,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        "Use the logging module for output, or move print statements to 'if __name__ == \"__main__\":' block"
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
    fn test_n10_registered() {
        let rule = PY_N10Rule::new();
        assert_eq!(rule.id(), "PY_N10");
    }

    #[test]
    fn test_n10_detects_print_no_main() {
        let rule = PY_N10Rule::new();
        let smelly = r#"
def foo():
    print("hello")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect print without __main__ block");
        assert_eq!(issues[0].rule_id, "PY_N10");
    }

    #[test]
    fn test_n10_detects_print_outside_main() {
        let rule = PY_N10Rule::new();
        let smelly = r#"
def foo():
    print("hello")

if __name__ == "__main__":
    foo()
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect print outside __main__ block");
    }

    #[test]
    fn test_n10_allows_print_in_main() {
        let rule = PY_N10Rule::new();
        let clean = r#"
def foo():
    pass

if __name__ == "__main__":
    print("hello")
    foo()
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag print inside __main__ block");
    }
}