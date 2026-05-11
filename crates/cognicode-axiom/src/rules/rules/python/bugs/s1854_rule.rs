//! S1854 — Unused import
//!
//! Detects import statements where the imported module is never used.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1854"
    name: "Imports should not be unused"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "An imported module is never used in the file. This adds unnecessary import overhead and indicates incomplete code.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Track imports and their line numbers
        let import_re = regex::Regex::new(r"^\s*import\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        let from_import_re = regex::Regex::new(r"^\s*from\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+import").unwrap();
        
        let mut imports: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        
        // First pass: collect all imports
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            
            if let Some(caps) = import_re.captures(trimmed) {
                if let Some(module) = caps.get(1) {
                    imports.insert(module.as_str().to_string(), line_num);
                }
            }
            if let Some(caps) = from_import_re.captures(trimmed) {
                if let Some(module) = caps.get(1) {
                    imports.insert(module.as_str().to_string(), line_num);
                }
            }
        }
        
        // Second pass: check if imports are used
        for (module, line_num) in &imports {
            let module_name = module.as_str();
            // Simple heuristic: check if module name appears outside import statements
            let mut used = false;
            for (check_line_num, line) in lines.iter().enumerate() {
                if check_line_num != *line_num {
                    let trimmed = line.trim();
                    // Check if module name is used as identifier
                    if trimmed.contains(module_name) && !trimmed.starts_with("import ") && !trimmed.starts_with("from ") {
                        used = true;
                        break;
                    }
                }
            }
            
            if !used {
                issues.push(Issue::new(
                    "PY_S1854",
                    &format!("Unused import: '{}'", module_name),
                    Severity::Minor,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    &format!("Remove the unused import of '{}'.", module_name)
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
    fn test_s1854_registered() {
        let rule = PY_S1854Rule::new();
        assert_eq!(rule.id(), "PY_S1854");
    }

    #[test]
    fn test_s1854_detects_unused_import() {
        let rule = PY_S1854Rule::new();
        let smelly = r#"
import os
print("hello")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unused import");
        assert_eq!(issues[0].rule_id, "PY_S1854");
    }

    #[test]
    fn test_s1854_allows_used_import() {
        let rule = PY_S1854Rule::new();
        let clean = r#"
import os
os.path.join("a", "b")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag used import");
    }
}
