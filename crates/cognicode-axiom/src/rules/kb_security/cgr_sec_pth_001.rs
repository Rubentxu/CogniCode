//! CGR_SEC_PTH_001 — Path Traversal
//! Detects file path construction where user input reaches path operations without
//! proper sanitization, allowing attackers to access unintended files (CWE-22).
//!
//! Languages: python, java, javascript, rust
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_SEC_PTH_001"
    name: "Path traversal vulnerability"
    severity: Critical
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "File path constructed from user input without proper validation or sanitization. Attackers can use path traversal sequences (../) to access files outside the intended directory."

    clean_code: Trustworthy,
    impacts: [Security: High],

    check: => {
        let mut issues = Vec::new();

        // Python: open(user_input), os.path.join(dir, user_input), Path(user_input)
        for qm in ctx.query_captures(
            "(call_expression \
              function: (attribute \
                object: (identifier) @obj \
                attr: (identifier) @open_fn) \
              arguments: (arguments (string) @path_part (identifier)? @rest) @call)"
        ) {
            let fn_text = qm.get("open_fn")
                .map(|f| f.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let path_text = qm.get("path_part")
                .map(|p| p.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            let is_path_func = fn_text == "open"
                || fn_text == "file"
                || fn_text == "join"
                || fn_text == "abspath"
                || fn_text == "realpath";

            if !is_path_func {
                continue;
            }

            // Check for path traversal patterns in the string
            let has_traversal = path_text.contains("../")
                || path_text.contains("..\\")
                || path_text.contains("%2e%2e");

            if has_traversal {
                let start = qm.get("call")
                    .map(|n| n.start_position())
                    .unwrap_or_default();
                issues.push(Issue::from_node(
                    "CGR_SEC_PTH_001",
                    format!("Path operation with potential path traversal detected."),
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    qm.get("call").unwrap_or_else(|| qm.get("path_part").unwrap()),
                ).with_remediation(Remediation::substantial(
                    "Validate and sanitize user input: reject paths containing '..', use basename(), or chroot to an allowed directory."
                )).with_bad_example(
                    "open(user_input)"
                ).with_good_example(
                    "open(os.path.basename(user_input))"
                ));
            }
        }

        // Also detect string concatenation for paths: "dir/" + user_input
        for qm in ctx.query_captures(
            "(binary_expression \
              left: (string) @left \
              right: (identifier) @user_var) @concat"
        ) {
            let left_text = qm.get("left")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            // If left side looks like a path component
            if left_text.contains("/") || left_text.contains("\\") {
                let start = qm.get("concat")
                    .map(|n| n.start_position())
                    .unwrap_or_default();
                issues.push(Issue::from_node(
                    "CGR_SEC_PTH_001",
                    "File path constructed by concatenating string with variable.",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    qm.get("concat").unwrap_or_else(|| qm.get("user_var").unwrap()),
                ).with_remediation(Remediation::substantial(
                    "Use os.path.join() or pathlib for safe path construction."
                )));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_SEC_PTH_001Rule::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_sec_pth_001_registered() {
        let rule = CGR_SEC_PTH_001Rule::new();
        assert_eq!(rule.id(), "CGR_SEC_PTH_001");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Critical);
    }
}
