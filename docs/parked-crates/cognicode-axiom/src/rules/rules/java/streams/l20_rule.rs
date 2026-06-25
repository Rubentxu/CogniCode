//! L35 — Collectors.toMap() without merge function
//!
//! Detects `Collectors.toMap()` without a merge function, which may cause issues with duplicate keys.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L35"
    name: "Collectors.toMap() without merge function"
    severity: Major
    category: Bug
    language: "Java"
    params: {}

    explanation: "Using Collectors.toMap() without a merge function throws IllegalStateException when duplicate keys are encountered. Provide a merge function to handle duplicates.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect Collectors.toMap(...) and count top-level arguments so nested calls/lambdas do not confuse matching.
        let to_map_pattern = regex::Regex::new(r"Collectors\s*\.\s*toMap\s*\(").unwrap();

        for matched in to_map_pattern.find_iter(source) {
            let args_start = matched.end();
            if let Some(args_end) = find_matching_paren(source, args_start) {
                let args = &source[args_start..args_end];
                if count_top_level_args(args) == 2 {
                    let line_num = source[..matched.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "JAVA_L35",
                        "Collectors.toMap() without merge function - may throw on duplicate keys",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Add a merge function as the third argument to handle duplicate keys."
                    )));
                }
            }
        }
        issues
    }
}

fn find_matching_paren(source: &str, start_after_open: usize) -> Option<usize> {
    let mut depth = 1;
    for (offset, ch) in source[start_after_open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start_after_open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn count_top_level_args(args: &str) -> usize {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return 0;
    }

    let mut depth = 0;
    let mut count = 1;
    for ch in trimmed.chars() {
        match ch {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    count
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
    fn test_l35_registered() {
        let rule = JAVA_L35Rule::new();
        assert_eq!(rule.id(), "JAVA_L35");
    }

    #[test]
    fn test_l35_detects_toMap_without_merge() {
        let rule = JAVA_L35Rule::new();
        let smelly = r#"
Map<String, Integer> map = items.stream()
    .collect(Collectors.toMap(Function.identity(), String::length));
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(
            !issues.is_empty(),
            "Should detect toMap() without merge function"
        );
        assert_eq!(issues[0].rule_id, "JAVA_L35");
    }

    #[test]
    fn test_l35_allows_toMap_with_merge() {
        let rule = JAVA_L35Rule::new();
        let clean = r#"
Map<String, Integer> map = items.stream()
    .collect(Collectors.toMap(Function.identity(), String::length, (a, b) -> a));
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(
            issues.is_empty(),
            "Should not flag toMap() with merge function"
        );
    }
}
