//! N13 — Unused import
//!
//! Detects import statements where the imported name is never used in the file.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N13"
    name: "Unused import detected"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Imports that are never used add noise and may indicate incomplete refactoring. Remove unused imports.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all import statements
        // Pattern: import X or from X import Y
        let import_pattern = regex::Regex::new(
            r"(?m)^(?:from\s+(\w+)\s+import\s+(\w+)|import\s+(\w+))"
        ).unwrap();

        for cap in import_pattern.captures_iter(source) {
            let module_name = cap.get(1).or(cap.get(3)).map(|m| m.as_str()).unwrap_or("");
            let imported_name = cap.get(2).map(|m| m.as_str()).unwrap_or(module_name);

            // Check if the imported name appears elsewhere in the source
            // (excluding the import line itself and any comments on that line)
            let import_line_num = source[..cap.get(0).unwrap().start()].lines().count();

            let name_appears_elsewhere = source.lines()
                .enumerate()
                .filter(|(line_num, _)| *line_num != import_line_num)
                .any(|(_, line)| {
                    let code_part = line.split('#').next().unwrap_or(line);
                    // Check if the name appears as a word (not part of another word)
                    let re = regex::Regex::new(&format!(r"\b{}\b", imported_name)).unwrap();
                    re.is_match(code_part)
                });

            if !name_appears_elsewhere {
                let line_num = import_line_num + 1;
                issues.push(Issue::new(
                    "PY_N13",
                    format!("Unused import '{}'. Remove or use the imported name.", imported_name),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Remove the unused import statement"
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
    fn test_n13_registered() {
        let rule = PY_N13Rule::new();
        assert_eq!(rule.id(), "PY_N13");
    }

    #[test]
    fn test_n13_detects_unused_import() {
        let rule = PY_N13Rule::new();
        let smelly = r#"
import os

def foo():
    return 1
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unused import");
        assert_eq!(issues[0].rule_id, "PY_N13");
    }

    #[test]
    fn test_n13_detects_unused_from_import() {
        let rule = PY_N13Rule::new();
        let smelly = r#"
from collections import OrderedDict

def foo():
    return 1
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unused from import");
    }

    #[test]
    fn test_n13_allows_used_import() {
        let rule = PY_N13Rule::new();
        let clean = r#"
import os

def foo():
    return os.path.join("a", "b")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag used import");
    }

    #[test]
    fn test_n13_allows_used_from_import() {
        let rule = PY_N13Rule::new();
        let clean = r#"
from collections import OrderedDict

def foo():
    return OrderedDict()
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag used from import");
    }
}