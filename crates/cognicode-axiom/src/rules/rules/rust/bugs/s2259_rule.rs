//! S2259 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S2259"
    name: "Null pointer dereferences should be avoided"
    severity: Blocker
    category: Bug
    language: "rust"
    params: {}

    explanation: "Raw pointer dereferences without verification can cause undefined behavior including crashes, memory corruption, or security vulnerabilities.",
    clean_code: Logical,
    impacts: [Reliability: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:\(?\s*\*\s*(\w+)\s*\)\s*\.\s*\w+|\s*\*\s*(\w+)\s*(?:[^\w]|$))").unwrap();
        let mut unsafe_depth = 0;
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("unsafe{") || line.contains("unsafe {") { unsafe_depth += 1; }
            if unsafe_depth > 0 && re.is_match(line) {
                issues.push(Issue::new("S2259", "Raw pointer dereference in unsafe block - verify non-null", Severity::Blocker, Category::Bug, ctx.file_path, idx + 1));
            }
            if line.trim() == "}" && unsafe_depth > 0 { unsafe_depth -= 1; }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S115 — Constant names should follow UPPER_CASE
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S115"
    name: "Constant names should follow UPPER_CASE convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Constant names not following UPPER_CASE convention reduce code readability and make it harder to distinguish constants from variables.",
    clean_code: Efficient,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"const\s+([a-z][A-Za-z0-9_]*)\s*:").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                if name != name.to_uppercase() {
                    issues.push(Issue::new("S115", format!("Constant '{}' should be UPPER_CASE", name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1541 — High cyclomatic complexity (simplified)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1541"
    name: "Functions should not have too many branches"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { threshold: usize = 10 }

    explanation: "Functions with high cyclomatic complexity are difficult to test thoroughly and maintain, often indicating the need for refactoring into smaller functions.",
    clean_code: Focused,
    impacts: [Maintainability: Medium, Reliability: Low],
    check: => {
        let mut issues = Vec::new();
        for func_node in ctx.query_functions() {
            let mut branch_count = 0;
            crate::rules::helpers::count_branches_impl(func_node, &mut branch_count);
            if branch_count > self.threshold {
                let pt = func_node.start_position();
                if let Some(name) = ctx.function_name(func_node) {
                    issues.push(Issue::new("S1541", format!("Function '{}' has {} branches", name, branch_count), Severity::Major, Category::CodeSmell, ctx.file_path, pt.row + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1142 — Too many return statements
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1142"
    name: "Functions should not have too many return points"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { max_returns: usize = 3 }

    explanation: "Functions with too many return points are harder to understand and trace through, increasing the risk of logic errors during maintenance.",
    clean_code: Clear,
    impacts: [Maintainability: Medium, Reliability: Low],
    check: => {
        let mut issues = Vec::new();
        for func_node in ctx.query_functions() {
            let text = func_node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
            let return_count = text.matches("return ").count() + text.matches("return;").count();
            if return_count > self.max_returns {
                let pt = func_node.start_position();
                if let Some(name) = ctx.function_name(func_node) {
                    issues.push(Issue::new(
                        "S1142",
                        format!("Function '{}' has {} return statements", name, return_count),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        pt.row + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1151 — Match arm too big
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1151"
    name: "Match arms should not be too long"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: { max_lines: usize = 5 }

    explanation: "Match arms that span many lines indicate complex branching logic that could be extracted into separate functions for better readability.",
    clean_code: Focused,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let query_str = "(match_arm) @arm";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let lines = ctx.line_count(capture.node);
                    if lines > self.max_lines {
                        let pt = capture.node.start_position();
                        issues.push(Issue::new(
                            "S1151",
                            format!("Match arm is {} lines - consider extracting to function", lines),
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
// S1155 — Use .is_empty() instead of .len() == 0
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1155"
    name: "Use .is_empty() instead of comparing .len() to 0"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Using .is_empty() is more idiomatic and clear than comparing .len() to 0, improving code readability and consistency.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.len\(\)\s*==\s*0").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1155",
                    "Use .is_empty() instead of .len() == 0",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace with .is_empty()")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1158 — Redundant .clone() after .to_owned()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1158"
    name: "Unnecessary .clone() calls should be removed"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: ".to_owned() already creates an owned copy, so calling .clone() immediately after is redundant and wastes memory.",
    clean_code: Efficient,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.to_owned\(\)\s*\.clone\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1158",
                    "Redundant .clone() after .to_owned()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1161 — #[allow(deprecated)] should not be used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1161"
    name: "Deprecated code should not be used"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "#[allow(deprecated)] suppresses warnings about using deprecated APIs, preventing developers from migrating to supported alternatives.",
    clean_code: Complete,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("#[allow(deprecated)]") {
                issues.push(Issue::new(
                    "S1161",
                    "#[allow(deprecated)] suppresses useful warnings about deprecated API usage",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Remove the allow(deprecated) attribute and update deprecated code")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1163 — Redundant else after return/break/continue
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1163"
    name: "Redundant else after return, break, or continue"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Redundant else blocks after return/break/continue add unnecessary nesting and reduce code clarity.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for i in 0..lines.len().saturating_sub(1) {
            let prev = lines[i].trim();
            let next = lines[i+1].trim();
            if (prev.ends_with("return;") || prev.ends_with("break;") || prev.ends_with("continue;")) && next.starts_with("else ") {
                issues.push(Issue::new(
                    "S1163",
                    "Redundant else after control flow statement",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    i + 2,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1197 — Magic numbers should be replaced by named constants
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1197"
    name: "Magic numbers should be replaced by named constants"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "Magic numbers without context make code harder to understand and maintain, as their meaning and origin are not immediately clear.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"[=<>!]\s*\d{3,}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("const") && !line.contains("test") && !line.contains("\"") {
                issues.push(Issue::new(
                    "S1197",
                    "Magic number detected - use a named constant",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1214 — static mut should not be used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1214"
    name: "Mutable static variables should not be used"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}

    explanation: "static mut is inherently unsafe in Rust as it allows data races; interior mutability patterns like OnceCell or Mutex should be used instead.",
    clean_code: Logical,
    impacts: [Reliability: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("static mut") {
                issues.push(Issue::new(
                    "S1214",
                    "static mut is unsafe - use OnceCell, Lazy, or interior mutability",
                    Severity::Critical,
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
// S1244 — Floating point equality should not be used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1244"
    name: "Floating point equality should not be used"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Floating point equality comparisons can fail due to precision issues, producing unexpected results in numeric comparisons.",
    clean_code: Logical,
    impacts: [Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(f32|f64)\b.*\s*==\s*").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1244",
                    "Floating point equality comparison - may not behave as expected",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Use epsilon comparison: (a - b).abs() < EPSILON")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1481 — Unused local variable (strict: let _x = ...)
// ─────────────────────────────────────────────────────────────────────────────

// S1481 → segregated to crates/cognicode-axiom/src/rules/rules/rust/code_smells/s1481_rule.rs (SOLID)

// ─────────────────────────────────────────────────────────────────────────────
// S1643 — String concatenation in loop should use push_str or collect
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1643"
    name: "String concatenation in loops should use collect() or push_str"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}

    explanation: "String concatenation with + in loops is inefficient due to repeated allocations; push_str or iterator methods should be used instead.",
    clean_code: Efficient,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let mut in_loop = false;
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("for ") || line.contains("while ") || line.contains("loop {") {
                in_loop = true;
            }
            if in_loop
                && line.contains("+=") && (line.contains("String") || line.contains("str")) && !line.contains("push_str") {
                    issues.push(Issue::new(
                        "S1643",
                        "String concatenation in loop - use .push_str() or collect()",
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            if line.trim() == "}" {
                in_loop = false;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1751 — Loop with at most one iteration
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1751"
    name: "Loops with unconditional break have at most one iteration"
    severity: Major
    category: Bug
    language: "rust"
    params: {}

    explanation: "Loops with unconditional break execute at most once, indicating potentially incorrect loop logic or misunderstood control flow.",
    clean_code: Logical,
    impacts: [Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"loop\s*\{[^}]*break[^}]*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1751",
                    "Loop has unconditional break - at most one iteration",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_s2259_registered() {
        let rule=S2259Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
