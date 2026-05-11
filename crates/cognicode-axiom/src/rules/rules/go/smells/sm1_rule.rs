//! SM1 — Long function (>60 lines)
//!
//! Detects functions that are too long.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S138"
    name: "Function should not be too long (>60 lines)"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Long functions are hard to read and maintain. Consider splitting into smaller, focused functions.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find function definitions
        let func_pattern = regex::Regex::new(r"func\s+(\w+)\s*\([^)]*\)\s*\{").unwrap();

        for cap in func_pattern.captures_iter(source) {
            if let Some(func_name) = cap.get(1) {
                let func_start = cap.get(0).unwrap().start();

                // Find the closing brace by counting braces
                let remaining = &source[func_start..];
                let mut brace_count = 0;
                let mut func_end = 0;
                let mut in_string = false;
                let mut escaped = false;

                for (i, c) in remaining.char_indices() {
                    if escaped {
                        escaped = false;
                        continue;
                    }
                    match c {
                        '"' if !in_string => in_string = true,
                        '"' if in_string => in_string = false,
                        '\\' if in_string => escaped = true,
                        '{' if !in_string => {
                            brace_count += 1;
                            if brace_count == 1 {
                                func_end = i;
                            }
                        },
                        '}' if !in_string => {
                            brace_count -= 1;
                            if brace_count == 0 {
                                func_end = i + 1;
                                break;
                            }
                        },
                        _ => {}
                    }
                }

                let func_body = &remaining[0..func_end];
                let lines = func_body.lines().count();

                if lines > 60 {
                    let line_num = source[..func_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "GO_S138",
                        format!("Function '{}' is {} lines (max 60)", func_name.as_str(), lines),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Split this function into smaller, focused functions"
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
    fn test_sm1_registered() {
        let rule = GO_S138Rule::new();
        assert_eq!(rule.id(), "GO_S138");
    }

    #[test]
    fn test_sm1_detects_long_function() {
        let rule = GO_S138Rule::new();
        let smelly = r#"
func LongFunction() {
    fmt.Println(1)
    fmt.Println(2)
    fmt.Println(3)
    fmt.Println(4)
    fmt.Println(5)
    fmt.Println(6)
    fmt.Println(7)
    fmt.Println(8)
    fmt.Println(9)
    fmt.Println(10)
    fmt.Println(11)
    fmt.Println(12)
    fmt.Println(13)
    fmt.Println(14)
    fmt.Println(15)
    fmt.Println(16)
    fmt.Println(17)
    fmt.Println(18)
    fmt.Println(19)
    fmt.Println(20)
    fmt.Println(21)
    fmt.Println(22)
    fmt.Println(23)
    fmt.Println(24)
    fmt.Println(25)
    fmt.Println(26)
    fmt.Println(27)
    fmt.Println(28)
    fmt.Println(29)
    fmt.Println(30)
    fmt.Println(31)
    fmt.Println(32)
    fmt.Println(33)
    fmt.Println(34)
    fmt.Println(35)
    fmt.Println(36)
    fmt.Println(37)
    fmt.Println(38)
    fmt.Println(39)
    fmt.Println(40)
    fmt.Println(41)
    fmt.Println(42)
    fmt.Println(43)
    fmt.Println(44)
    fmt.Println(45)
    fmt.Println(46)
    fmt.Println(47)
    fmt.Println(48)
    fmt.Println(49)
    fmt.Println(50)
    fmt.Println(51)
    fmt.Println(52)
    fmt.Println(53)
    fmt.Println(54)
    fmt.Println(55)
    fmt.Println(56)
    fmt.Println(57)
    fmt.Println(58)
    fmt.Println(59)
    fmt.Println(60)
    fmt.Println(61)
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect long function");
        assert_eq!(issues[0].rule_id, "GO_S138");
    }

    #[test]
    fn test_sm1_allows_short_function() {
        let rule = GO_S138Rule::new();
        let clean = r#"
func ShortFunction() {
    fmt.Println(1)
    fmt.Println(2)
    fmt.Println(3)
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag short functions");
    }
}
