//! Proof-of-Concept rules using #[cogni_rule] attribute macro
//!
//! This module contains 10 simple rules migrated to the new #[cogni_rule] system
//! as a PoC for the rule macro infrastructure.
//!
//! Each rule:
//! - Uses #[cogni_rule] for automatic Rule trait implementation and registration
//! - Detects simple textual patterns in source code
//! - Participates in Layer-0 preflight filtering via required_keywords
//!
//! ## Rules
//! 1. `unwrap` - Use of unwrap() may cause panics
//! 2. `expect` - Use of expect() may cause panics
//! 3. `todo` - TODO markers left in code
//! 4. `panic` - Panic macro usage
//! 5. `dbg` - Debug macro usage in production code
//! 6. `md5` - Weak cryptographic hash
//! 7. `sha1` - Weak cryptographic hash
//! 8. `des` - Weak cryptographic cipher
//! 9. `rc4` - Weak cryptographic cipher
//! 10. `unsafe` - Unsafe block usage
//!
//! ## Preflight Integration
//! All PoC rules now define required_keywords that match their patterns,
//! enabling Layer-0 preflight filtering via Aho-Corasick automaton.

use crate::rules::types::*;
use crate::Rule;
use cognicode_macros::cogni_rule;
use inventory::submit;

// ═══════════════════════════════════════════════════════════════════════════
// PoC Rule 1: Unwrap Detection
// ═══════════════════════════════════════════════════════════════════════════

#[cogni_rule(
    id = "poc/unwrap",
    name = "POC Unwrap Detection",
    severity = Major,
    category = Bug,
    language = "rust",
    pattern = "unwrap",
    required_keywords = ["unwrap"],
    message = "Use of unwrap() detected - may panic at runtime"
)]
pub struct PocUnwrapRule;

impl PocUnwrapRule {
    pub fn new() -> Self {
        Self
    }
}

// Note: #[cogni_rule] generates impl Rule, layer(), and required_keywords()
// Current MVP: required_keywords returns empty vec (limitation)

// ═══════════════════════════════════════════════════════════════════════════
// PoC Rule 2: Expect Detection
// ═══════════════════════════════════════════════════════════════════════════

#[cogni_rule(
    id = "poc/expect",
    name = "POC Expect Detection",
    severity = Major,
    category = Bug,
    language = "rust",
    pattern = "expect",
    required_keywords = ["expect"],
    message = "Use of expect() detected - may panic at runtime"
)]
pub struct PocExpectRule;

impl PocExpectRule {
    pub fn new() -> Self {
        Self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PoC Rule 3: Todo Detection
// ═══════════════════════════════════════════════════════════════════════════

#[cogni_rule(
    id = "poc/todo",
    name = "POC Todo Marker Detection",
    severity = Minor,
    category = CodeSmell,
    language = "rust",
    pattern = "todo",
    required_keywords = ["todo"],
    message = "TODO marker found - unfinished code"
)]
pub struct PocTodoRule;

impl PocTodoRule {
    pub fn new() -> Self {
        Self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PoC Rule 4: Panic Detection
// ═══════════════════════════════════════════════════════════════════════════

#[cogni_rule(
    id = "poc/panic",
    name = "POC Panic Usage Detection",
    severity = Major,
    category = Bug,
    language = "rust",
    pattern = "panic",
    required_keywords = ["panic"],
    message = "panic! macro detected - causes immediate thread termination"
)]
pub struct PocPanicRule;

impl PocPanicRule {
    pub fn new() -> Self {
        Self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PoC Rule 5: Dbg Macro Detection
// ═══════════════════════════════════════════════════════════════════════════

#[cogni_rule(
    id = "poc/dbg",
    name = "POC Debug Macro Detection",
    severity = Minor,
    category = CodeSmell,
    language = "rust",
    pattern = "dbg",
    required_keywords = ["dbg"],
    message = "dbg! macro detected - should not be in production code"
)]
pub struct PocDbgRule;

impl PocDbgRule {
    pub fn new() -> Self {
        Self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PoC Rule 6: MD5 Weak Crypto Detection
// ═══════════════════════════════════════════════════════════════════════════

#[cogni_rule(
    id = "poc/crypto-md5",
    name = "POC MD5 Weak Crypto Detection",
    severity = Critical,
    category = Vulnerability,
    language = "rust",
    pattern = "md5",
    required_keywords = ["md5"],
    message = "Use of MD5 cryptographic hash detected - cryptographically weak"
)]
pub struct PocMd5Rule;

impl PocMd5Rule {
    pub fn new() -> Self {
        Self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PoC Rule 7: SHA1 Weak Crypto Detection
// ═══════════════════════════════════════════════════════════════════════════

#[cogni_rule(
    id = "poc/crypto-sha1",
    name = "POC SHA1 Weak Crypto Detection",
    severity = Major,
    category = Vulnerability,
    language = "rust",
    pattern = "sha1",
    required_keywords = ["sha1"],
    message = "Use of SHA1 cryptographic hash detected - cryptographically weak"
)]
pub struct PocSha1Rule;

