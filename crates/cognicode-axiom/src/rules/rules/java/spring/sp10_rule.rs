//! SP10 — Entity without Id
//!
//! Detects JPA @Entity classes without @Id annotation.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_SP10"
    name: "@Entity without @Id annotation"
    severity: Major
    category: Bug
    language: "Java"
    params: {}

    explanation: "Every JPA @Entity must have an @Id field annotated to uniquely identify the entity. Without it, the entity cannot be persisted.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find @Entity class
        let entity_pattern = regex::Regex::new(r"@Entity\s*(?:public\s+)?class\s+(\w+)").unwrap();

        for cap in entity_pattern.captures_iter(source) {
            if let Some(class_name) = cap.get(1) {
                let entity_start = cap.get(0).unwrap().end();
                let class_body = find_class_body(source, entity_start);

                // Check for @Id annotation
                let has_id = class_body.contains("@Id");

                if !has_id {
                    let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "JAVA_SP10",
                        format!("@Entity '{}' is missing @Id annotation", class_name.as_str()),
                        Severity::Critical,
                        Category::Bug,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Add @Id and @GeneratedValue annotations to the primary key field"
                    )));
                }
            }
        }
        issues
    }
}

fn find_class_body(source: &str, start: usize) -> String {
    let mut brace_count = 0;
    let mut in_string = false;
    let mut escaped = false;

    for (i, c) in source[start..].char_indices() {
        let absolute_i = start + i;
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '"' => in_string = !in_string,
            '\\' if in_string => escaped = true,
            '{' if !in_string => {
                if brace_count == 0 {
                    brace_count = 1;
                } else {
                    brace_count += 1;
                }
            },
            '}' if !in_string => {
                brace_count -= 1;
                if brace_count == 0 {
                    return source[start..absolute_i + 1].to_string();
                }
            },
            _ => {}
        }
    }
    source[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;
    use std::path::Path;
    use tree_sitter::Parser as TsParser;

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
    fn test_sp10_registered() {
        let rule = JAVA_SP10Rule::new();
        assert_eq!(rule.id(), "JAVA_SP10");
    }

    #[test]
    fn test_sp10_detects_entity_without_id() {
        let rule = JAVA_SP10Rule::new();
        let smelly = r#"
@Entity
public class User {
    private String name;
    private String email;
}
"#;
        let issues = with_java_context(smelly, "User.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect @Entity without @Id");
        assert_eq!(issues[0].rule_id, "JAVA_SP10");
    }

    #[test]
    fn test_sp10_allows_entity_with_id() {
        let rule = JAVA_SP10Rule::new();
        let clean = r#"
@Entity
public class User {
    @Id
    @GeneratedValue
    private Long id;
    private String name;
}
"#;
        let issues = with_java_context(clean, "User.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag @Entity with @Id");
    }
}
