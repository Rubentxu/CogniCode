//! CGR_BUG_ERR_006 — Exception Chaining Missing
//! Detects throw statements that do not preserve the original exception as the cause,
//! breaking the exception chain and losing valuable stack trace information (CWE-705).
//!
//! Languages: java, csharp, python
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_BUG_ERR_006"
    name: "Exception thrown without preserving original cause — exception chain broken"
    severity: Major
    category: Bug
    language: "java,python"
    params: {}

    explanation: "A new exception is thrown without passing the original exception as the cause. This breaks the exception chain and makes it impossible to trace the original source of the error. The full stack trace of the original exception is lost."

    clean_code: Clear,
    impacts: [Reliability: High],

    check: => {
        let mut issues = Vec::new();

        // Java: throw new Exception(...) without cause/initCause
        // Pattern: throw_statement > throw_expression > new_object > arguments
        // We need to find throw statements that create new exceptions without chaining
        for qm in ctx.query_captures(
            "(throw_statement \
              expression: (throw_expression \
                argument: (new_object \
                  type: (type_identifier) @exc_type \
                  arguments: (arguments) @args)))"
        ) {
            let exc_type = qm.get("exc_type")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let args_node = qm.get("args");
            
            // Common exception types that should chain
            let should_chain = exc_type == "Exception" 
                || exc_type == "RuntimeException"
                || exc_type == "Error"
                || exc_type == "IllegalStateException"
                || exc_type == "IllegalArgumentException"
                || exc_type == "AssertionError"
                || exc_type == "Throwable";

            if !should_chain {
                continue;
            }

            // Check if arguments contain anything that looks like chaining
            // A chained exception would have another exception as argument
            let args_text = args_node
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            // If the args don't contain a variable that could be the caught exception,
            // and the args are just string literals, it's likely not chained
            let has_chaining = args_text.contains("e.")
                || args_text.contains("err.")
                || args_text.contains("error.")
                || args_text.contains("exception.")
                || args_text.contains("orig.")
                || args_text.contains("cause")
                || args_text.contains("("); // method call that returns exception

            if !has_chaining && !args_text.is_empty() {
                // Only flag if there's actual content (string literal message)
                // Empty throw like "throw new Exception();" is also bad
                let throw_node = qm.get("exc_type").unwrap();
                let start = throw_node.start_position();

                issues.push(Issue::from_node(
                    "CGR_BUG_ERR_006",
                    format!("Exception type '{}' thrown without preserving original exception as cause. Use 'throw new {}Exception(msg, e)' or 'throw new {}Exception(msg).initCause(e)'", exc_type, exc_type, exc_type),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    throw_node,
                ).with_remediation(Remediation::moderate("Chain exceptions using: throw new SpecificException(msg, originalException) or throw new SpecificException(msg).initCause(originalException)")).with_bad_example("throw new RuntimeException(\"operation failed\");").with_good_example("throw new RuntimeException(\"operation failed\", e);"));
            }
        }

        // Python: raise Exception(...) without from clause
        for qm in ctx.query_captures(
            "(raise_statement \
              exception: (call \
                function: (identifier) @exc_type \
                arguments: (arguments) @args))"
        ) {
            let exc_type = qm.get("exc_type")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let args_node = qm.get("args");

            // Check for chaining keywords (Python 3: raise X from Y)
            // We look for "from" in the args which indicates explicit chaining
            let args_text = args_node
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            // raise Something() or raise Something("msg") without "from"
            if !args_text.contains("from") && !args_text.contains("None") {
                // Might be missing chaining
                let raise_node = qm.get("exc_type").unwrap();
                let start = raise_node.start_position();

                // Only flag if it's inside an except block (where we have access to 'e')
                // This requires checking parent context which is complex
                // For now, flag all potential cases that don't have explicit chaining
                issues.push(Issue::from_node(
                    "CGR_BUG_ERR_006",
                    format!("Exception type '{}' raised without chaining to original exception. Use 'raise SpecificException(msg) from e' to preserve the trace.", exc_type),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    raise_node,
                ).with_remediation(Remediation::moderate("Chain exceptions using 'raise SpecificException(msg) from original_exception'")));
            }
        }

        // C#: throw new Exception(...) without innerException
        for qm in ctx.query_captures(
            "(throw_statement \
              expression: (object_creation_expression \
                type: (type_identifier) @exc_type \
                arguments: (argument_list) @args))"
        ) {
            let exc_type = qm.get("exc_type")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let args_node = qm.get("args");

            // Common exception types that should chain
            let should_chain = exc_type == "Exception"
                || exc_type == "SystemException"
                || exc_type == "InvalidOperationException"
                || exc_type == "ArgumentException"
                || exc_type == "ArgumentNullException"
                || exc_type == "ApplicationException";

            if !should_chain {
                continue;
            }

            let args_text = args_node
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            // C# chains via second parameter: new Exception(msg, innerException)
            let has_chaining = args_text.contains(",")
                && (args_text.contains("e.")
                    || args_text.contains("err.")
                    || args_text.contains("inner")
                    || args_text.contains("exception."));

            if !has_chaining && !args_text.is_empty() {
                let throw_node = qm.get("exc_type").unwrap();
                let start = throw_node.start_position();

                issues.push(Issue::from_node(
                    "CGR_BUG_ERR_006",
                    format!("Exception type '{}' thrown without passing original exception as inner. Use 'throw new {}Exception(msg, e)'", exc_type, exc_type),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    throw_node,
                ).with_remediation(Remediation::moderate("Chain exceptions using: throw new SpecificException(msg, originalException)")));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_BUG_ERR_006Rule::new())
    }
}

/// Agent semantics for CGR_BUG_ERR_006 - Exception Chaining Missing
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const CGR_BUG_ERR_006_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects throw statements that do not preserve the original exception as the cause, breaking the exception chain and losing valuable stack trace information (CWE-705)",
    fix_playbook: "1. Identify the throw without exception chaining\n2. In Java: use 'throw new SpecificException(msg, originalException)' or '.initCause(e)'\n3. In Python: use 'raise NewException(msg) from original_exception'\n4. In C#: use 'throw new SpecificException(msg, innerException)'",
    review_questions: &[
        "What is the original exception that should be chained?",
        "Is the message being lost by not including the original exception?",
        "Should a custom exception type be used instead?"
    ],
    agent_actions: &[
        "Identify the caught exception that should be preserved",
        "Check the exception type hierarchy for appropriate chaining",
        "Suggest the correct chaining syntax for the language"
    ],
    safe_autofix: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_bug_err_006_registered() {
        let rule = CGR_BUG_ERR_006Rule::new();
        assert_eq!(rule.id(), "CGR_BUG_ERR_006");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Bug);
    }
}