impl PocSha1Rule {
    pub fn new() -> Self {
        Self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PoC Rule 8: DES Weak Crypto Detection
// ═══════════════════════════════════════════════════════════════════════════

#[cogni_rule(
    id = "poc/crypto-des",
    name = "POC DES Weak Crypto Detection",
    severity = Critical,
    category = Vulnerability,
    language = "rust",
    pattern = "DES",
    required_keywords = ["DES"],
    message = "Use of DES cipher detected - cryptographically weak"
)]
pub struct PocDesRule;

impl PocDesRule {
    pub fn new() -> Self {
        Self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PoC Rule 9: RC4 Weak Crypto Detection
// ═══════════════════════════════════════════════════════════════════════════

#[cogni_rule(
    id = "poc/crypto-rc4",
    name = "POC RC4 Weak Crypto Detection",
    severity = Critical,
    category = Vulnerability,
    language = "rust",
    pattern = "RC4",
    required_keywords = ["RC4"],
    message = "Use of RC4 cipher detected - cryptographically weak"
)]
pub struct PocRc4Rule;

impl PocRc4Rule {
    pub fn new() -> Self {
        Self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PoC Rule 10: Unsafe Block Detection
// ═══════════════════════════════════════════════════════════════════════════

#[cogni_rule(
    id = "poc/unsafe",
    name = "POC Unsafe Block Detection",
    severity = Major,
    category = SecurityHotspot,
    language = "rust",
    pattern = "unsafe",
    required_keywords = ["unsafe"],
    message = "Unsafe block detected - requires manual verification for memory safety"
)]
pub struct PocUnsafeRule;

impl PocUnsafeRule {
    pub fn new() -> Self {
        Self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Module Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;
    use cognicode_core::infrastructure::parser::Language;
    use std::path::Path;
    use tree_sitter::Parser as TsParser;

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

    // ═══════════════════════════════════════════════════════════════════════
    // Test Rule Registration
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_poc_unwrap_rule_registered() {
        let rule = PocUnwrapRule {};
        assert_eq!(rule.id(), "poc/unwrap");
        assert_eq!(rule.name(), "POC Unwrap Detection");
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Bug);
        assert_eq!(rule.language(), "rust");
        assert_eq!(rule.layer(), 1); // macro default
    }

    #[test]
    fn test_poc_expect_rule_registered() {
        let rule = PocExpectRule {};
        assert_eq!(rule.id(), "poc/expect");
        assert_eq!(rule.severity(), Severity::Major);
    }

    #[test]
    fn test_poc_todo_rule_registered() {
        let rule = PocTodoRule {};
        assert_eq!(rule.id(), "poc/todo");
        assert_eq!(rule.severity(), Severity::Minor);
        assert_eq!(rule.category(), Category::CodeSmell);
    }

    #[test]
    fn test_poc_panic_rule_registered() {
        let rule = PocPanicRule {};
        assert_eq!(rule.id(), "poc/panic");
        assert_eq!(rule.severity(), Severity::Major);
    }

    #[test]
    fn test_poc_dbg_rule_registered() {
        let rule = PocDbgRule {};
        assert_eq!(rule.id(), "poc/dbg");
        assert_eq!(rule.category(), Category::CodeSmell);
    }

    #[test]
    fn test_poc_md5_rule_registered() {
        let rule = PocMd5Rule {};
        assert_eq!(rule.id(), "poc/crypto-md5");
        assert_eq!(rule.severity(), Severity::Critical);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    #[test]
    fn test_poc_sha1_rule_registered() {
        let rule = PocSha1Rule {};
        assert_eq!(rule.id(), "poc/crypto-sha1");
        assert_eq!(rule.severity(), Severity::Major);
    }

    #[test]
    fn test_poc_des_rule_registered() {
        let rule = PocDesRule {};
        assert_eq!(rule.id(), "poc/crypto-des");
        assert_eq!(rule.severity(), Severity::Critical);
    }

    #[test]
    fn test_poc_rc4_rule_registered() {
        let rule = PocRc4Rule {};
        assert_eq!(rule.id(), "poc/crypto-rc4");
        assert_eq!(rule.severity(), Severity::Critical);
    }

    #[test]
    fn test_poc_unsafe_rule_registered() {
        let rule = PocUnsafeRule {};
        assert_eq!(rule.id(), "poc/unsafe");
        assert_eq!(rule.category(), Category::SecurityHotspot);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Test Pattern Detection
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_unwrap_detects_pattern() {
        let rule = PocUnwrapRule {};
        let source = r#"
            fn main() {
                let x = Some(5).unwrap();
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            assert!(!issues.is_empty(), "Should detect unwrap pattern");
            assert_eq!(issues[0].rule_id, "poc/unwrap");
        });
    }

    #[test]
    fn test_unwrap_no_false_positive() {
        let rule = PocUnwrapRule {};
        let source = r#"
            fn main() {
                let x = Some(5).expect("value");
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            // Should NOT detect because source doesn't contain literal "unwrap"
            assert!(issues.is_empty(), "Should not trigger on expect only");
        });
    }

