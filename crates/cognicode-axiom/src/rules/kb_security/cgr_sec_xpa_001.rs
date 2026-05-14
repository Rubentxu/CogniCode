//! CGR_SEC_XPA_001 — XPath Injection
//! Detects XPath expressions built from user input without proper escaping,
//! allowing attackers to manipulate XPath queries (CWE-643).
//!
//! Languages: python, java
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_SEC_XPA_001"
    name: "XPath injection vulnerability"
    severity: Critical
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "XPath expression constructed by concatenating user input without proper escaping. Attackers can modify query logic to access unauthorized XML data."

    clean_code: Trustworthy,
    impacts: [Security: High],

    check: => {
        let mut issues = Vec::new();

        // Java: DocumentBuilder.evaluate(xpathExpr), XPath.compile()
        for qm in ctx.query_captures(
            "(call_expression \
              function: (method_invocation \
                name: (identifier) @method) \
              arguments: (arguments (string) @xpath_str) @call)"
        ) {
            let method_text = qm.get("method")
                .map(|m| m.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let xpath_text = qm.get("xpath_str")
                .map(|s| s.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            let is_xpath_method = method_text == "evaluate"
                || method_text == "compile"
                || method_text == "selectNodes"
                || method_text == "query";

            if !is_xpath_method {
                continue;
            }

            // Check for XPath injection patterns
            let has_interpolation =
                xpath_text.contains("${")
                || xpath_text.contains("{")
                || xpath_text.contains("%")
                || xpath_text.contains("'\"'")
                || xpath_text.contains("' or ")
                || xpath_text.contains("' and ");

            if has_interpolation {
                let start = qm.get("call")
                    .map(|n| n.start_position())
                    .unwrap_or_default();
                issues.push(Issue::from_node(
                    "CGR_SEC_XPA_001",
                    "XPath expression with string interpolation detected.",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    qm.get("call").unwrap_or_else(|| qm.get("xpath_str").unwrap()),
                ).with_remediation(Remediation::substantial(
                    "Use parameterized XPath expressions or escape user input for XPath special characters."
                )));
            }
        }

        // Python: ElementTree.parse(), lxml.etree.XPath()
        for qm in ctx.query_captures(
            "(call_expression \
              function: (attribute \
                object: (identifier) @obj \
                attr: (identifier) @method) \
              arguments: (arguments (string) @xpath_str) @call)"
        ) {
            let obj_text = qm.get("obj")
                .map(|o| o.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let method_text = qm.get("method")
                .map(|m| m.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let xpath_text = qm.get("xpath_str")
                .map(|s| s.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            let is_xpath_call =
                (obj_text == "etree" || obj_text == "ElementTree")
                && (method_text == "XPath" || method_text == "find" || method_text == "findall");

            if !is_xpath_call {
                continue;
            }

            let has_interpolation =
                xpath_text.contains("${")
                || xpath_text.contains("{")
                || xpath_text.contains("%")
                || xpath_text.contains("'\"'");

            if has_interpolation {
                let start = qm.get("call")
                    .map(|n| n.start_position())
                    .unwrap_or_default();
                issues.push(Issue::from_node(
                    "CGR_SEC_XPA_001",
                    "XPath expression with string interpolation detected.",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    qm.get("call").unwrap_or_else(|| qm.get("xpath_str").unwrap()),
                ).with_remediation(Remediation::substantial(
                    "Escape special XPath characters or use a safe XML query library."
                )));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_SEC_XPA_001Rule::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_sec_xpa_001_registered() {
        let rule = CGR_SEC_XPA_001Rule::new();
        assert_eq!(rule.id(), "CGR_SEC_XPA_001");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Critical);
    }
}
