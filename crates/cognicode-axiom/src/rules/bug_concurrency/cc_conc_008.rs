//! CC_CONC008: Concurrent map access without synchronization
//!
//! Detects when HashMap or BTreeMap is accessed from multiple threads
//! without proper synchronization, which can cause data races.
//!
//! # Problem
//! HashMap and BTreeMap are not thread-safe. Accessing them concurrently
//! from multiple threads without synchronization causes data races and
//! undefined behavior.
//!
//! # Fix
//! Use DashMap for concurrent access, wrap HashMap/BTreeMap in Mutex or
//! RwLock, or use the thread-safe equivalents from other crates.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};

/// CC_CONC008 Rule: Concurrent map access without synchronization
pub struct ConcurrentMapAccessRule;

impl Default for ConcurrentMapAccessRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for ConcurrentMapAccessRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CONC008")
    }

    fn name(&self) -> &'static str {
        "Concurrent map access without synchronization"
    }

    fn description(&self) -> &'static str {
        "HashMap or BTreeMap is accessed concurrently without synchronization. \
         These types are not thread-safe. Use DashMap, Mutex<HashMap>, or \
         RwLock<HashMap> for concurrent access."
    }

    fn category(&self) -> Category {
        Category::Correctness
    }

    fn severity(&self) -> Severity {
        Severity::Critical
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Rust]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Check for HashMap/BTreeMap imports
        let has_hashmap = source.contains("HashMap") || source.contains("BTreeMap");
        let has_thread = source.contains("thread::spawn") || source.contains("std::thread") || source.contains("Arc");

        if !has_hashmap || !has_thread {
            return issues;
        }

        // Check if proper synchronization is used
        let has_dashmap = source.contains("DashMap");
        let has_mutex_map = source.contains("Mutex<HashMap>") || source.contains("Mutex<BTreeMap>");
        // Check for RwLock::new(HashMap or RwLock<HashMap> pattern
        let has_rwlock_map = source.contains("RwLock<HashMap>")
            || source.contains("RwLock<BTreeMap>")
            || (source.contains("RwLock::new") && source.contains("HashMap"));
        let has_arc_map = source.contains("Arc<HashMap") || source.contains("Arc<BTreeMap");

        if has_dashmap || has_mutex_map || has_rwlock_map || has_arc_map {
            return issues; // Proper synchronization detected
        }

        // Report issues for HashMap/BTreeMap in concurrent context
        for line in source.lines().enumerate() {
            let (line_num, line_str) = line;
            let trimmed = line_str.trim();

            if trimmed.contains("HashMap") && !trimmed.starts_with("//") && !trimmed.contains("use ") == false {
                // Check if this is a use statement or declaration
                if trimmed.contains("use") && (trimmed.contains("HashMap") || trimmed.contains("BTreeMap")) {
                    issues.push(Issue::new(
                        "CC_CONC008",
                        "Concurrent map access without synchronization",
                        Severity::Critical,
                        Category::Correctness,
                        ctx.file_path.to_string_lossy(),
                        line_num + 1,
                        0,
                        "HashMap is not thread-safe and is being used in a \
                         multi-threaded context. Use DashMap, Mutex<HashMap>, \
                         or RwLock<HashMap> instead.",
                    ));
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["HashMap", "BTreeMap", "thread", "Arc", "Mutex", "RwLock"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_code(code: &str) -> (tree_sitter::Tree, String) {
        let lang = SrcLanguage::Rust.to_ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(code, None).unwrap();
        (tree, code.to_string())
    }

    fn check_rule(code: &str) -> Vec<Issue> {
        let (tree, source) = parse_code(code);
        let metrics = crate::types::FileMetrics::default();
        let ctx = RuleContext::new(
            &tree,
            &source,
            std::path::Path::new("test.rs"),
            &SrcLanguage::Rust,
            &metrics,
        );
        let rule = ConcurrentMapAccessRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_hashmap_in_thread_context() {
        let code = r#"
use std::collections::HashMap;
use std::thread;
use std::sync::Arc;

fn main() {
    let shared_map = Arc::new(HashMap::new());

    let handle = thread::spawn({
        let map = shared_map.clone();
        move || {
            map.insert("key", 42);
        }
    });

    handle.join().unwrap();
    shared_map.get("key");
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect HashMap in thread context");
        assert_eq!(issues[0].rule_id, "CC_CONC008");
    }

    #[test]
    fn test_no_false_positive_dashmap() {
        let code = r#"
use std::collections::HashMap;
use dashmap::DashMap;
use std::thread;

fn correct_concurrent_map() {
    let map = DashMap::new();

    let handles: Vec<_> = (0..4).map(|_| {
        let map_clone = map.clone();
        thread::spawn(move || {
            map_clone.insert("key", 42);
        })
    }).collect();

    for h in handles { h.join().unwrap(); }
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag DashMap");
    }

    #[test]
    fn test_no_false_positive_rwlock_map() {
        let code = r#"
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;

fn locked_hashmap() {
    let map = Arc::new(RwLock::new(HashMap::new()));

    let handles: Vec<_> = (0..4).map(|_| {
        let m = map.clone();
        thread::spawn(move || {
            let mut map = m.write().unwrap();
            map.insert("key", 42);
        })
    }).collect();

    for h in handles { h.join().unwrap(); }
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag RwLock<HashMap>");
    }

    #[test]
    fn test_no_false_positive_single_thread() {
        let code = r#"
use std::collections::HashMap;

fn single_thread() {
    let mut map = HashMap::new();
    map.insert("key", 42);
    println!("{:?}", map.get("key"));
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag single-threaded HashMap");
    }
}
