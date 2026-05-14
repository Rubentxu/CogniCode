//! CGR_BUG_ERR_004 — Throw in Finally Block
//! Detects throw statements inside finally blocks which mask exceptions that were
//! previously thrown, making error diagnosis difficult (CWE-584).
//!
//! Languages: java, csharp, javascript
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_BUG_ERR_004"
    name: "Throw statement in finally block masks original exception"
    severity: Major
    category: Bug
    language: "java"
    params: {}

    explanation: "A throw statement inside a finally block will mask any exception that was about to be thrown from the try/catch. This makes error diagnosis extremely difficult because the original exception information is lost."

    clean_code: Clear,
    impacts: [Reliability: High],

    check: => {
        let mut issues = Vec::new();

        // Java/C#: finally_clause > block > throw_statement
        // JavaScript: finally_statement > statement > throw_statement
        for qm in ctx.query_captures(
            "(finally_clause \
              body: (block \
                (throw_statement) @throw))"
        ) {
            let throw_node = qm.get("throw");
            if throw_node.is_none() {
                continue;
            }
            let throw_node = throw_node.unwrap();
            let start = throw_node.start_position();

            issues.push(Issue::from_node(
                "CGR_BUG_ERR_004",
                "Throw statement inside finally block masks the original exception. Use try-with-resources or avoid the finally block throw.",
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                start.row + 1,
                ctx,
                throw_node,
            ).with_remediation(Remediation::moderate("Remove the throw from the finally block. If cleanup must run, use try-with-resources (Java) or IDisposable pattern (C#). Alternatively, store the original exception and rethrow after cleanup.")).with_bad_example(
                "finally { conn.close(); throw new RuntimeException(\"cleanup failed\"); }"
            ).with_good_example(
                "try { /* work */ } finally { conn.close(); } // let original exception propagate"
            ));
        }

        // JavaScript variant: finally_statement with throw inside
        for qm in ctx.query_captures(
            "(finally_statement \
              (statement \
                (throw_statement) @throw))"
        ) {
            let throw_node = qm.get("throw");
            if throw_node.is_none() {
                continue;
            }
            let throw_node = throw_node.unwrap();
            let start = throw_node.start_position();

            issues.push(Issue::from_node(
                "CGR_BUG_ERR_004",
                "Throw statement inside finally block masks the original exception.",
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                start.row + 1,
                ctx,
                throw_node,
            ).with_remediation(Remediation::moderate(
                "Remove the throw from the finally block. Let the original exception propagate."
            )));
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_BUG_ERR_004Rule::new())
    }
}

/// Agent semantics for CGR_BUG_ERR_004 - Throw in Finally Block
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const CGR_BUG_ERR_004_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects throw statements inside finally blocks which mask exceptions that were previously thrown, making error diagnosis difficult (CWE-584)",
    fix_playbook: "1. Identify the throw in the finally block\n2. Remove the throw or move cleanup to a separate mechanism\n3. Use try-with-resources (Java) or IDisposable pattern (C#) for cleanup\n4. Let the original exception propagate for proper error handling",
    review_questions: &[
        "Is the throw in finally masking a more important original exception?",
        "Can cleanup be done without throwing?",
        "Should the original exception be stored and rethrown after cleanup?"
    ],
    agent_actions: &[
        "Analyze what exception is being masked by the finally throw",
        "Check if the cleanup can be done without throwing",
        "Suggest try-with-resources or equivalent patterns for resource cleanup"
    ],
    safe_autofix: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_bug_err_004_registered() {
        let rule = CGR_BUG_ERR_004Rule::new();
        assert_eq!(rule.id(), "CGR_BUG_ERR_004");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Bug);
    }
}
