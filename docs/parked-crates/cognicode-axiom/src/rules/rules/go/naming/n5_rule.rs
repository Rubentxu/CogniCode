//! N5 — Unused import
//!
//! Detects imported packages that are never used in the code.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S170"
    name: "Unused import should be removed"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Imported packages that are never used add unnecessary overhead and can indicate incomplete refactoring.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Extract import names
        // Match: import "package" or import "package" // or import (
        let import_pattern = regex::Regex::new(r#"import\s+(?:\(\s*)?["']([^"']+)["']"#).unwrap();
        let import_alias_pattern = regex::Regex::new(r#"import\s+\w+\s+["']([^"']+)["']"#).unwrap();

        let mut imported_pkgs = Vec::new();
        for cap in import_pattern.captures_iter(source) {
            if let Some(pkg) = cap.get(1) {
                imported_pkgs.push(pkg.as_str().to_string());
            }
        }
        for cap in import_alias_pattern.captures_iter(source) {
            if let Some(pkg) = cap.get(1) {
                imported_pkgs.push(pkg.as_str().to_string());
            }
        }

        // For each imported package, check if it's used anywhere in the code
        for pkg in imported_pkgs {
            // Skip standard library packages that might not appear directly
            let pkg_name = pkg.split('/').last().unwrap_or(&pkg);
            // Check if package name appears outside of import statement
            let usage_pattern = format!(r"{}\.", pkg_name);
            let import_line_pattern = format!(r#"import\s+.*["']{}["']"#, regex::escape(&pkg));

            let re_usage = regex::Regex::new(&usage_pattern).unwrap();
            let re_import = regex::Regex::new(&import_line_pattern).unwrap();

            let usages: Vec<_> = re_usage.find_iter(source).collect();
            let import_lines: Vec<_> = re_import.find_iter(source).collect();

            // If no usages and we found the import
            if usages.is_empty() && !import_lines.is_empty() {
                // Get line number from import
                let import_match = import_lines[0];
                let line_num = source[..import_match.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "GO_S170",
                    format!("Imported package '{}' is never used", pkg),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Remove the unused import or use the package"
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

    fn with_go_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Go.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Go,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_n5_registered() {
        let rule = GO_S170Rule::new();
        assert_eq!(rule.id(), "GO_S170");
    }

    #[test]
    fn test_n5_detects_unused_import() {
        let rule = GO_S170Rule::new();
        let smelly = r#"
package main

import "fmt"

func main() {
    x := 1
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unused import");
        assert_eq!(issues[0].rule_id, "GO_S170");
    }

    #[test]
    fn test_n5_allows_used_import() {
        let rule = GO_S170Rule::new();
        let clean = r#"
package main

import "fmt"

func main() {
    fmt.Println("Hello")
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag used imports");
    }
}
