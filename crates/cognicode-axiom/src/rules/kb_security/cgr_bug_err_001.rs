//! CGR_BUG_ERR_001 — Empty Catch Block
//! Detects catch blocks with empty body or only comments/whitespace, which may
//! swallow exceptions and hide bugs from developers.
//!
//! Languages: java, python, javascript, csharp
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_BUG_ERR_001"
    name: "Empty catch block — exceptions may be silently swallowed"
    severity: Major
    category: Bug
    language: "java,python,javascript"
    params: {}

    explanation: "A catch block has an empty body or contains only comments/whitespace. This silently swallows exceptions, making debugging difficult and potentially hiding serious issues from developers."

    clean_code: Clear,
    impacts: [Reliability: Medium],

    check: => {
        let mut issues = Vec::new();

        // Query for catch_clause nodes
        // Empty or trivial body = only comments or whitespace
        for qm in ctx.query_captures(
            "(catch_clause body: (block (comment)*) @catch_body)"
        ) {
            let catch_body_text = qm.get("catch_body")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            // Check if body is effectively empty (only comments, whitespace, or empty)
            let is_trivial = catch_body_text.trim().is_empty()
                || catch_body_text.lines()
                    .filter(|line| !line.trim().is_empty())
                    .filter(|line| !line.trim().starts_with("//") && !line.trim().starts_with("#") && !line.trim().starts_with("<!--"))
                    .count() == 0;

            if is_trivial {
                let node = qm.get("catch_body").unwrap();
                let start = node.start_position();
                issues.push(Issue::from_node(
                    "CGR_BUG_ERR_001",
                    "Empty catch block detected — exceptions are silently swallowed. Add logging or handle the exception properly.",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    node,
                ).with_remediation(Remediation::moderate(
                    "Either log the exception, re-throw it, or handle it meaningfully. Empty catch blocks hide bugs."
                )).with_bad_example(
                    "try { ... } catch (Exception e) { }"
                ).with_good_example(
                    "try { ... } catch (Exception e) { logger.log(e); }"
                ));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_BUG_ERR_001Rule::new())
    }
}

/// Agent semantics for CGR_BUG_ERR_001 - Empty Catch Block
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const CGR_BUG_ERR_001_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects catch/except blocks with empty body or only comments/whitespace that silently swallow exceptions",
    fix_playbook: "1. Identify the empty catch block\n2. Add meaningful error handling: log the exception, take corrective action, or rethrow\n3. If silencing is intentional, add a comment explaining why and use a more specific exception type",
    review_questions: &[
        "Is the exception intentionally silenced?",
        "Would logging the exception help with debugging?",
        "Should a more specific exception type be caught?"
    ],
    agent_actions: &[
        "Analyze the caught exception type",
        "Check if the exception should be logged or propagated",
        "Suggest appropriate error handling based on context"
    ],
    safe_autofix: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_bug_err_001_registered() {
        let rule = CGR_BUG_ERR_001Rule::new();
        assert_eq!(rule.id(), "CGR_BUG_ERR_001");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Bug);
    }
}
