//! CGR_BUG_ERR_003 — Return in Finally Block
//! Detects return statements inside finally blocks which suppress exceptions
//! and prevent proper error propagation.
//!
//! Languages: java, csharp, javascript, python
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_BUG_ERR_003"
    name: "Return in finally block — suppresses exceptions"
    severity: Major
    category: Bug
    language: "*"
    params: {}

    explanation: "A return statement inside a finally block will suppress any exception that was being propagated, replacing it with the return value. This can hide errors and make debugging extremely difficult."

    clean_code: Trustworthy,
    impacts: [Reliability: High],

    check: => {
        let mut issues = Vec::new();

        // Query for return_statement inside finally_clause body
        // Tree-sitter pattern: finally_clause > block > return_statement
        for qm in ctx.query_captures(
            "(finally_clause \
              body: (block \
                (return_statement) @return_stmt))"
        ) {
            let node = qm.get("return_stmt").unwrap();
            let start = node.start_position();
            issues.push(Issue::from_node(
                "CGR_BUG_ERR_003",
                "Return statement in finally block suppresses exceptions. Errors may be silently lost.",
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                start.row + 1,
                ctx,
                node,
            ).with_remediation(Remediation::substantial(
                "Remove the return statement from the finally block. If you need to return a value, do so after the try-catch block completes."
            )).with_bad_example(
                "try { ... } finally { return value; }"
            ).with_good_example(
                "try { ... } finally { cleanup(); } // no return here\nreturn value;"
            ));
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_BUG_ERR_003Rule::new())
    }
}

/// Agent semantics for CGR_BUG_ERR_003 - Return in Finally Block
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const CGR_BUG_ERR_003_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects return statements inside finally blocks which suppress exceptions and prevent proper error propagation",
    fix_playbook: "1. Identify the return statement in the finally block\n2. Move the return statement outside the try-finally block\n3. If cleanup is needed, keep it in finally but remove the return\n4. Ensure the exception can propagate properly",
    review_questions: &[
        "Is the return value needed from the finally block?",
        "Should the exception propagate instead of returning?",
        "Can the cleanup be done without suppressing the exception?"
    ],
    agent_actions: &[
        "Analyze the control flow to understand what exception is being suppressed",
        "Check if the return value is actually needed outside the finally",
        "Suggest restructuring to let exceptions propagate"
    ],
    safe_autofix: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_bug_err_003_registered() {
        let rule = CGR_BUG_ERR_003Rule::new();
        assert_eq!(rule.id(), "CGR_BUG_ERR_003");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Bug);
    }
}
