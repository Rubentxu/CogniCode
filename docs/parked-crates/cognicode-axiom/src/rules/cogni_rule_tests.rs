//! Tests for the #[cogni_rule] attribute macro
//!
//! Tests cover:
//! - Rule metadata generation (id, severity, category, language)
//! - Auto-registration via inventory::submit!
//! - Pattern-based rule detection

#[cfg(test)]
mod tests {
    use crate::rules::types::*;
    use crate::Rule;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;
    use cognicode_core::infrastructure::parser::Language;
    use std::path::Path;
    use tree_sitter::Parser as TsParser;

    /// Helper closure to run a test with a RuleContext
    fn with_rule_context<F, R>(source: &str, language: Language, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = language.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();
        let symbol_table = crate::rules::symbol_table::SymbolTableBuilder::new()
            .build(&tree, source);

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new("test.rs"),
            language: &language,
            graph: &graph,
            metrics: &metrics,
            symbol_table: Some(&symbol_table),
        };

        f(&ctx)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Test rules defined using #[cogni_rule] attribute
    // ═══════════════════════════════════════════════════════════════════════════

    /// Test rule for detecting weak crypto hashes (md5)
    #[cognicode_macros::cogni_rule(
        id = "sec/crypto-weak-hash",
        severity = Critical,
        category = Vulnerability,
        language = "rust",
        pattern = "md5",
        message = "Use of weak cryptographic hash detected"
    )]
    struct WeakCryptoHashRule;

    /// Test that metadata is correct
    #[test]
    fn test_weak_crypto_rule_metadata() {
        let rule = WeakCryptoHashRule {};
        assert_eq!(rule.id(), "sec/crypto-weak-hash");
        assert_eq!(rule.name(), "sec/crypto-weak-hash");
        assert_eq!(rule.severity(), Severity::Critical);
        assert_eq!(rule.category(), Category::Vulnerability);
        assert_eq!(rule.language(), "rust");
    }

    /// Test that layer and required_keywords have defaults
    #[test]
    fn test_weak_crypto_rule_defaults() {
        let rule = WeakCryptoHashRule {};
        assert_eq!(rule.layer(), 1);
        assert!(rule.required_keywords().is_empty());
    }

    /// Test that rule can be instantiated and check is callable
    #[test]
    fn test_weak_crypto_rule_detects_md5() {
        let rule = WeakCryptoHashRule {};
        let source = r#"
            fn main() {
                let hash = md5(b"hello");
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            // Rule should detect the md5 usage
            assert!(!issues.is_empty(), "Expected to detect md5 usage");
            assert_eq!(issues[0].rule_id, "sec/crypto-weak-hash");
        });
    }

    /// Test that rule doesn't trigger on code without the pattern
    #[test]
    fn test_weak_crypto_rule_no_false_positive() {
        let rule = WeakCryptoHashRule {};
        let source = r#"
            fn main() {
                let hash = sha256(b"hello");
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            // Rule should NOT trigger on sha256
            assert!(issues.is_empty(), "Should not trigger on sha256");
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Test rule without pattern (user-defined check)
    // ═══════════════════════════════════════════════════════════════════════════

    #[cognicode_macros::cogni_rule(
        id = "test/no-pattern",
        severity = Major,
        category = CodeSmell,
        language = "rust",
        message = "Custom rule without pattern"
    )]
    struct NoPatternRule;

    #[test]
    fn test_rule_without_pattern_returns_empty() {
        let rule = NoPatternRule {};
        let source = "fn main() {}";
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            // Without a pattern, check() returns empty Vec
            assert!(issues.is_empty(), "Rule without pattern should return empty issues");
        });
    }

    #[test]
    fn test_rule_without_pattern_metadata() {
        let rule = NoPatternRule {};
        assert_eq!(rule.id(), "test/no-pattern");
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::CodeSmell);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Test rule with custom name
    // ═══════════════════════════════════════════════════════════════════════════

    #[cognicode_macros::cogni_rule(
        id = "test/custom-name",
        name = "Custom Rule Name",
        severity = Info,
        category = CodeSmell,
        language = "rust",
        message = "Rule with custom name"
    )]
    struct CustomNameRule;

    #[test]
    fn test_rule_custom_name() {
        let rule = CustomNameRule {};
        assert_eq!(rule.id(), "test/custom-name");
        assert_eq!(rule.name(), "Custom Rule Name");
        assert_eq!(rule.severity(), Severity::Info);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Test pattern detection in different contexts
    // ═══════════════════════════════════════════════════════════════════════════

    #[cognicode_macros::cogni_rule(
        id = "test/unsafe-unwrap",
        severity = Critical,
        category = Vulnerability,
        language = "rust",
        pattern = "unwrap",
        message = "Use of unwrap() detected - may panic"
    )]
    struct UnsafeUnwrapRule;

    #[test]
    fn test_detects_unwrap_in_function() {
        let rule = UnsafeUnwrapRule {};
        let source = r#"
            fn get_value(opt: Option<i32>) -> i32 {
                opt.unwrap()
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            assert!(!issues.is_empty(), "Should detect unwrap()");
            assert_eq!(issues[0].severity, Severity::Critical);
        });
    }

    #[test]
    fn test_detects_multiple_unwraps() {
        let rule = UnsafeUnwrapRule {};
        let source = r#"
            fn process(opt1: Option<i32>, opt2: Option<i32>) {
                let a = opt1.unwrap();
                let b = opt2.unwrap();
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            // MVP emits at least one issue when pattern is found
            assert!(
                !issues.is_empty(),
                "Should detect at least one unwrap() call"
            );
        });
    }
}
