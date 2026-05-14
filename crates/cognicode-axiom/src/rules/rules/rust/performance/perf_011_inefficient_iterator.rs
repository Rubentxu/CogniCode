//! PERF_011 — Inefficient Iterator Usage
//!
//! Detects unnecessary collect() followed by iteration, or collecting
//! to Vec when iteration would suffice.

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;

/// Rule constant for PERF_011
const RULE_ID: &str = "PERF_011";

declare_rule! {
    id: "PERF_011"
    name: "Unnecessary collect() followed by iteration"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Detects cases where .collect::<Vec<_>>() is followed by iteration over the same data. This creates an intermediate allocation when iterator methods could be used directly."
    clean_code: Clear,
    impacts: [Maintainability: Medium],

    agent_semantics: {
        summary: "Detects unnecessary collect() before iteration",
        fix_playbook: "1. Chain iterator methods directly\n2. Iterate over the source without collecting\n3. Keep collect if Vec is needed for multi-pass or return type",
        review_questions: [
            "Is the Vec actually needed for indexing or multi-pass?",
            "Could iterator methods be chained directly?",
            "Is the intermediate collection providing clarity?"
        ],
        semantic_chunks: [
            "collect() creates an intermediate Vec allocation",
            "Iterator methods can often replace collect + iteration",
            "Sometimes Vec is genuinely needed - don't over-optimize"
        ],
        safe_autofix: false,
        autofix_guidance: "Cannot safely autofix - requires understanding whether Vec is needed"
    }

    check: => {
        detect_inefficient_iterator(&ctx)
    }
}

/// Detects unnecessary collect before iteration.
fn detect_inefficient_iterator(ctx: &RuleContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    let source = ctx.source;

    // Find .collect::<Vec<_>>() patterns
    let collect_re = regex::Regex::new(r"\.collect::<Vec<[^>]+>>\s*\(\s*\)").unwrap();

    for collect_cap in collect_re.find_iter(source) {
        let collect_start = collect_cap.start();

        // Get the statement/line containing the collect
        let line_start = source[..collect_start].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line_end = source[collect_start..]
            .find('\n')
            .map(|p| collect_start + p)
            .unwrap_or(source.len());
        let collect_line = &source[line_start..line_end];

        // Check if the next few lines contain a for loop over a similar variable
        let rest_of_file = &source[collect_end(collect_cap.as_str(), collect_start)..];
        let next_lines = &rest_of_file[..rest_of_file.char_indices()
            .nth(10)
            .map(|(i, _)| i)
            .unwrap_or(rest_of_file.len())
            .min(500)];

        // Pattern: let x = ...collect(); for ... in &x { ... }
        let for_loop_re = regex::Regex::new(r"for\s+\w+\s+in\s+&(\w+)\s*\{").unwrap();
        let for_loop_re2 = regex::Regex::new(r"for\s+\w+\s+in\s+(\w+)\.iter\(\)\s*\{").unwrap();

        // Extract variable name from collect
        let var_re = regex::Regex::new(r"let\s+(\w+)\s*=\s*").unwrap();
        if let Some(var_cap) = var_re.captures(collect_line) {
            let var_name = var_cap.get(1).map(|m| m.as_str()).unwrap_or("");

            if next_lines.contains(&format!("in &{}", var_name))
                || next_lines.contains(&format!("in {}.iter()", var_name))
            {
                let line_num = source[..collect_start].lines().count();
                issues.push(Issue::new(
                    RULE_ID,
                    "Unnecessary collect() followed by iteration - iterator methods could be used directly",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Chain iterator methods directly instead of collecting to Vec"
                )));
            }
        }
    }

    issues
}

fn collect_end(s: &str, start: usize) -> usize {
    start + s.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_011_registered() {
        let rule = PERF_011Rule::new();
        assert_eq!(rule.id(), "PERF_011");
        assert!(rule.name().len() > 0);
    }
}
