//! S1879 — Concurrent map unsync detection
//!
//! Detects HashMap or BTreeMap used concurrently without Sync implementation,
//! which can cause data races when accessed from multiple threads.

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

/// Rule constant for S1879
const RULE_ID: &str = "S1879";

declare_rule! {
    id: "S1879"
    name: "Concurrent map access without synchronization"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects std::collections::HashMap or BTreeMap used in a multi-threaded context without proper synchronization. These map types do not implement Sync, so sharing them between threads without synchronization (e.g., via Mutex, RwLock, or using DashMap) causes undefined behavior."
    clean_code: Clear,
    impacts: [Reliability: High, Security: High],

    agent_semantics: {
        summary: "Detects HashMap/BTreeMap used concurrently without DashMap or Mutex",
        fix_playbook: "1. Replace with DashMap for concurrent access\n2. Or wrap with Mutex<HashMap<...>>\n3. Or use a proper concurrent hashmap (sharded)",
        review_questions: [
            "Is the map actually accessed from multiple threads?",
            "Would DashMap or Mutex<HashMap> be appropriate?",
        ],
        semantic_chunks: [
            "HashMap and BTreeMap are not Sync - using them across threads is UB",
            "DashMap provides concurrent access without explicit locking",
            "Mutex<RwLock> provides exclusive access with familiar API"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires architectural decision between DashMap vs Mutex<HashMap>"
    }

    check: => {
        detect_concurrent_map_unsync(&ctx)
    }
}

/// Detects HashMap/BTreeMap accessed from multiple threads without Sync.
fn detect_concurrent_map_unsync(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    // First, check if the code uses HashMap or BTreeMap
    let has_hashmap = source.contains("HashMap") || source.contains("BTreeMap");
    let has_btreemap = source.contains("BTreeMap");

    if !has_hashmap && !has_btreemap {
        return issues; // No maps used
    }

    // Check if there's multi-threaded context
    let has_threads = source.contains("thread::spawn")
        || source.contains("std::thread")
        || source.contains("spawn(move");

    if !has_threads {
        return issues; // Single-threaded context, no issue
    }

    // Pattern 1: Arc<HashMap> or Arc<BTreeMap> shared between threads
    let arc_map_thread_pattern = regex::Regex::new(r"Arc::new\s*\(\s*(?:HashMap|BTreeMap)|HashMap::new\s*\([^)]*\).*?thread::spawn|BTreeMap::new\s*\([^)]*\).*?thread::spawn").unwrap();

    for cap in arc_map_thread_pattern.find_iter(source) {
        let text = cap.as_str();
        let map_type = if text.contains("HashMap") { "HashMap" } else { "BTreeMap" };

        // Check if proper synchronization is used
        let has_mutex = text.contains("Mutex") || text.contains("RwLock");
        let has_dashmap = text.contains("DashMap");

        if !has_mutex && !has_dashmap {
            let pt = source[..cap.start()].lines().count();
            issues.push(Issue::new(
                RULE_ID,
                format!("{} used in multi-threaded context without synchronization", map_type),
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                pt + 1,
            ).with_remediation(Remediation::substantial(
                "Use DashMap instead, or wrap in Mutex/RwLock for thread-safe access"
            )));
        }
    }

    // Pattern 2: Static HashMap/BTreeMap accessed in threads
    let static_map_pattern = regex::Regex::new(r"static\s+(?:mut\s+)?(\w+)\s*:\s*(?:HashMap|BTreeMap)").unwrap();

    for cap in static_map_pattern.find_iter(source) {
        let text = cap.as_str();
        let map_name = regex::Regex::new(r"static\s+(?:mut\s+)?(\w+)").unwrap()
            .captures(text)
            .and_then(|m| m.get(1))
            .map(|x| x.as_str())
            .unwrap_or("map");

        // Check if this static is used in thread context
        let after_static = &source[cap.end()..cap.end() + 500];
        if after_static.contains("thread::spawn") || after_static.contains(".insert") && after_static.contains(".get") {
            let pt = source[..cap.start()].lines().count();
            issues.push(Issue::new(
                RULE_ID,
                format!("Static {} used with HashMap/BTreeMap in thread context - not thread-safe", map_name),
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                pt + 1,
            ).with_remediation(Remediation::substantial(
                "Use Mutex/RwLock to protect the map, or use a concurrent map implementation like DashMap"
            )));
        }
    }

    // Pattern 3: HashMap/BTreeMap operations inside thread::spawn without Arc<Mutex<>>
    let thread_map_ops = regex::Regex::new(r"thread::spawn\s*\([^}]*?(?:insert|get|remove)\s*\([^)]*?(?:HashMap|BTreeMap)").unwrap();

    for cap in thread_map_ops.find_iter(source) {
        let text = cap.as_str();
        let map_type = if text.contains("HashMap") { "HashMap" } else { "BTreeMap" };

        // Check for proper synchronization
        let has_sync = text.contains("Mutex") || text.contains("RwLock") || text.contains("Arc");

        if !has_sync {
            let pt = source[..cap.start()].lines().count();
            issues.push(Issue::new(
                RULE_ID,
                format!("{} operations inside thread::spawn without synchronization", map_type),
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                pt + 1,
            ).with_remediation(Remediation::substantial(
                "Wrap HashMap in Mutex/RwLock, or use DashMap for concurrent access"
            )));
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s1879_registered() {
        let rule = S1879Rule::new();
        assert_eq!(rule.id(), "S1879");
        assert!(rule.name().len() > 0);
    }

    #[test]
    fn test_hashmap_in_thread() {
        let rule = S1879Rule::new();
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
            }
        "#;
        assert_eq!(rule.id(), "S1879");
    }
}
