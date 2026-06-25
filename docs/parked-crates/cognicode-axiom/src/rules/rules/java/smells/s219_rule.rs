//! S219 — For loop variable used outside
//!
//! Detects for loop variables (i) used after the loop ends.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S219"
    name: "For loop variable used outside loop"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Using a for loop variable outside the loop scope is a code smell. The loop variable should not be used after the loop ends.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect for loop with int i
        let for_pattern = regex::Regex::new(r"for\s*\(\s*int\s+(\w+)\s*=").unwrap();

        for cap in for_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let for_start = matched.start();
                let var_name = cap.get(1).map(|m| m.as_str()).unwrap_or("i");

                // Find the end of this for loop
                let after_for = &source[for_start..];
                let block_end = find_for_loop_end(after_for);

                if let Some(end_idx) = block_end {
                    let after_loop = &after_for[end_idx..];

                    // Look for usage of 'i' after the loop
                    // Simple heuristic: check if 'i' appears in next few lines
                    let next_lines = after_loop.lines().take(5).collect::<String>();

                    let usage_pattern = regex::Regex::new(&format!(r"\b{}\b", regex::escape(var_name))).unwrap();
                    if usage_pattern.is_match(&next_lines) {
                        let line_num = source[..for_start].lines().count() + 1;
                        issues.push(Issue::new(
                            "JAVA_S219",
                            format!("For loop variable '{}' used after loop - consider redesign", var_name),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "Avoid using loop variables outside their scope."
                        )));
                    }
                }
            }
        }
        issues
    }
}

fn find_for_loop_end(s: &str) -> Option<usize> {
    let mut depth = 0;
    let chars: Vec<char> = s.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if *c == '{' {
            depth += 1;
        } else if *c == '}' {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1);
            }
        }
    }
    // Check for single statement for loops (no braces)
    if let Some(semi) = s.find(';') {
        if let Some(close_paren) = s.find(')') {
            if close_paren > semi {
                return Some(close_paren + 1);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;
    use cognicode_core::infrastructure::parser::Language;

    fn with_java_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Java.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Java,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_s219_registered() {
        let rule = JAVA_S219Rule::new();
        assert_eq!(rule.id(), "JAVA_S219");
    }

    #[test]
    fn test_s219_detects_var_used_after() {
        let rule = JAVA_S219Rule::new();
        let smelly = r#"
for (int i = 0; i < 10; i++) {
    System.out.println(i);
}
System.out.println(i);
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect 'i' used after loop");
        assert_eq!(issues[0].rule_id, "JAVA_S219");
    }

    #[test]
    fn test_s219_allows_var_not_used_after() {
        let rule = JAVA_S219Rule::new();
        let clean = r#"
for (int i = 0; i < 10; i++) {
    System.out.println(items.get(i));
}
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(
            issues.is_empty(),
            "Should not flag if 'i' is not used after"
        );
    }
}
