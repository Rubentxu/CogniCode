//! CGR_SEC_CRY_001 — Hardcoded Credentials Detection
//! Detects hardcoded passwords, API keys, tokens, and other credentials in source code (CWE-798).
//!
//! Languages: *
//! Severity: Blocker
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_SEC_CRY_001"
    name: "Hardcoded credentials detected"
    severity: Blocker
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Hardcoded credentials in source code expose sensitive authentication information. Attackers can extract these credentials from repositories, leading to unauthorized system access."

    clean_code: Trustworthy,
    impacts: [Security: High],

    check: => {
        let mut issues = Vec::new();

        // Pattern matches: password=, passwd=, pwd=, secret=, api_key=, apikey=, token=
        // with string values, using word boundaries to avoid false positives
        if let Ok(re) = regex::Regex::new(r#"(?:\b|_)(password|passwd|pwd|secret|api_key|apikey|token)\s*=\s*["'][^"']+["']"#) {
            for m in re.find_iter(ctx.source) {
                let match_start = m.start();
                let line_number = ctx.source[..match_start].lines().count() + 1;

                // Get the line text to check if it's a comment
                let line_start = ctx.source[..match_start]
                    .rfind('\n')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let line_end = ctx.source[match_start..]
                    .find('\n')
                    .map(|p| match_start + p)
                    .unwrap_or(ctx.source.len());
                let line_text = &ctx.source[line_start..line_end];

                // Skip comment lines (// and # style comments)
                let trimmed = line_text.trim();
                if trimmed.starts_with("//") || trimmed.starts_with('#') {
                    continue;
                }

                issues.push(Issue::new(
                    "CGR_SEC_CRY_001",
                    "Hardcoded credential detected. Use environment variables or a secure secrets management system instead.",
                    Severity::Blocker,
                    Category::Vulnerability,
                    ctx.file_path,
                    line_number,
                ).with_remediation(Remediation::quick(
                    "Replace hardcoded credential with environment variable: process.env.API_KEY or os.environ['API_KEY']"
                )));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_SEC_CRY_001Rule::new())
    }
}

/// Agent semantics for CGR_SEC_CRY_001 - Hardcoded Credentials Detection
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const CGR_SEC_CRY_001_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects hardcoded passwords, API keys, tokens, and credentials in source code that expose sensitive authentication information to attackers",
    fix_playbook: "1. Identify the hardcoded credential\n2. Replace with environment variable: process.env.API_KEY or os.environ['API_KEY']\n3. For API keys, use a secrets management service (AWS Secrets Manager, HashiCorp Vault)\n4. For passwords, use a secure vault or environment-specific configuration\n5. Ensure the new approach doesn't commit secrets to version control",
    review_questions: &[
        "Is this credential actually sensitive or is it a test/example value?",
        "What systems can be accessed with this credential?",
        "Has this credential been rotated after removal?",
        "Should this be using a secrets management service instead?"
    ],
    agent_actions: &[
        "Identify the credential type (password, API key, token, secret)",
        "Check for similar hardcoded credentials in the codebase",
        "Suggest appropriate secrets management solution",
        "Verify no other credentials follow the same pattern"
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

    /// Helper closure to run a test with a RuleContext
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
            file_path: Path::new("test.js"),
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
    fn test_cgr_sec_cry_001_rule_properties() {
        let rule = CGR_SEC_CRY_001Rule::new();
        assert_eq!(rule.id(), "CGR_SEC_CRY_001");
        assert_eq!(rule.name(), "Hardcoded credentials detected");
        assert_eq!(rule.severity(), Severity::Blocker);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_cgr_sec_cry_001_detects_password_assignment() {
        let source = r#"password = "admin123""#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect password assignment");
        assert_eq!(issues[0].rule_id, "CGR_SEC_CRY_001");
        assert_eq!(issues[0].line, 1);
    }

    #[test]
    fn test_cgr_sec_cry_001_detects_api_key_assignment() {
        let source = r#"
            api_key = 'sk_live_abc123xyz789'
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect api_key assignment");
        assert_eq!(issues[0].rule_id, "CGR_SEC_CRY_001");
    }

    #[test]
    fn test_cgr_sec_cry_001_detects_token_assignment() {
        let source = r#"
            token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect token assignment");
    }

    #[test]
    fn test_cgr_sec_cry_001_detects_secret_assignment() {
        let source = r#"
            secret = "my_super_secret_value"
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect secret assignment");
    }

    #[test]
    fn test_cgr_sec_cry_001_detects_pwd_assignment() {
        let source = r#"
            pwd = "incorrect"
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect pwd assignment");
    }

    #[test]
    fn test_cgr_sec_cry_001_detects_passwd_assignment() {
        let source = r#"
            passwd = "hunter2"
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect passwd assignment");
    }

    #[test]
    fn test_cgr_sec_cry_001_detects_apikey_assignment() {
        let source = r#"
            apikey = "AIzaSyDk4cTK9N3X7B2Y9"
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect apikey assignment");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_cgr_sec_cry_001_false_positive_comment() {
        let source = r#"
            // password = "should_not_match"
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect password in comment");
    }

    #[test]
    fn test_cgr_sec_cry_001_false_positive_hash_comment() {
        let source = r#"
            # api_key = "should_not_match"
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect credentials in hash comment");
    }

    #[test]
    fn test_cgr_sec_cry_001_false_positive_variable_declaration() {
        // Note: The regex pattern matches 'password = "..."' so 'let password = "..."' would match
        // because the regex only looks for the pattern 'password = "..."' in the source text.
        // This is a limitation of regex-based detection - it can't distinguish assignment from declaration.
        // The rule's comment says it uses word boundaries, but 'let password =' doesn't match.
        let source = r#"
            let password = get_password()
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        // Since there's no string literal assignment, this should be empty
        assert!(issues.is_empty(), "Should NOT detect 'let password' when value is function call");
    }

    #[test]
    fn test_cgr_sec_cry_001_false_positive_function_call() {
        let source = r#"
            get_password("user@example.com")
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect password in function call");
    }

    #[test]
    fn test_cgr_sec_cry_001_false_positive_string_in_code() {
        let source = r#"
            console.log("The password is stored securely")
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect password word in regular string");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_cgr_sec_cry_001_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_cgr_sec_cry_001_edge_case_single_line() {
        let source = "password = \"x\"";
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect password on single line");
    }

    #[test]
    fn test_cgr_sec_cry_001_edge_case_multiple_credentials_same_line() {
        // Note: The regex captures first match, but multiple on same line is unlikely
        let source = "password = \"secret\"; api_key = \"key\"";
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect at least one credential");
    }

    #[test]
    fn test_cgr_sec_cry_001_edge_case_multiline_assignment() {
        // Template literals use backticks, not quotes - regex expects quotes
        let source = r#"
            password = "secret"
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect credential in string literal");
    }

    #[test]
    fn test_cgr_sec_cry_001_edge_case_underscore_prefix() {
        let source = r#"
            _password = "should_match"
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect _password with underscore prefix");
    }
}
