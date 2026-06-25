//! S4829 — print() in production code
//!
//! Detects print() statements outside of __main__ block, which may expose sensitive data.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S4829"
    name: "print() statements should not be used in production"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "print() statements in production code can expose sensitive data and indicate debug code left behind. Use proper logging instead.",
    clean_code: Clear,
    impacts: [Security: Low, Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let print_call = regex::Regex::new(r"\bprint\s*\(").unwrap();
        let in_main_block = regex::Regex::new(r#"if\s+__name__\s*==\s*["']__main__["']"#).unwrap();
        
        let mut in_main = false;
        let mut main_indent: Option<usize> = None;
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            
            // Track if we're inside __main__ block
            if in_main_block.is_match(trimmed) {
                in_main = true;
                main_indent = Some(line.len() - line.trim_start().len());
                continue;
            }
            
            if in_main {
                // Check if we've left the main block (dedent)
                if !trimmed.is_empty() {
                    let current_indent = line.len() - line.trim_start().len();
                    if let Some(main_indent_val) = main_indent {
                        if current_indent <= main_indent_val && !trimmed.starts_with("if ") {
                            in_main = false;
                            main_indent = None;
                        }
                    }
                }
            }
            
            if trimmed.starts_with('#') {
                continue;
            }
            
            if !in_main && print_call.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_S4829",
                    "print() statement found outside __main__ block",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Use proper logging (logging.info, logging.debug, etc.) instead of print()."
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
    fn test_s4829_registered() {
        let rule = PY_S4829Rule::new();
        assert_eq!(rule.id(), "PY_S4829");
    }

    #[test]
    fn test_s4829_detects_print_in_function() {
        let rule = PY_S4829Rule::new();
        let smelly = r#"
def process_data():
    print("Processing data")
    return result
"#;
        let issues = with_python_context(smelly, "processor.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect print() outside __main__");
        assert_eq!(issues[0].rule_id, "PY_S4829");
    }

    #[test]
    fn test_s4829_allows_print_in_main() {
        let rule = PY_S4829Rule::new();
        let clean = r#"
if __name__ == "__main__":
    print("Running main")
    main()
"#;
        let issues = with_python_context(clean, "main.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag print() inside __main__ block");
    }
}
