//! L21 — Redundant identity map
//!
//! Detects `.map(x -> x)` or `.map(Function.identity())` which are redundant operations.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L21"
    name: "Redundant identity map"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Using map(x -> x) or map(Function.identity()) is redundant as it does not transform the stream elements.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect .map(x -> x), .map(name -> name), and .map(Function.identity()).
        // Rust regex does not support backreferences, so compare captures in code.
        let lambda_pattern = regex::Regex::new(r"\.map\s*\(\s*([A-Za-z_]\w*)\s*->\s*([A-Za-z_]\w*)\s*\)").unwrap();

        for cap in lambda_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                if cap.get(1).map(|m| m.as_str()) != cap.get(2).map(|m| m.as_str()) {
                    continue;
                }
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L21",
                    "Redundant identity map - map(x -> x) does nothing",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Remove the redundant map() call."
                )));
            }
        }

        let identity_pattern = regex::Regex::new(r"\.map\s*\(\s*Function\s*\.\s*identity\s*\(\s*\)\s*\)").unwrap();
        for matched in identity_pattern.find_iter(source) {
            let start = matched.start();
            let line_num = source[..start].lines().count() + 1;
            issues.push(Issue::new(
                "JAVA_L21",
                "Redundant identity map - map(Function.identity()) does nothing",
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Remove the redundant map() call."
            )));
        }
        issues
    }
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
    fn test_l21_registered() {
        let rule = JAVA_L21Rule::new();
        assert_eq!(rule.id(), "JAVA_L21");
    }

    #[test]
    fn test_l21_detects_identity_map() {
        let rule = JAVA_L21Rule::new();
        let smelly = r#"
List<String> result = items.stream()
    .map(x -> x)
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect map(x -> x)");
        assert_eq!(issues[0].rule_id, "JAVA_L21");
    }

    #[test]
    fn test_l21_allows_transforming_map() {
        let rule = JAVA_L21Rule::new();
        let clean = r#"
List<String> result = items.stream()
    .map(String::toUpperCase)
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag transforming map");
    }
}
