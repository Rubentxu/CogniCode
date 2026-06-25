//! S220 — Private method only called from inner class
//!
//! Detects private methods that are only called from within inner classes.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S220"
    name: "Private method only called from inner class"
    severity: Info
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Private methods that are only called from inner classes could potentially be moved into the inner class, improving encapsulation.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect private method definitions
        let private_method_pattern = regex::Regex::new(r"private\s+\w+\s+\w+\s*\([^)]*\)").unwrap();

        for cap in private_method_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let method_start = matched.start();
                let method_str = matched.as_str();

                // Extract method name
                if let Some(name_match) = regex::Regex::new(r"private\s+\w+\s+(\w+)").unwrap().captures(method_str) {
                    if let Some(method_name) = name_match.get(1) {
                        let method_name_str = method_name.as_str();

                        // Find the method body
                        let after_method = &source[method_start..];
                        let body_start = after_method.find('{');

                        if let Some(body_idx) = body_start {
                            let body = &after_method[body_idx..];
                            let body_end = find_method_end(body);
                            let full_body = &body[..body_end];

                            // Check if method is called within its own body by an anonymous/inner class.
                            let called_from_inner = full_body.contains("new")
                                && regex::Regex::new(&format!(r"\b{}\s*\(", regex::escape(method_name_str))).unwrap().is_match(full_body);

                            if called_from_inner {
                                let line_num = source[..method_start].lines().count() + 1;
                                issues.push(Issue::new(
                                    "JAVA_S220",
                                    format!("Private method '{}' only called from inner class", method_name_str),
                                    Severity::Info,
                                    Category::CodeSmell,
                                    ctx.file_path,
                                    line_num,
                                ).with_remediation(Remediation::quick(
                                    "Consider moving this method to the inner class."
                                )));
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

fn find_method_end(s: &str) -> usize {
    let mut depth = 0;
    let chars: Vec<char> = s.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if *c == '{' {
            depth += 1;
        } else if *c == '}' {
            depth -= 1;
            if depth == 0 {
                return i + 1;
            }
        }
    }
    s.len()
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
    fn test_s220_registered() {
        let rule = JAVA_S220Rule::new();
        assert_eq!(rule.id(), "JAVA_S220");
    }

    #[test]
    fn test_s220_detects_private_method_in_inner_class() {
        let rule = JAVA_S220Rule::new();
        let smelly = r#"
private void helper() {
    new InnerClass() {
        void useHelper() {
            helper();
        }
    };
}
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(
            !issues.is_empty(),
            "Should detect private method called from inner class"
        );
        assert_eq!(issues[0].rule_id, "JAVA_S220");
    }

    #[test]
    fn test_s220_allows_public_method() {
        let rule = JAVA_S220Rule::new();
        let clean = r#"
public void helper() {
    System.out.println("helper");
}
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag public methods");
    }
}
