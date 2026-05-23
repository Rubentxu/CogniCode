//! S1872 — Race condition detection rules
//!
//! S1872a: Detects shared mutable state accessed from multiple threads without synchronization
//! S1872b: Detects Arc::clone() in hot paths without proper benchmarking

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

// ─────────────────────────────────────────────────────────────────────────────
// S1872a — Race condition: shared mutable state without synchronization
// ─────────────────────────────────────────────────────────────────────────────

/// Rule constant for S1872a
const RULE_ID_S1872A: &str = "S1872a";

declare_rule! {
    id: "S1872a"
    name: "Race condition: shared mutable state without synchronization"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects shared mutable state (static mut, Rc<RefCell<>>) accessed from multiple threads without proper synchronization primitives like Mutex, RwLock, or atomic types."
    clean_code: Clear,
    impacts: [Reliability: High, Security: High],

    agent_semantics: {
        summary: "Detects shared mutable state captured by thread/spawn without synchronization",
        fix_playbook: "1. Identify the shared variable\n2. Wrap with Mutex<T> or RwLock<T>\n3. Use .lock().unwrap() or .write().unwrap() to access\n4. For Arc<T>, ensure all clones are properly managed",
        review_questions: [
            "Is the shared state genuinely accessed from multiple threads?",
            "Could this be intentional thread-local storage?",
        ],
        semantic_chunks: [
            "Race conditions occur when shared mutable state is accessed concurrently without synchronization",
            "static mut is inherently unsafe in multi-threaded contexts",
            "Rc<RefCell> is not thread-safe - use Arc<Mutex> or Arc<RwLock>"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires understanding of data flow and ownership patterns"
    }

    check: => {
        let mut issues = Vec::new();

        // Pattern 1: static mut variables
        issues.extend(detect_static_mut(&ctx));

        // Pattern 2: Rc<RefCell<>> used across thread boundaries
        issues.extend(detect_rc_refcell_across_threads(&ctx));

        issues
    }
}

/// Detect `static mut` variables which are inherently unsafe for concurrency.
///
/// # Arguments
/// * `ctx` - The rule context containing source code and AST
///
/// # Returns
/// Vector of detected issues
fn detect_static_mut(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();

    // Simple text-based detection for static mut
    for (idx, line) in ctx.source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("static mut") {
            issues.push(Issue::new(
                RULE_ID_S1872A,
                "Shared mutable state: 'static mut' variable is not thread-safe",
                Severity::Critical,
                Category::Bug,
                ctx.file_path,
                idx + 1,
            ).with_remediation(Remediation::substantial(
                "Use std::sync::Mutex or std::sync::atomic types for thread-safe shared state"
            )));
        }
    }

    issues
}

/// Detect `Rc<RefCell<>>` patterns used across thread boundaries.
///
/// Rc is not thread-safe; when combined with RefCell and used across threads,
/// it creates a data race window.
///
/// # Arguments
/// * `ctx` - The rule context containing source code and AST
///
/// # Returns
/// Vector of detected issues
fn detect_rc_refcell_across_threads(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();

    // Check for Rc<RefCell<>> in thread context
    let has_rc_refcell = ctx.source.contains("Rc::new") && ctx.source.contains("RefCell");
    let has_threads = ctx.source.contains("thread::spawn") || ctx.source.contains("std::thread");

    if has_rc_refcell && has_threads {
        // Find line with Rc::new in thread spawn context
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("Rc::new") && line.contains("RefCell") {
                // Check if this is in a thread context (look for thread::spawn nearby)
                let context_start = idx.saturating_sub(5);
                let context_end = (idx + 5).min(ctx.source.lines().count());
                let context: String = ctx.source.lines().skip(context_start).take(context_end - context_start).collect();

                if context.contains("thread::spawn") || context.contains("spawn(") {
                    issues.push(Issue::new(
                        RULE_ID_S1872A,
                        "Shared mutable state: Rc<RefCell<>> used in thread spawn - not thread-safe",
                        Severity::Critical,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Replace Rc<RefCell<>> with Arc<Mutex<>> or Arc<RwLock<>> for thread-safe shared state"
                    )));
                }
            }
        }
    }

    issues
}

