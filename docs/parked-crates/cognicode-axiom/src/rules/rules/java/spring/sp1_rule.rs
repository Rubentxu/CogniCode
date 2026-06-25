//! SP1 — Autowired field injection
//!
//! Detects @Autowired on fields instead of constructor injection.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_SP1"
    name: "@Autowired field injection should be constructor injection"
    severity: Major
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Field injection via @Autowired is discouraged. Use constructor injection for better testability and immutability.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find @Autowired on fields
        let autowired_field_pattern = regex::Regex::new(r"@Autowired\s+(?:private|public|protected)?\s*(\w+)\s+(\w+)\s*;").unwrap();

        for cap in autowired_field_pattern.captures_iter(source) {
            if let Some(field_type) = cap.get(1) {
                let field_name = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;

                issues.push(Issue::new(
                    "JAVA_SP1",
                    format!("@Autowired field injection '{} {}\n'. Use constructor injection instead.", field_type.as_str(), field_name),
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Inject via constructor: add the field as a constructor parameter"
                )));
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
    fn test_sp1_registered() {
        let rule = JAVA_SP1Rule::new();
        assert_eq!(rule.id(), "JAVA_SP1");
    }

    #[test]
    fn test_sp1_detects_autowired_field() {
        let rule = JAVA_SP1Rule::new();
        let smelly = r#"
@Service
public class MyService {
    @Autowired
    private UserRepository userRepository;
}
"#;
        let issues = with_java_context(smelly, "MyService.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect @Autowired field injection");
        assert_eq!(issues[0].rule_id, "JAVA_SP1");
    }

    #[test]
    fn test_sp1_allows_constructor_injection() {
        let rule = JAVA_SP1Rule::new();
        let clean = r#"
@Service
public class MyService {
    private final UserRepository userRepository;

    @Autowired
    public MyService(UserRepository userRepository) {
        this.userRepository = userRepository;
    }
}
"#;
        let issues = with_java_context(clean, "MyService.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag constructor injection");
    }
}
