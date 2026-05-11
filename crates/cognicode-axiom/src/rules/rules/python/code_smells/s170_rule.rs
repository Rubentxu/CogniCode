//! S170 — Unused import
//!
//! Detects imported modules that are not used in the code.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S170"
    name: "Unused imports should be removed"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Unused imports increase load time and make code harder to understand. Remove them.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();

        // Extract all imports
        let mut imports: Vec<(String, usize)> = Vec::new();
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // import x
            if trimmed.starts_with("import ") && !trimmed.contains("from ") {
                if let Some(name) = trimmed.split("import ").nth(1) {
                    let module = name.trim().split_whitespace().next().unwrap_or(name.trim());
                    let clean_module = module.split('.').next().unwrap_or(module).trim();
                    imports.push((clean_module.to_string(), line_num));
                }
            }
            // from x import y
            if trimmed.starts_with("from ") {
                if let Some(name) = trimmed.split("from ").nth(1) {
                    if let Some(_imports) = name.split(" import ").next() {
                        let module = name.split(" import ").next().unwrap_or(name).trim();
                        imports.push((module.to_string(), line_num));
                    }
                }
            }
        }

        // Check each import against the rest of the code
        let code_without_imports: String = ctx.source.lines()
            .enumerate()
            .filter(|(line_num, _)| !imports.iter().any(|(_, ln)| *ln == *line_num))
            .map(|(_, line)| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        for (module, line_num) in imports {
            // Skip common built-in modules that might be used in special ways
            if module == "os" || module == "sys" || module == "typing" || module == "collections" {
                continue;
            }
            // Check if module name appears as usage
            let module_pattern = regex::Regex::new(&format!(r"\b{}(?:\s*\.|\s*:|\s*,|\s*\))", regex::escape(&module))).unwrap();
            let from_pattern = regex::Regex::new(&format!(r"from\s+{}(?:\s*\.|\s*,|\s|$)", regex::escape(&module))).unwrap();

            if !module_pattern.is_match(&code_without_imports) && !from_pattern.is_match(ctx.source) {
                // Additional check: if it's "from X import Y", check if Y is used
                issues.push(Issue::new(
                    "PY_S170",
                    format!("Unused import '{}' at line {}", module, line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Remove this unused import."
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
    fn test_s170_registered() {
        let rule = PY_S170Rule::new();
        assert_eq!(rule.id(), "PY_S170");
    }

    #[test]
    fn test_s170_detects_unused() {
        let rule = PY_S170Rule::new();
        let smelly = r#"
import json
def process():
    return True
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unused import");
        assert_eq!(issues[0].rule_id, "PY_S170");
    }

    #[test]
    fn test_s170_allows_used() {
        let rule = PY_S170Rule::new();
        let clean = r#"
import json
def process():
    return json.loads('{}')
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag used import");
    }
}
