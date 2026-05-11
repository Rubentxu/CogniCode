//! SP3 — Service with state
//!
//! Detects @Service classes with mutable instance fields (not thread-safe).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_SP3"
    name: "@Service with mutable state"
    severity: Major
    category: Bug
    language: "Java"
    params: {}

    explanation: "@Service classes with mutable instance fields are not thread-safe and can cause race conditions in concurrent environments.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find @Service class with non-final fields
        let service_class_pattern = regex::Regex::new(r"@Service\s*(?:public\s+)?class\s+(\w+)").unwrap();

        for cap in service_class_pattern.captures_iter(source) {
            if let Some(class_name) = cap.get(1) {
                let class_start = cap.get(0).unwrap().end();
                let class_end = find_class_end(source, class_start);
                let class_body = &source[class_start..class_end];

                // Find fields - look for patterns like "private type name;" or "private final type name;"
                // A field line typically has: modifier + type + name + optional_init + ;
                // Methods have: modifier + return_type + name + () + body
                let lines = class_body.lines();
                let mut has_mutable_field = false;

                for line in lines {
                    let trimmed = line.trim();
                    // Check if it's a field declaration (not a method)
                    // Fields: have a type directly after modifier, then variable name, then semicolon
                    // Methods: have parentheses after name
                    if (trimmed.starts_with("private") || trimmed.starts_with("public") || trimmed.starts_with("protected"))
                        && !trimmed.starts_with("static")
                        && trimmed.contains(';')
                        && !trimmed.contains('(')  // Methods have (), fields don't
                    {
                        // If it's not a final field, it's mutable
                        if !trimmed.contains("final") {
                            has_mutable_field = true;
                            break;
                        }
                    }
                }

                if has_mutable_field {
                    let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "JAVA_SP3",
                        format!("@Service '{}' has mutable instance fields and may not be thread-safe", class_name.as_str()),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Use immutable objects or make fields final, or document thread-safety assumptions"
                    )));
                }
            }
        }
        issues
    }
}

fn find_class_end(source: &str, start: usize) -> usize {
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
            '{' if !in_string => brace_count += 1,
            '}' if !in_string => {
                brace_count -= 1;
                if brace_count == 0 {
                    return absolute_i + 1;
                }
            },
            _ => {}
        }
    }
    source.len()
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
    fn test_sp3_registered() {
        let rule = JAVA_SP3Rule::new();
        assert_eq!(rule.id(), "JAVA_SP3");
    }

    #[test]
    fn test_sp3_detects_mutable_state() {
        let rule = JAVA_SP3Rule::new();
        let smelly = r#"
@Service
public class MyService {
    private int counter;
    public void increment() { counter++; }
}
"#;
        let issues = with_java_context(smelly, "MyService.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect mutable state in @Service");
        assert_eq!(issues[0].rule_id, "JAVA_SP3");
    }

    #[test]
    fn test_sp3_allows_immutable_service() {
        let rule = JAVA_SP3Rule::new();
        let clean = r#"
@Service
public class MyService {
    private final int counter = 0;
    public int getCounter() { return counter; }
}
"#;
        let issues = with_java_context(clean, "MyService.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag immutable @Service");
    }
}
