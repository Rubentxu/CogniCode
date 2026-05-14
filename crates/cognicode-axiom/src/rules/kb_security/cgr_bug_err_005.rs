//! CGR_BUG_ERR_005 — Silent Exception Swallowing
//! Detects catch blocks that explicitly silence exceptions via comments, pass statements,
//! or empty bodies, hiding errors that should be handled (CWE-390, CWE-460).
//!
//! Languages: java, python, javascript, csharp, go, rust
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_BUG_ERR_005"
    name: "Exception silently swallowed — catch block contains no error handling"
    severity: Major
    category: Bug
    language: "*"
    params: {}

    explanation: "A catch block contains no meaningful error handling — it either has an empty body, only contains a pass/continue statement, or only has comments. This silently swallows exceptions and makes debugging impossible."

    clean_code: Clear,
    impacts: [Reliability: High],

    check: => {
        let mut issues = Vec::new();

        // Python: except_clause with empty body or only pass/comment
        for qm in ctx.query_captures(
            "(except_clause \
              body: (block) @block)"
        ) {
            let block_node = qm.get("block");
            if block_node.is_none() {
                continue;
            }
            let block_node = block_node.unwrap();

            let children: Vec<_> = block_node.children(&mut block_node.walk()).collect();
            let has_meaningful_stmt = children.iter().any(|child| {
                let kind = child.kind();
                // Skip pass, comment, and empty statements
                kind != "pass_statement"
                    && kind != "comment"
                    && kind != "expression_statement" // empty expr statement
            });

            if !has_meaningful_stmt {
                let start = block_node.start_position();
                issues.push(Issue::from_node(
                    "CGR_BUG_ERR_005",
                    "Catch block silently swallows exception. Add error handling or logging.",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    block_node,
                ).with_remediation(Remediation::moderate("Either handle the exception properly (log, recover, retry) or let it propagate. If silencing is intentional, add a comment explaining why and consider using a more specific exception type.")).with_bad_example("except Exception:\\n    pass").with_good_example("except FileNotFoundError:\\n    logger.warning(\"Config file missing, using defaults\")\\n    load_defaults()"));
            }
        }

        // Java/C#/JavaScript: catch_clause with empty or comment-only body
        for qm in ctx.query_captures(
            "(catch_clause \
              body: (block) @block)"
        ) {
            let block_node = qm.get("block");
            if block_node.is_none() {
                continue;
            }
            let block_node = block_node.unwrap();

            let children: Vec<_> = block_node.children(&mut block_node.walk()).collect();
            let has_meaningful_stmt = children.iter().any(|child| {
                let kind = child.kind();
                // Skip throw, pass, comment, and empty statements
                kind != "throw_statement"
                    && kind != "pass_statement"
                    && kind != "comment"
                    && kind != "expression_statement" // empty expr statement
            });

            if !has_meaningful_stmt {
                let start = block_node.start_position();
                issues.push(Issue::from_node(
                    "CGR_BUG_ERR_005",
                    "Catch block silently swallows exception.",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    block_node,
                ).with_remediation(Remediation::moderate("Add proper exception handling: log the error, take corrective action, or rethrow a more specific exception.")));
            }
        }

        // Go: if with empty body after error check
        for qm in ctx.query_captures(
            "(if_statement \
              condition: (call_expression \
                function: (identifier) @err_check \
                arguments: (arguments)) \
              consequence: (block) @block)"
        ) {
            let func_name = qm.get("err_check")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            // Check for common Go error check functions
            if !func_name.contains("err") && func_name != "Ok" && func_name != "Is" && func_name != "Has" {
                continue;
            }

            let block_node = qm.get("block");
            if block_node.is_none() {
                continue;
            }
            let block_node = block_node.unwrap();

            let children: Vec<_> = block_node.children(&mut block_node.walk()).collect();
            let has_meaningful_stmt = !children.is_empty() && children.iter().any(|child| {
                let kind = child.kind();
                kind != "comment"
            });

            if !has_meaningful_stmt || children.is_empty() {
                let start = block_node.start_position();
                issues.push(Issue::from_node(
                    "CGR_BUG_ERR_005",
                    "Error check with empty or no handling — error is silently ignored.",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    block_node,
                ).with_remediation(Remediation::moderate(
                    "Handle the error: log it, return it, or take corrective action."
                )));
            }
        }

        // Rust: if let or match with empty arm body (only contains None/Ok/Err followed by nothing)
        for qm in ctx.query_captures(
            "(if_let_expression \
              body: (block) @block)"
        ) {
            let block_node = qm.get("block");
            if block_node.is_none() {
                continue;
            }
            let block_node = block_node.unwrap();

            let children: Vec<_> = block_node.children(&mut block_node.walk()).collect();
            let has_meaningful_stmt = !children.is_empty() && children.iter().any(|child| {
                child.kind() != "comment"
            });

            if !has_meaningful_stmt || children.is_empty() {
                let start = block_node.start_position();
                issues.push(Issue::from_node(
                    "CGR_BUG_ERR_005",
                    "Result/Option handling with empty body — error is silently ignored.",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    block_node,
                ).with_remediation(Remediation::moderate(
                    "Handle the Result/Option: use expect(), unwrap_or(), or proper error propagation."
                )));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_BUG_ERR_005Rule::new())
    }
}

/// Agent semantics for CGR_BUG_ERR_005 - Silent Exception Swallowing
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const CGR_BUG_ERR_005_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects catch blocks that explicitly silence exceptions via comments, pass statements, or empty bodies, hiding errors that should be handled (CWE-390, CWE-460)",
    fix_playbook: "1. Identify the silent catch/except block\n2. Add meaningful error handling: log the error, take corrective action, or rethrow\n3. If silencing is intentional, add a comment explaining why and consider using a more specific exception type\n4. For Python: replace 'pass' with actual handling or re-raise",
    review_questions: &[
        "Is the exception intentionally silenced?",
        "What information is lost by not handling this exception?",
        "Should this exception be allowed to propagate instead?"
    ],
    agent_actions: &[
        "Analyze the exception type and context",
        "Check if logging would help debugging without changing behavior",
        "Suggest appropriate handling based on the exception type"
    ],
    safe_autofix: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_bug_err_005_registered() {
        let rule = CGR_BUG_ERR_005Rule::new();
        assert_eq!(rule.id(), "CGR_BUG_ERR_005");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Bug);
    }
}
