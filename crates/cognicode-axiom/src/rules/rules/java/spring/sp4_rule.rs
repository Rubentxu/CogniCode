//! SP4 — RestController without ResponseBody
//!
//! Detects @RestController classes missing @ResponseBody.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_SP4"
    name: "@RestController should have @ResponseBody on methods"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "@RestController combines @Controller and @ResponseBody. If a method lacks @ResponseBody, it may return a view instead of JSON/XML.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find @RestController class
        let rest_controller_pattern = regex::Regex::new(r"@RestController\s*(?:public\s+)?class\s+(\w+)").unwrap();

        for cap in rest_controller_pattern.captures_iter(source) {
            if let Some(class_name) = cap.get(1) {
                let class_start = cap.get(0).unwrap().end();
                let class_end = find_class_end(source, class_start);
                let class_body = &source[class_start..class_end];

                // Find public methods without @ResponseBody or @GetMapping/@PostMapping etc.
                let method_pattern = regex::Regex::new(r"(?:public|protected)\s+\w+\s+\w+\s*\([^)]*\)\s*(?:throws\s+\w+)?\s*\{").unwrap();

                for method_cap in method_pattern.captures_iter(class_body) {
                    let method_start = method_cap.get(0).unwrap().start();
                    let method_line_start = class_start + method_start;
                    let preceding = &source[..method_line_start];

                    // Check if @ResponseBody or mapping annotation exists before this method
                    let has_mapping = preceding.contains("@GetMapping")
                        || preceding.contains("@PostMapping")
                        || preceding.contains("@PutMapping")
                        || preceding.contains("@DeleteMapping")
                        || preceding.contains("@PatchMapping")
                        || preceding.contains("@ResponseBody");

                    // Get the method line number
                    let line_num = source[..method_line_start].lines().count() + 1;

                    if !has_mapping {
                        issues.push(Issue::new(
                            "JAVA_SP4",
                            format!("Method in @RestController may be missing @ResponseBody or mapping annotation"),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "Add @ResponseBody or a @*Mapping annotation to the method"
                        )));
                    }
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
    fn test_sp4_registered() {
        let rule = JAVA_SP4Rule::new();
        assert_eq!(rule.id(), "JAVA_SP4");
    }

    #[test]
    fn test_sp4_detects_method_without_mapping() {
        let rule = JAVA_SP4Rule::new();
        let smelly = r#"
@RestController
public class MyController {
    public String getSomething() { return "data"; }
}
"#;
        let issues = with_java_context(smelly, "MyController.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect method without mapping annotation");
        assert_eq!(issues[0].rule_id, "JAVA_SP4");
    }

    #[test]
    fn test_sp4_allows_mapped_method() {
        let rule = JAVA_SP4Rule::new();
        let clean = r#"
@RestController
public class MyController {
    @GetMapping("/something")
    public String getSomething() { return "data"; }
}
"#;
        let issues = with_java_context(clean, "MyController.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag method with mapping annotation");
    }
}
