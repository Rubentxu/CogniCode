//! P5 — Mutex lock ordering (potential deadlock)
//!
//! Detects nested mu.Lock() calls that could cause deadlocks.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1860"
    name: "Nested mutex.Lock() can cause deadlocks"
    severity: Major
    category: Bug
    language: "Go"
    params: {}

    explanation: "Nested mutex locks without consistent ordering can cause deadlocks. Always acquire locks in the same order.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find nested lock patterns
        let lock_pattern = regex::Regex::new(r"\.Lock\(\)").unwrap();

        let mut lock_positions: Vec<(usize, String)> = Vec::new();
        for cap in lock_pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            // Try to identify which mutex
            let line_start = source[..cap.start()].rfind('\n').map(|p| p + 1).unwrap_or(0);
            let line = source[line_start..cap.start() + 7].trim();
            lock_positions.push((line_num, line.to_string()));
        }

        // Check for locks within locks
        let mut in_lock = false;
        let mut lock_depth = 0;
        let mut max_depth = 0;

        for (line_num, _) in &lock_positions {
            if lock_depth == 0 {
                in_lock = true;
            }
            lock_depth += 1;
            max_depth = max_depth.max(lock_depth);

            // Check if next lock is on a different line (not nested)
            // This is a simplified heuristic
            if line_num + 1 < lock_positions.len() {
                let next_line = lock_positions.iter().find(|(ln, _)| *ln == line_num + 1);
                if next_line.is_none() {
                    lock_depth = 0;
                }
            }
        }

        if max_depth >= 2 {
            issues.push(Issue::new(
                "GO_S1860",
                format!("Nested mutex.Lock() calls detected - potential deadlock"),
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::quick(
                "Ensure locks are always acquired in the same order to prevent deadlocks"
            )));
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;

    fn with_go_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Go.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Go,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_p5_registered() {
        let rule = GO_S1860Rule::new();
        assert_eq!(rule.id(), "GO_S1860");
    }

    #[test]
    fn test_p5_detects_nested_locks() {
        let rule = GO_S1860Rule::new();
        let smelly = r#"
func main() {
    mu1.Lock()
    mu2.Lock()
    mu2.Unlock()
    mu1.Unlock()
}
"#;
        let issues = with_go_context(smelly, "main.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect nested locks");
        assert_eq!(issues[0].rule_id, "GO_S1860");
    }

    #[test]
    fn test_p5_allows_single_lock() {
        let rule = GO_S1860Rule::new();
        let clean = r#"
func main() {
    mu.Lock()
    mu.Unlock()
}
"#;
        let issues = with_go_context(clean, "main.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag single lock");
    }
}
