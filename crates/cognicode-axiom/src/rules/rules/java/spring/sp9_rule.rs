//! SP9 — JpaRepository method naming convention
//!
//! Detects JpaRepository methods that don't follow naming conventions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_SP9"
    name: "JpaRepository method naming convention violation"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "JpaRepository derived query methods should follow Spring Data naming conventions. Invalid method names will cause runtime exceptions.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find JpaRepository interface
        let repo_pattern = regex::Regex::new(r"extends\s+JpaRepository\s*<").unwrap();

        for cap in repo_pattern.captures_iter(source) {
            let repo_start = cap.get(0).unwrap().end();
            // Find interface body
            let interface_start = source[..cap.get(0).unwrap().start()].rfind("interface").unwrap_or(0);
            let body_start = source[interface_start..].find('{').unwrap_or(0) + interface_start;

            // Look for methods with problematic names
            let method_pattern = regex::Regex::new(r"\b(find|read|get|query|stream|count|exists|delete|remove)\w*\s*\(").unwrap();

            for method_cap in method_pattern.captures_iter(&source[body_start..]) {
                let method_name = method_cap.get(0).unwrap().as_str();
                let line_offset = body_start + method_cap.get(0).unwrap().start();
                let line_num = source[..line_offset].lines().count() + 1;

                // Check for known bad patterns
                let bad_patterns = [
                    "findById",  // Should use Optional<Entity> findById
                    "getById",   // Deprecated, use getReferenceById
                ];

                let is_bad = bad_patterns.iter().any(|p| method_name.contains(p));

                if !is_bad && !method_name.contains("By") {
                    issues.push(Issue::new(
                        "JAVA_SP9",
                        format!("JpaRepository method '{}' may not follow naming conventions", method_name.trim()),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Use Spring Data method naming: findBy{Property}, existsBy{Property}, countBy{Property}"
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
    fn test_sp9_registered() {
        let rule = JAVA_SP9Rule::new();
        assert_eq!(rule.id(), "JAVA_SP9");
    }

    #[test]
    fn test_sp9_detects_bad_naming() {
        let rule = JAVA_SP9Rule::new();
        let smelly = r#"
@Repository
public interface UserRepository extends JpaRepository<User, Long> {
    List<User> findAllData();
}
"#;
        let issues = with_java_context(smelly, "UserRepository.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect invalid method naming");
        assert_eq!(issues[0].rule_id, "JAVA_SP9");
    }

    #[test]
    fn test_sp9_allows_valid_naming() {
        let rule = JAVA_SP9Rule::new();
        let clean = r#"
@Repository
public interface UserRepository extends JpaRepository<User, Long> {
    Optional<User> findById(Long id);
    boolean existsByEmail(String email);
}
"#;
        let issues = with_java_context(clean, "UserRepository.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow valid JpaRepository naming");
    }
}
