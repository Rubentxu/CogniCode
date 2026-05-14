//! CGR_BUG_ERR_002 — Generic Exception Catch
//! Detects catching overly broad exception types like Exception/Throwable/BaseException
//! which prevents proper error handling and recovery.
//!
//! Languages: java, csharp, python, javascript
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_BUG_ERR_002"
    name: "Catching overly broad exception — prevents proper error handling"
    severity: Major
    category: Bug
    language: "*"
    params: {}

    explanation: "A catch block catches Exception, Throwable, BaseException, or object. This is too broad and prevents proper error handling, as it catches ProgrammingErrors and system exceptions that should typically not be caught."

    clean_code: Clear,
    impacts: [Reliability: Medium, Maintainability: Low],

    check: => {
        let mut issues = Vec::new();

        // Query for catch_clause nodes with exception_type
        // Exception types to flag: Exception, Throwable, BaseException, object
        for qm in ctx.query_captures(
            "(catch_clause \
              exception_type: (type_identifier) @exc_type \
              body: (block) @catch_body)"
        ) {
            let exc_type_text = qm.get("exc_type")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            // Check if it's a generic/broad exception type
            let is_generic = exc_type_text == "Exception"
                || exc_type_text == "Throwable"
                || exc_type_text == "BaseException"
                || exc_type_text == "object"
                || exc_type_text == "Error";

            if is_generic {
                let node = qm.get("exc_type").unwrap();
                let start = node.start_position();
                issues.push(Issue::from_node(
                    "CGR_BUG_ERR_002",
                    format!("Catching overly broad exception type '{}'. Catch more specific exception types for proper error handling.", exc_type_text),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    node,
                ).with_remediation(Remediation::moderate(
                    &format!("Catch specific exception types instead of '{}'. For example, catch IOException or custom exceptions.", exc_type_text)
                )).with_bad_example(
                    &format!("try {{ ... }} catch ({}) {{ ... }}", exc_type_text)
                ).with_good_example(
                    "try { ... } catch (IOException e) { ... }"
                ));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_BUG_ERR_002Rule::new())
    }
}

/// Agent semantics for CGR_BUG_ERR_002 - Generic Exception Catch
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const CGR_BUG_ERR_002_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects catch blocks that catch overly broad exception types like Exception/Throwable/BaseException, preventing proper error handling",
    fix_playbook: "1. Identify the broad exception type being caught\n2. Determine what specific exceptions can actually be thrown\n3. Replace with the most specific exception type available\n4. If multiple specific catches are needed, use separate catch blocks",
    review_questions: &[
        "What specific exceptions can this code actually throw?",
        "Should different exception types be handled differently?",
        "Is there a custom exception type that better describes the error?"
    ],
    agent_actions: &[
        "Analyze the try block for potential exceptions",
        "Check the method signatures for declared exceptions",
        "Suggest the most specific exception type to catch"
    ],
    safe_autofix: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_bug_err_002_registered() {
        let rule = CGR_BUG_ERR_002Rule::new();
        assert_eq!(rule.id(), "CGR_BUG_ERR_002");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Bug);
    }
}
