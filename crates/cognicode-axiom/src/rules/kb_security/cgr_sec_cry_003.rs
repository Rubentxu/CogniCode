//! CGR_SEC_CRY_003 — Weak Encryption Algorithm Detection
//! Detects usage of weak encryption algorithms (DES, RC4, 3DES, AES with ECB mode)
//! which have known cryptographic vulnerabilities (CWE-327).
//!
//! Languages: *
//! Severity: Critical
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "CGR_SEC_CRY_003"
    name: "Weak encryption algorithm usage"
    severity: Critical
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Weak encryption algorithms expose data to unauthorized access. DES has a small key size, RC4 has biases, 3DES is slow and has limited blocksize, and AES ECB mode reveals patterns in encrypted data."

    clean_code: Trustworthy,
    impacts: [Security: High],

    check: => {
        let mut issues = Vec::new();

        // Weak encryption algorithm names
        let weak_encryptions = [
            "des", "des_encrypt", "des_decrypt",
            "rc4", "rc4_encrypt", "arc4",
            "3des", "tripledes", "des3",
            "aes_ecb", "aes_ecb_encrypt", "aes_ecb_decrypt",
            "blowfish", "bf_ecb",
            "pbkdf1", "pbkdf2_md5",
            "Crypto.Cipher.DES", "Crypto.Cipher.RC4",
            "Crypto.Cipher.3DES", "Crypto.Cipher.Blowfish"
        ];

        // Detect function calls to weak encryption functions
        for qm in ctx.query_captures(
            "(call_expression \
              function: (identifier) @fn_name \
              arguments: (arguments) @args) @call"
        ) {
            let fn_name = qm.get("fn_name")
                .map(|n| n.utf8_text(ctx.source.as_bytes()).unwrap_or(""))
                .unwrap_or_default();

            let fn_lower = fn_name.to_lowercase();
            let is_weak_enc = weak_encryptions.iter().any(|e| {
                fn_lower == *e || fn_lower.contains(e) || fn_lower.contains("_ecb")
            });

            // Also check for ECB mode patterns
            let is_ecb_mode = fn_lower.contains("ecb") && fn_lower.contains("aes");

            if is_weak_enc || is_ecb_mode {
                let start = qm.get("call")
                    .map(|n| n.start_position())
                    .unwrap_or_default();
                let algorithm = if is_ecb_mode { "AES ECB" } else { &fn_name };

                issues.push(Issue::from_node(
                    "CGR_SEC_CRY_003",
                    format!("Weak encryption algorithm '{}' detected. Use AES-GCM, AES-CBC with HMAC, or ChaCha20-Poly1305 instead.", algorithm),
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    start.row + 1,
                    ctx,
                    qm.get("call").unwrap(),
                ).with_remediation(Remediation::moderate(
                    "Replace with AES-GCM or ChaCha20-Poly1305 for authenticated encryption"
                )));
            }
        }

        // Detect string literals referencing weak encryption
        if let Ok(re) = regex::Regex::new(r#"(?i)['"](?:des|rc4|3des|tripledes|arc4|bf_ecb)['"]"#) {
            for m in re.find_iter(ctx.source) {
                let line_number = ctx.source[..m.start()].lines().count() + 1;

                // Skip comment lines
                let line_start = ctx.source[..m.start()]
                    .rfind('\n')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let line_text = &ctx.source[line_start..m.start()];
                if line_text.trim().starts_with("//") || line_text.trim().starts_with('#') {
                    continue;
                }

                issues.push(Issue::new(
                    "CGR_SEC_CRY_003",
                    "Weak encryption algorithm name detected in string literal.",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    line_number,
                ));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(CGR_SEC_CRY_003Rule::new())
    }
}

/// Agent semantics for CGR_SEC_CRY_003 - Weak Encryption Algorithm Detection
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const CGR_SEC_CRY_003_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects weak encryption algorithms (DES, RC4, 3DES, AES-ECB) that expose data to unauthorized access through known cryptographic weaknesses",
    fix_playbook: "1. Identify the weak encryption algorithm (DES, RC4, 3DES, AES-ECB)\n2. For AES: use AES-GCM or AES-CBC with HMAC for authenticated encryption\n3. For legacy systems: use ChaCha20-Poly1305 as alternative\n4. Ensure key sizes meet minimum requirements (AES-256)\n5. Re-encrypt existing data with the new algorithm if upgrading",
    review_questions: &[
        "Is the encrypted data at risk of exposure?",
        "What is the minimum encryption strength required?",
        "Are there existing encrypted records that need re-encryption?",
        "Does the new algorithm support the required key sizes?"
    ],
    agent_actions: &[
        "Identify the encryption algorithm and mode being used",
        "Recommend AES-GCM or ChaCha20-Poly1305 as modern replacements",
        "Check for data that requires re-encryption",
        "Verify proper IV/crypto nonce handling in replacement"
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
    fn test_cgr_sec_cry_003_rule_properties() {
        let rule = CGR_SEC_CRY_003Rule::new();
        assert_eq!(rule.id(), "CGR_SEC_CRY_003");
        assert_eq!(rule.name(), "Weak encryption algorithm usage");
        assert_eq!(rule.severity(), Severity::Critical);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_cgr_sec_cry_003_detects_des_call() {
        let source = r#"
            const cipher = des(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect des function call");
        assert_eq!(issues[0].rule_id, "CGR_SEC_CRY_003");
    }

    #[test]
    fn test_cgr_sec_cry_003_detects_rc4_call() {
        let source = r#"
            const cipher = rc4(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect rc4 function call");
    }

    #[test]
    fn test_cgr_sec_cry_003_detects_3des_call() {
        let source = r#"
            const cipher = tripledes(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect 3des function call");
    }

    #[test]
    fn test_cgr_sec_cry_003_detects_aes_ecb_call() {
        let source = r#"
            const cipher = aes_ecb(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect AES ECB mode");
    }

    #[test]
    fn test_cgr_sec_cry_003_detects_blowfish_call() {
        let source = r#"
            const cipher = blowfish(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect blowfish function call");
    }

    #[test]
    fn test_cgr_sec_cry_003_detects_des_encrypt_call() {
        let source = r#"
            const encrypted = des_encrypt(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect des_encrypt function call");
    }

    #[test]
    fn test_cgr_sec_cry_003_detects_rc4_encrypt_call() {
        let source = r#"
            const encrypted = rc4_encrypt(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect rc4_encrypt function call");
    }

    #[test]
    fn test_cgr_sec_cry_003_detects_des_string_literal() {
        let source = r#"
            const algorithm = "des";
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect des string literal");
    }

    #[test]
    fn test_cgr_sec_cry_003_detects_rc4_string_literal() {
        let source = r#"
            const algorithm = "rc4";
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect rc4 string literal");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_cgr_sec_cry_003_false_positive_aes_gcm() {
        let source = r#"
            const cipher = aes_gcm(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect aes_gcm (strong mode)");
    }

    #[test]
    fn test_cgr_sec_cry_003_false_positive_aes_cbc() {
        let source = r#"
            const cipher = aes_cbc(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect aes_cbc when properly used with HMAC");
    }

    #[test]
    fn test_cgr_sec_cry_003_false_positive_chacha20() {
        let source = r#"
            const cipher = chacha20(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect chacha20 (strong cipher)");
    }

    #[test]
    fn test_cgr_sec_cry_003_false_positive_comment() {
        let source = r#"
            // Use des for legacy compatibility only
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect des in comment");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_cgr_sec_cry_003_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_cgr_sec_cry_003_edge_case_single_line() {
        let source = "des(data, key)";
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect des on single line");
    }

    #[test]
    fn test_cgr_sec_cry_003_edge_case_case_insensitive() {
        let source = r#"
            const cipher = RC4(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect RC4 (case insensitive)");
    }

    #[test]
    fn test_cgr_sec_cry_003_edge_case_multiple_algorithms() {
        let source = r#"
            const c1 = des(data, k1); const c2 = rc4(data, k2);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect at least one weak algorithm");
    }

    #[test]
    fn test_cgr_sec_cry_003_edge_case_arc4_variant() {
        let source = r#"
            const cipher = arc4(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect arc4 variant");
    }

    #[test]
    fn test_cgr_sec_cry_003_edge_case_3des_variant() {
        let source = r#"
            const cipher = des3(data, key);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = CGR_SEC_CRY_003Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect des3 variant");
    }
}
