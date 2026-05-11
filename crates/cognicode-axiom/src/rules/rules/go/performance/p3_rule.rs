//! P3 — append in loop without pre-allocation
//!
//! Detects append in loops without pre-allocation.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1943"
    name: "append in loop without pre-allocation"
    severity: Info
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Using append in a loop without pre-allocation can cause multiple reallocations. Consider pre-allocating the slice.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find for loops containing append
        let append_pattern = regex::Regex::new(r"append\s*\(").unwrap();

        let mut in_loop = false;
        let mut loop_start = 0;

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("for ") || trimmed == "for {" {
                in_loop = true;
                loop_start = line_num;
            }

            if in_loop && append_pattern.is_match(line) {
                issues.push(Issue::new(
                    "GO_S1943",
                    format!("append in loop may benefit from pre-allocation"),
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Consider pre-allocating the slice with make() or using append with initial capacity"
                )));
            }

            if in_loop && trimmed == "}" {
                in_loop = false;
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
    fn test_p3_registered() {
        let rule = GO_S1943Rule::new();
        assert_eq!(rule.id(), "GO_S1943");
    }

    #[test]
    fn test_p3_detects_append_in_loop() {
        let rule = GO_S1943Rule::new();
        let smelly = r#"
func main() {
    var s []int
    for i := 0; i < 10; i++ {
        s = append(s, i)
    }
}
"#;
        let issues = with_go_context(smelly, "main.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect append in loop");
        assert_eq!(issues[0].rule_id, "GO_S1943");
    }

    #[test]
    fn test_p3_allows_no_append_in_loop() {
        let rule = GO_S1943Rule::new();
        let clean = r#"
func main() {
    s := make([]int, 10)
    for i := 0; i < 10; i++ {
        s[i] = i
    }
}
"#;
        let issues = with_go_context(clean, "main.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag index-based assignment");
    }
}
