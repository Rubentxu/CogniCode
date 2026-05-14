//! CGR_SEC_TPL_001 — Format String Injection
//! Detects printf-style format strings where user input is used as format
//! argument without proper sanitization (CWE-133).
//!
//! Languages: python, java, rust, c, javascript
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_SEC_TPL_001"
    name: "Format string injection vulnerability"
    severity: Major
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Format string where user-controlled data is used as the format template. Attackers can use format specifiers (%s, %x, %n) to read memory or write to arbitrary locations."

    clean_code: Trustworthy,
    impacts: [Security: High],

    check: => {
        let mut issues = Vec::new();

        // Python: "Hello %s" % user_input or f"Hello {user_input}"
        for qm in ctx.query_captures(
            "(string) @fmt"
        ) {
            let fmt_text = qm.get("fmt")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            // Check for format string patterns: %s, %d, %x, format(), f-string
            let has_format_specifiers =
                fmt_text.contains("%s") || fmt_text.contains("%d")
                || fmt_text.contains("%x") || fmt_text.contains("%n")
                || fmt_text.contains("%f") || fmt_text.contains("{}")
                || fmt_text.contains("{:}") || fmt_text.contains("{0");

            if has_format_specifiers {
                // Check if the string is used in a printf-style context
                // We look for the string being used as first arg to format-like functions
                let parent_matches = ctx.query_captures(
                    "(binary_expression (string) @str_repr)"
                );
                let is_format_arg = parent_matches.iter().any(|pm| {
                    pm.get("str_repr").map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or("")).unwrap_or_default() == fmt_text
                });

                if !is_format_arg {
                    // Simple heuristic: if string has format chars, flag it
                    let start = qm.get("fmt")
                        .map(|n| n.start_position())
                        .unwrap_or_default();
                    issues.push(Issue::from_node(
                        "CGR_SEC_TPL_001",
                        format!("Format string with format specifiers detected. Ensure user input is not used as the format template."),
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        start.row + 1,
                        ctx,
                        qm.get("fmt").unwrap(),
                    ).with_remediation(Remediation::substantial(
                        "Use typed formatting: f'Hello {name}' if name is data, not a format string. Or use % formatting only with literal templates."
                    )).with_bad_example(
                        "print(user_input %s)"
                    ).with_good_example(
                        "print('Hello %s' % safe_name)"
                    ));
                }
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_SEC_TPL_001Rule::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_sec_tpl_001_registered() {
        let rule = CGR_SEC_TPL_001Rule::new();
        assert_eq!(rule.id(), "CGR_SEC_TPL_001");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Major);
    }
}
