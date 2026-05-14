//! S5130 — Missing Input Sanitization Detection
//! Detects user input used in security-sensitive operations without apparent sanitization.
//!
//! Languages: *
//! Severity: Minor
//! Category: Vulnerability
//!
//! This is a catch-all rule for cases where user input flows into security-sensitive
//! operations without visible sanitization. For more specific cases (SQL injection,
//! XSS, etc.), other rules should take precedence.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S5130
const RULE_ID: &str = "S5130";
const RULE_NAME: &str = "Missing input sanitization detected";
const SEVERITY: Severity = Severity::Minor;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// User input sources - functions/variables that typically receive external input
static USER_INPUT_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Generic user input patterns
        regex::Regex::new(r#"(?i)(?:user_input|user_data|user_val|raw_input|get_param|query_param|request_param|request_body|form_data)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:req\.|request\.|http_request|HttpRequest)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:argv|args\.parse|CommandLine|stdin)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:\.read\(\)|\.read_to_string\(\))"#).unwrap(),
        // HTTP frameworks
        regex::Regex::new(r#"(?i)(?:Path|Query|Form|Json|Header)\s*<"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:axum|actix|rocket|rouille)\s*::"#).unwrap(),
    ]
});

/// Sanitization function names - indicate input is being sanitized
static SANITIZATION_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"(?i)sanitize|escape|validate|clean|filter|strip|normalize"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:html_escape|entity_escape|xml_escape|json_escape)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:PreparedStatement|parameterized|bind_param)"#).unwrap(),
        regex::Regex::new(r#"(?i)allowlist|whitelist|blocklist|blacklist"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:base64_encode|base64_decode|url_encode|html_attribute)"#).unwrap(),
    ]
});