    #[test]
    fn test_md5_detects_weak_crypto() {
        let rule = PocMd5Rule {};
        let source = r#"
            fn hash_data(data: &[u8]) -> String {
                use md5::{Md5, Digest};
                let mut hasher = Md5::new();
                hasher.update(data);
                format!("{:x}", hasher.finalize())
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            assert!(!issues.is_empty(), "Should detect md5 usage");
            assert_eq!(issues[0].rule_id, "poc/crypto-md5");
            assert_eq!(issues[0].severity, Severity::Critical);
        });
    }

    #[test]
    fn test_sha1_detects_weak_crypto() {
        let rule = PocSha1Rule {};
        let source = r#"
            fn hash_data(data: &[u8]) -> String {
                use sha1::{Sha1, Digest};
                let mut hasher = Sha1::new();
                hasher.update(data);
                format!("{:x}", hasher.finalize())
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            assert!(!issues.is_empty(), "Should detect sha1 usage");
        });
    }

    #[test]
    fn test_des_detects_weak_crypto() {
        let rule = PocDesRule {};
        let source = r#"
            fn encrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
                let cipher = DES::new(key);
                cipher.encrypt(data)
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            assert!(!issues.is_empty(), "Should detect DES usage");
        });
    }

    #[test]
    fn test_rc4_detects_weak_crypto() {
        let rule = PocRc4Rule {};
        let source = r#"
            fn decrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
                RC4::new(key).decrypt(data)
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            assert!(!issues.is_empty(), "Should detect RC4 usage");
        });
    }

    #[test]
    fn test_unsafe_detects_unsafe_block() {
        let rule = PocUnsafeRule {};
        let source = r#"
            unsafe {
                let ptr = 0x1234 as *const i32;
                println!("{}", *ptr);
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            assert!(!issues.is_empty(), "Should detect unsafe block");
        });
    }

    #[test]
    fn test_panic_detects_panic_macro() {
        let rule = PocPanicRule {};
        let source = r#"
            fn validate(input: &str) {
                if input.is_empty() {
                    panic!("Input cannot be empty");
                }
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            assert!(!issues.is_empty(), "Should detect panic! macro");
        });
    }

    #[test]
    fn test_todo_detects_todo_macro() {
        let rule = PocTodoRule {};
        let source = r#"
            fn unimplemented() {
                todo!("This function needs implementation");
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            assert!(!issues.is_empty(), "Should detect todo! macro");
        });
    }

    #[test]
    fn test_dbg_detects_dbg_macro() {
        let rule = PocDbgRule {};
        let source = r#"
            fn process(value: i32) {
                dbg!(value);
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            assert!(!issues.is_empty(), "Should detect dbg! macro");
        });
    }

    #[test]
    fn test_expect_detects_expect_pattern() {
        let rule = PocExpectRule {};
        let source = r#"
            fn main() {
                let value = get_option().expect("value required");
            }
        "#;
        with_rule_context(source, Language::Rust, |ctx| {
            let issues = rule.check(ctx);
            assert!(!issues.is_empty(), "Should detect expect pattern");
        });
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Test Layer and required_keywords defaults
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_all_rules_have_layer_1() {
        // All rules return layer 1 (default from macro)
        assert_eq!(PocUnwrapRule {}.layer(), 1);
        assert_eq!(PocExpectRule {}.layer(), 1);
        assert_eq!(PocTodoRule {}.layer(), 1);
        assert_eq!(PocPanicRule {}.layer(), 1);
        assert_eq!(PocDbgRule {}.layer(), 1);
        assert_eq!(PocMd5Rule {}.layer(), 1);
        assert_eq!(PocSha1Rule {}.layer(), 1);
        assert_eq!(PocDesRule {}.layer(), 1);
        assert_eq!(PocRc4Rule {}.layer(), 1);
        assert_eq!(PocUnsafeRule {}.layer(), 1);
    }

    #[test]
    fn test_all_rules_have_required_keywords() {
        // All PoC rules now define required_keywords matching their patterns
        assert_eq!(PocUnwrapRule {}.required_keywords(), vec!["unwrap"]);
        assert_eq!(PocExpectRule {}.required_keywords(), vec!["expect"]);
        assert_eq!(PocTodoRule {}.required_keywords(), vec!["todo"]);
        assert_eq!(PocPanicRule {}.required_keywords(), vec!["panic"]);
        assert_eq!(PocDbgRule {}.required_keywords(), vec!["dbg"]);
        assert_eq!(PocMd5Rule {}.required_keywords(), vec!["md5"]);
        assert_eq!(PocSha1Rule {}.required_keywords(), vec!["sha1"]);
        assert_eq!(PocDesRule {}.required_keywords(), vec!["DES"]);
        assert_eq!(PocRc4Rule {}.required_keywords(), vec!["RC4"]);
        assert_eq!(PocUnsafeRule {}.required_keywords(), vec!["unsafe"]);
    }
}
