//! SM9 — Variable assigned but never used
//!
//! Detects variables that are assigned but never read.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1845"
    name: "Variable assigned but never used"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Variables that are assigned but never read are dead code.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find variable declarations
        let var_pattern = regex::Regex::new(r"(?:var\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s*:?=\s*").unwrap();

        let mut var_uses: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        // Find all identifier uses
        let ident_pattern = regex::Regex::new(r"\b([a-zA-Z_][a-zA-Z0-9_]*)\b").unwrap();
        for cap in ident_pattern.captures_iter(source) {
            if let Some(name) = cap.get(1) {
                *var_uses.entry(name.as_str().to_string()).or_insert(0) += 1;
            }
        }

        // Find assignments and check if used
        for cap in var_pattern.captures_iter(source) {
            if let Some(var_name) = cap.get(1) {
                let name_str = var_name.as_str().to_string();
                let line_num = source[..var_name.start()].lines().count() + 1;

                // Skip common short-lived variables
                if name_str == "_" || name_str == "i" || name_str == "j" || name_str == "k" {
                    continue;
                }

                // Count occurrences - if > 1, it's likely used
                let uses = var_uses.get(&name_str).copied().unwrap_or(0);
                if uses <= 1 {
                    issues.push(Issue::new(
                        "GO_S1845",
                        format!("Variable '{}' assigned but never used", name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Remove the unused variable assignment"
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
    fn test_sm9_registered() {
        let rule = GO_S1845Rule::new();
        assert_eq!(rule.id(), "GO_S1845");
    }

    #[test]
    fn test_sm9_detects_unused_var() {
        let rule = GO_S1845Rule::new();
        let smelly = r#"
func main() {
    unused := 42
    fmt.Println("hello")
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unused variable");
        assert_eq!(issues[0].rule_id, "GO_S1845");
    }

    #[test]
    fn test_sm9_allows_used_var() {
        let rule = GO_S1845Rule::new();
        let clean = r#"
func main() {
    used := 42
    fmt.Println(used)
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag used variables");
    }
}