/// Security-sensitive operations that require sanitization
static SECURITY_SENSITIVE_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // File operations
        regex::Regex::new(r#"(?i)(?:file_write|write_file|create_file|open.*write|fs::write|std::fs::create)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:remove_file|delete_file|unlink|rmdir)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:File::create|std::fs::write|std::fs::remove)"#).unwrap(),
        // Command execution
        regex::Regex::new(r#"(?i)(?:Command::new|process::Command|exec|system\(|popen)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:shell|bash|sh|-c)"#).unwrap(),
        // SQL execution
        regex::Regex::new(r#"(?i)(?:execute|query|sql::|diesel::|rusqlite|sqlx::query)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:insert_into|update_table|delete_from)"#).unwrap(),
        // HTML/JS injection targets
        regex::Regex::new(r#"(?i)(?:innerHTML|outerHTML|insertAdjacentHTML)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:document\.write|eval\(|Function\()"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:template\.render|render_template|render_to_string)"#).unwrap(),
        // Environment/config
        regex::Regex::new(r#"(?i)(?:set_env|env::set_var|export!|Config::new)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:permission|chmod|chown|chgrp)"#).unwrap(),
        // Crypto operations
        regex::Regex::new(r#"(?i)(?:sign|verify|encrypt|decrypt|certificate)"#).unwrap(),
    ]
});

/// Safe patterns that indicate sanitization is happening
static SAFE_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"(?i)sanitize\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)validate\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)escape\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)\.bind\("#).unwrap(),
        regex::Regex::new(r#"(?i)PreparedStatement"#).unwrap(),
        regex::Regex::new(r#"(?i)\$1|\?.*bind"#).unwrap(),
        regex::Regex::new(r#"(?i)whitelist|allowlist"#).unwrap(),
    ]
});

declare_rule! {
    id: "S5130"
    name: "Missing input sanitization detected"
    severity: Minor
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "User input is being used in security-sensitive operations without visible sanitization. This could lead to various injection attacks depending on the context."
    clean_code: Complete,
    impacts: [Security: Medium],
    check: => {
        let mut issues = Vec::new();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("#") {
                continue;
            }

            // Check if line contains user input pattern
            let has_user_input = USER_INPUT_PATTERNS.iter().any(|re| re.is_match(trimmed));

            // Check if line contains security-sensitive operation
            let has_sensitive_op = SECURITY_SENSITIVE_PATTERNS.iter().any(|re| re.is_match(trimmed));

            // Check if sanitization is visible on this line
            let has_sanitization = SANITIZATION_PATTERNS.iter().any(|re| re.is_match(trimmed));

            // Check for safe patterns (parameterized queries, etc.)
            let has_safe_pattern = SAFE_PATTERNS.iter().any(|re| re.is_match(trimmed));

            // If user input + security-sensitive operation without sanitization
            if has_user_input && has_sensitive_op && !has_sanitization && !has_safe_pattern {
                issues.push(Issue::new(
                    RULE_ID,
                    format!("User input used in security-sensitive operation without visible sanitization."),
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Ensure all user input is sanitized/validated before use in security-sensitive operations. \
                     Use allowlists, parameterized queries, or context-appropriate escaping."
                )));
            }
        }

        issues
    }
}


/// Agent semantics for S2077 - Missing Input Sanitization
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S5130_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects user input used in security-sensitive operations (file ops, command execution, SQL, HTML/JS) without visible sanitization, which could enable various injection attacks",
    fix_playbook: "1. Identify the user input source and the security-sensitive operation\n\
                   2. Determine appropriate sanitization for the context:\n\
                      - SQL: Use parameterized queries or prepared statements\n\
                      - File paths: Validate against allowlist, canonicalize paths\n\
                      - Commands: Avoid shell execution, use exec with args array\n\
                      - HTML/JS: Use context-appropriate escaping (html_escape, textContent)\n\
                      - Shell: Never pass raw user input to shell\n\
                   3. Implement sanitization before the operation\n\
                   4. Add input validation (allowlists preferred over blocklists)\n\
                   5. Add unit tests for sanitization functions",
    review_questions: &[
        "What is the source of this input (user, file, network)?",
        "What security-sensitive operation is this input used in?",
        "What is the appropriate sanitization for this context?",
        "Is an allowlist approach possible instead of blocklist?",
        "Could this input reach other security-sensitive operations?",
        "Is there existing sanitization that might be applied upstream?"
    ],
    agent_actions: &[
        "Identify user input sources in security-sensitive operations",
        "Check for visible sanitization functions (sanitize, validate, escape)",
        "Suggest context-appropriate sanitization strategies",
        "Recommend parameterized queries for SQL operations",
        "Suggest allowlist validation where possible",
        "Flag command execution with user input as high risk"
    ],
    safe_autofix: false,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::*;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;
    use cognicode_core::infrastructure::parser::Language;
    use std::path::Path;
    use tree_sitter::Parser as TsParser;

    fn with_rule_context<F, R>(source: &str, language: Language, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = language.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();
        let symbol_table = crate::rules::symbol_table::SymbolTableBuilder::new()
            .build(&tree, source);

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new("test.rs"),
            language: &language,
            graph: &graph,
            metrics: &metrics,
            symbol_table: Some(&symbol_table),
        };

        f(&ctx)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Rule Properties Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2077_rule_properties() {
        let rule = MISSING_INPUT_SANITIZATIONRule::new();
        assert_eq!(rule.id(), "S2077");
        assert_eq!(rule.name(), "Missing input sanitization detected");
        assert_eq!(rule.severity(), Severity::Minor);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2077_detects_user_input_in_sql() {
        let source = r#"
            let result = sql::execute(&user_input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect user_input in SQL execute");
        assert_eq!(issues[0].rule_id, "S2077");
    }

    #[test]
    fn test_s2077_detects_user_input_in_file_write() {
        let source = r#"
            std::fs::write(&filename, &user_data)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect user input in file write");
    }

    #[test]
    fn test_s2077_detects_raw_input_in_command() {
        let source = r#"
            Command::new("sh").arg("-c").arg(&user_input).output()?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect raw user input in command execution");
    }

    #[test]
    fn test_s2077_detects_request_param_in_sensitive_op() {
        let source = r#"
            fn handler(req: HttpRequest) {
                let data = req.query_param("input");
                file_write(data);
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect request param in sensitive operation");
    }

    #[test]
    fn test_s2077_detects_stdin_injection() {
        let source = r#"
            let input = std::io::stdin().read_to_string();
            execute_sql(&input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect stdin in sensitive operation");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2077_false_positive_sanitized_input() {
        let source = r#"
            let safe_input = sanitize(&user_input);
            sql::execute(&safe_input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect when sanitization is visible");
    }

    #[test]
    fn test_s2077_false_positive_validated_input() {
        let source = r#"
            let validated = validate(&user_input)?;
            file_write(&validated);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect when validation is visible");
    }

    #[test]
    fn test_s2077_false_positive_parameterized_query() {
        let source = r#"
            sqlx::query("SELECT * FROM users WHERE id = $1").bind(user_id);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect parameterized queries");
    }

    #[test]
    fn test_s2077_false_positive_comment() {
        let source = r#"
            // sql::execute(&user_input); -- This would be dangerous
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect code in comments");
    }

    #[test]
    fn test_s2077_false_positive_static_data() {
        let source = r#"
            let safe_data = "static content";
            file_write(safe_data);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static data without user input");
    }

    #[test]
    fn test_s2077_false_positive_whitelist() {
        let source = r#"
            if whitelist.contains(&user_input) {
                execute_cmd(&user_input);
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect when allowlist validation is present");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2077_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s2077_edge_case_multiple_inputs() {
        let source = r#"
            let result = execute_sql(&user_input1, &user_input2);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect multiple user inputs");
    }

    #[test]
    fn test_s2077_edge_case_case_insensitive() {
        let source = r#"
            let result = SQL::EXECUTE(&USER_INPUT);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = MISSING_INPUT_SANITIZATIONRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect case-insensitive patterns");
    }
}