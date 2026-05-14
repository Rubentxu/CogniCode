//! CGR_SEC_LDP_001 — LDAP Injection
//! Detects LDAP search filters constructed from user input without proper escaping
//! (CWE-90).
//!
//! Languages: java, python
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_SEC_LDP_001"
    name: "LDAP injection vulnerability"
    severity: Critical
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "LDAP search filter constructed by concatenating user input without proper escaping. Attackers can use special LDAP characters (*, (, ), \\, NUL) to manipulate the filter and access unauthorized directory entries."

    clean_code: Trustworthy,
    impacts: [Security: High],

    check: => {
        let mut issues = Vec::new();

        // Java: DirContext.search() with string concatenation
        for qm in ctx.query_captures(
            "(call_expression \
              function: (method_invocation \
                name: (identifier) @search_method \
                arguments: (arguments \
                  (string) @filter \
                  (identifier)? @rest) @call) @invoke)"
        ) {
            let method_text = qm.get("search_method")
                .map(|m| m.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let filter_text = qm.get("filter")
                .map(|f| f.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            let is_search_method =
                method_text == "search"
                || method_text == "searchEntries"
                || method_text == "searchS"
                || method_text == "searchRecursive";

            if !is_search_method {
                continue;
            }

            let has_interpolation =
                filter_text.contains("${")
                || filter_text.contains("{")
                || filter_text.contains("%")
                || filter_text.contains("+")
                || filter_text.contains("*");

            if has_interpolation {
                let start = qm.get("invoke")
                    .map(|n| n.start_position())
                    .unwrap_or_default();
                issues.push(Issue::from_node(
                    "CGR_SEC_LDP_001",
                    "LDAP search filter with string interpolation detected.",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    qm.get("invoke").unwrap_or_else(|| qm.get("filter").unwrap()),
                ).with_remediation(Remediation::substantial(
                    "Escape special LDAP characters in user input: *, (, ), \\, NUL. Or use a safe search filter builder."
                )));
            }
        }

        // Python: ldap.search_s() or ldap.search() with string filter
        for qm in ctx.query_captures(
            "(call_expression \
              function: (attribute \
                object: (identifier) @ldap_obj \
                attr: (identifier) @search_fn) \
              arguments: (arguments (string) @filter) @call)"
        ) {
            let obj_text = qm.get("ldap_obj")
                .map(|o| o.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let fn_text = qm.get("search_fn")
                .map(|f| f.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let filter_text = qm.get("filter")
                .map(|f| f.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            let is_ldap_search =
                (obj_text == "ldap" || obj_text == "conn")
                && (fn_text == "search_s"
                    || fn_text == "search"
                    || fn_text == "search_ext");

            if !is_ldap_search {
                continue;
            }

            let has_interpolation =
                filter_text.contains("${")
                || filter_text.contains("{")
                || filter_text.contains("+")
                || filter_text.contains("*")
                || filter_text.contains("'\"' +");

            if has_interpolation {
                let start = qm.get("call")
                    .map(|n| n.start_position())
                    .unwrap_or_default();
                issues.push(Issue::from_node(
                    "CGR_SEC_LDP_001",
                    "LDAP filter constructed with string interpolation.",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    qm.get("call").unwrap_or_else(|| qm.get("filter").unwrap()),
                ).with_remediation(Remediation::substantial(
                    "Escape special LDAP characters in user input or use a search filter builder."
                )));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_SEC_LDP_001Rule::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_sec_ldp_001_registered() {
        let rule = CGR_SEC_LDP_001Rule::new();
        assert_eq!(rule.id(), "CGR_SEC_LDP_001");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Critical);
    }
}
