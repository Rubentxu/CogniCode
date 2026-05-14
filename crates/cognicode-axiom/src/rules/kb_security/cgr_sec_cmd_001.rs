//! CGR_SEC_CMD_001 — OS Command Injection
//! Detects OS command execution where user input reaches the command string without
//! proper escaping (CWE-78).
//!
//! Languages: python, java, javascript, rust
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_SEC_CMD_001"
    name: "OS command injection vulnerability"
    severity: Critical
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "User-controlled input reaches an OS command execution function without proper sanitization. Attackers can inject shell metacharacters to execute arbitrary commands."

    clean_code: Trustworthy,
    impacts: [Security: High, Reliability: High],

    check: => {
        let mut issues = Vec::new();

        // Python: os.system(cmd), os.popen(cmd), subprocess.run(cmd, shell=True), subprocess.Popen(cmd, shell=True)
        // Java: Runtime.getRuntime().exec(cmd), ProcessBuilder
        for qm in ctx.query_captures(
            "(call_expression \
              function: (attribute \
                object: (identifier) @obj \
                attr: (identifier) @method) \
              arguments: (arguments (string) @cmd_str) @call)"
        ) {
            let obj_text = qm.get("obj")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let method_text = qm.get("method")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let cmd_text = qm.get("cmd_str")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            let is_cmd_func = method_text == "system"
                || method_text == "popen"
                || method_text == "spawn"
                || method_text == "spawnl"
                || method_text == "spawnle";

            if !is_cmd_func {
                continue;
            }

            // Flag if string contains potentially dangerous patterns
            let has_user_input_indicator =
                cmd_text.contains("${")
                || cmd_text.contains("{")
                || cmd_text.contains("%")
                || cmd_text.contains("$")
                || cmd_text.contains(";");

            if has_user_input_indicator || !cmd_text.is_empty() {
                let start = qm.get("call")
                    .map(|n| n.start_position())
                    .unwrap_or_default();
                issues.push(Issue::from_node(
                    "CGR_SEC_CMD_001",
                    format!("OS command constructed with string. Shell metacharacters may allow injection."),
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    qm.get("call").unwrap_or_else(|| qm.get("cmd_str").unwrap()),
                ).with_remediation(Remediation::moderate(
                    "Use argument list form instead of shell string: subprocess.run([cmd, arg1, arg2])"
                )).with_bad_example(
                    "os.system(f\"ls {user_input}\")"
                ).with_good_example(
                    "subprocess.run([\"ls\", user_input], shell=False)"
                ));
            }
        }

        // Rust: std::process::Command::new(user_input) where user_input is a variable
        for qm in ctx.query_captures(
            "(call_expression \
              function: (field_expression \
                object: (identifier) @obj \
                field: (identifier) @method) \
              arguments: (arguments (identifier) @arg) @call)"
        ) {
            let obj_text = qm.get("obj")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();
            let method_text = qm.get("method")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            if obj_text == "Command" && method_text == "new" {
                let start = qm.get("call")
                    .map(|n| n.start_position())
                    .unwrap_or_default();
                issues.push(Issue::from_node(
                    "CGR_SEC_CMD_001",
                    "OS command constructed with variable argument. Verify the argument is not user-controlled.",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    qm.get("call").unwrap_or_else(|| qm.get("arg").unwrap()),
                ).with_remediation(Remediation::moderate(
                    "Prefer Command::new(\"prog\").arg(value) over Command::new(value)"
                )).with_bad_example(
                    "Command::new(user_input)"
                ).with_good_example(
                    "Command::new(\"ls\").arg(user_input)"
                ));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_SEC_CMD_001Rule::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_sec_cmd_001_registered() {
        let rule = CGR_SEC_CMD_001Rule::new();
        assert_eq!(rule.id(), "CGR_SEC_CMD_001");
        assert!(!rule.name().is_empty());
        assert_eq!(rule.severity(), Severity::Critical);
    }
}