// ─────────────────────────────────────────────────────────────────────────────
// S1872b — Arc::clone() in hot paths without proper benchmarking
// ─────────────────────────────────────────────────────────────────────────────

/// Rule constant for S1872b
const RULE_ID_S1872B: &str = "S1872b";

declare_rule! {
    id: "S1872b"
    name: "Race condition: Arc::clone() in hot path without benchmarking"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "Detects Arc::clone() calls inside loops or frequently-called functions, which may cause performance issues due to atomic reference counting overhead without proper benchmarking justification."
    clean_code: Clear,
    impacts: [Maintainability: Low, Reliability: Medium],

    agent_semantics: {
        summary: "Detects Arc<T> used with interior mutability without proper synchronization",
        fix_playbook: "1. Replace Arc<T> with Arc<Mutex<T>> or Arc<RwLock<T>>\n2. Access data through the lock guard\n3. Consider using send+Sync traits if implementing custom types",
        review_questions: [
            "Is this truly a data race or intentional message passing?",
            "Is Arc<Mutex<T>> the right choice vs channels?",
        ],
        semantic_chunks: [
            "Arc::clone involves atomic reference counting overhead",
            "Hot paths with frequent clones can become performance bottlenecks",
            "Consider passing &Arc<T> instead of cloning when possible"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires architectural understanding of concurrency model"
    }

    check: => {
        detect_arc_clone_in_hot_paths(&ctx)
    }
}

/// Detects Arc::clone() inside loops or frequently-called functions.
///
/// Arc::clone() involves atomic operations which are significantly slower
/// than regular reference counting. In hot paths (loops, recursive functions),
/// this can become a bottleneck.
///
/// # Arguments
/// * `ctx` - The rule context containing source code and AST
///
/// # Returns
/// Vector of detected issues
fn detect_arc_clone_in_hot_paths(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    // Find for loops that contain Arc::clone
    let for_loop_pattern = regex::Regex::new(r"(?s)for\s+.*?\{[^}]*?Arc::clone\(\)[^}]*?\}").unwrap();

    for cap in for_loop_pattern.find_iter(source) {
        let pt = source[..cap.start()].lines().count();
        issues.push(Issue::new(
            RULE_ID_S1872B,
            "Arc::clone() detected inside a for loop - potential performance issue",
            Severity::Critical,
            Category::Bug,
            ctx.file_path,
            pt + 1,
        ).with_remediation(Remediation::moderate(
            "Clone Arc outside the loop if possible, or benchmark to confirm clone overhead is acceptable"
        )));
    }

    // Also detect in recursive functions
    let recursive_arc_clone = regex::Regex::new(r"(?s)(fn\s+\w+[^}]*?Arc::clone\(\)[^}]*?\w+\(\))").unwrap();

    for cap in recursive_arc_clone.find_iter(source) {
        let text = cap.as_str();
        // Check if this looks like a recursive call
        if text.contains("move ||") || text.contains("spawn") {
            continue; // Skip closure patterns
        }

        let fn_name_match = regex::Regex::new(r"fn\s+(\w+)").unwrap();
        if let Some(m) = fn_name_match.captures(text) {
            let fn_name = m.get(1).map(|x| x.as_str()).unwrap_or("");
            let pt = source[..cap.start()].lines().count();

            // Simple heuristic: if function calls itself
            if text.matches(&format!("{}()", fn_name)).count() >= 2 {
                issues.push(Issue::new(
                    RULE_ID_S1872B,
                    format!("Arc::clone() detected in recursive function '{}' - potential performance issue", fn_name),
                    Severity::Critical,
                    Category::Bug,
                    ctx.file_path,
                    pt + 1,
                ).with_remediation(Remediation::moderate(
                    "Consider restructuring to avoid clones in recursion, or benchmark to confirm overhead is acceptable"
                )));
            }
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s1872a_registered() {
        let rule = S1872aRule::new();
        assert_eq!(rule.id(), "S1872a");
        assert!(rule.name().len() > 0);
    }

    #[test]
    fn test_s1872b_registered() {
        let rule = S1872bRule::new();
        assert_eq!(rule.id(), "S1872b");
        assert!(rule.name().len() > 0);
    }
}
