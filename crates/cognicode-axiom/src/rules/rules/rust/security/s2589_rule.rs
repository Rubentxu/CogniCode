//! S2589 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

declare_rule! {
    id: "S2589"
    name: "Boolean expressions should not be constant"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Constant boolean expressions in conditions always evaluate to the same result, indicating dead code that should be removed or replaced with meaningful logic.",
    clean_code: Logical,
    impacts: [Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let const_bool_re = regex::Regex::new(r"(if|while)\s*\(?\s*(true|false)\s*\)?\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
        if const_bool_re.is_match(trimmed) {
                issues.push(Issue::new(
                    "S2589",
                    format!("Constant boolean expression at line {}", idx + 1),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Remove the redundant condition or use a meaningful expression")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2757 — Unexpected assignment operators in conditions
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2757"
    name: "Unexpected assignment operators in conditions"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Pattern matches in conditions that look like assignments can confuse developers and lead to unintended behavior due to the difference between = and ==.",
    clean_code: Logical,
    impacts: [Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"if\s+let\s+[A-Z]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue; // Skip comments and disabled code
            }
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S2757",
                    "Potentially unintended pattern match in condition",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1313 — IP addresses should not be hardcoded
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1313"
    name: "IP addresses should not be hardcoded"
    severity: Minor
    category: SecurityHotspot
    language: "*"
    params: {}

    explanation: "Hardcoded IP addresses make applications inflexible and difficult to deploy in different environments, reducing configurability and portability.",
    clean_code: Focused,
    impacts: [Security: Low, Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#""([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])"(?![0-9])"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") 
            || trimmed.starts_with("///") || trimmed.starts_with("//!")
            || trimmed.starts_with("/*") || trimmed.starts_with("*")
            || trimmed.starts_with("#")
            || trimmed.contains("version") || trimmed.contains("Version")
            || trimmed.contains("coordinate") || trimmed.contains("Coordinate")
            { continue; }
            
            if let Some(m) = re.find(trimmed) {
                issues.push(Issue::new(
                    "S1313",
                    format!("Hardcoded IP address: {}", m.as_str()),
                    Severity::Minor,
                    Category::SecurityHotspot,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1141 — Error handling should not be deeply nested
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1141"
    name: "Error handling should not be deeply nested"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Deeply nested error handling with multiple match arms or Result types indicates complex control flow that could be simplified using the ? operator.",
    clean_code: Clear,
    impacts: [Maintainability: Low, Reliability: Low],
    check: => {
        let mut issues = Vec::new();
        let query_str = "(match_expression pattern: (identifier) @pat (#any-of? @pat \"Err\" \"Ok\" \"Some\" \"None\")) @match";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let depth = ctx.nesting_depth(capture.node);
                    if depth > 3 {
                        let pt = capture.node.start_position();
                        issues.push(Issue::new(
                            "S1141",
                            "Deeply nested error handling - consider using ? operator or extracting to function",
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            pt.row + 1,
                        ));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1994 — Loop counters should not be modified inside the loop
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1994"
    name: "Loop counters should not be modified inside the loop"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "Modifying loop counters inside the loop body leads to unpredictable behavior and hard-to-debug issues as the loop termination condition becomes unreliable.",
    clean_code: Logical,
    impacts: [Reliability: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s+(\w+)\s+in\s+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let counter = cap.get(1).unwrap().as_str();
                let body_start = idx + 1;
                for (body_idx, body_line) in ctx.source.lines().skip(body_start).enumerate() {
                    if body_line.contains(&format!("{} =", counter)) || body_line.contains(&format!("{} +=", counter)) {
                        issues.push(Issue::new(
                            "S1994",
                            format!("Loop counter '{}' modified inside loop", counter),
                            Severity::Critical,
                            Category::Bug,
                            ctx.file_path,
                            body_start + body_idx + 1,
                        ));
                    }
                    if body_line.trim() == "}" { break; }
                }
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s2589_registered() {
        let rule = S2589Rule::new();
        assert_eq!(rule.id(), "S2589");
        assert!(rule.name().len() > 0);
    }
}
