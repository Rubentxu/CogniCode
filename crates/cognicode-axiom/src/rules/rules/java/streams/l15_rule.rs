//! L30 — Lambda captures mutable variable
//!
//! Detects lambda expressions that reference non-final local variables.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L30"
    name: "Lambda captures mutable variable"
    severity: Minor
    category: Bug
    language: "Java"
    params: {}

    explanation: "Lambdas should only capture final or effectively final variables. Capturing a mutable variable can lead to unexpected behavior due to variable capture semantics.",
    clean_code: Clear,
    impacts: [Reliability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // This is a simplified heuristic detection
        // We look for variable assignments after declarations that are used in lambdas
        // Pattern: type name; ... name = ... in lambda

        // Find all variable declarations
        let var_decl_pattern = regex::Regex::new(r"\b(final\s+)?(?:int|long|double|float|boolean|String|List|Map|Set|var)\s+(\w+)\s*(?:=[^;]*)?;").unwrap();

        for cap in var_decl_pattern.captures_iter(source) {
            if cap.get(1).is_some() {
                continue;
            }

            if let Some(var_name) = cap.get(2) {
                let var_name_str = var_name.as_str();
                let decl_start = cap.get(0).unwrap().start();

                // Check if this variable is reassigned later
                let escaped = regex::escape(var_name_str);
                let reassign_pattern = regex::Regex::new(&format!(r"\b{}\s*=\s*[^=]", escaped)).unwrap();

                for reassign_cap in reassign_pattern.captures_iter(source) {
                    if let Some(reassign_match) = reassign_cap.get(0) {
                        let reassign_start = reassign_match.start();

                        // Now check if there's a lambda after the reassignment using the variable
                        let after_reassign = &source[reassign_start..];
                        let lambda_pattern = regex::Regex::new(&format!(r"->[^;{{}}]*\b{}\b", escaped)).unwrap();

                        if lambda_pattern.is_match(after_reassign) {
                            let line_num = source[..decl_start].lines().count() + 1;
                            issues.push(Issue::new(
                                "JAVA_L30",
                                format!("Lambda captures non-final variable '{}' that was modified after declaration", var_name_str),
                                Severity::Minor,
                                Category::Bug,
                                ctx.file_path,
                                line_num,
                            ).with_remediation(Remediation::quick(
                                "Make the variable final or use a different variable for the lambda."
                            )));
                            break;
                        }
                    }
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
    fn test_l30_registered() {
        let rule = JAVA_L30Rule::new();
        assert_eq!(rule.id(), "JAVA_L30");
    }

    #[test]
    fn test_l30_detects_mutable_capture() {
        let rule = JAVA_L30Rule::new();
        let smelly = r#"
int count = 0;
count = 5;
items.forEach(item -> System.out.println(count));
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(
            !issues.is_empty(),
            "Should detect mutable variable captured in lambda"
        );
        assert_eq!(issues[0].rule_id, "JAVA_L30");
    }

    #[test]
    fn test_l30_allows_final_variable() {
        let rule = JAVA_L30Rule::new();
        let clean = r#"
final int count = 0;
items.forEach(item -> System.out.println(count));
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(
            issues.is_empty(),
            "Should not flag final variable in lambda"
        );
    }
}
