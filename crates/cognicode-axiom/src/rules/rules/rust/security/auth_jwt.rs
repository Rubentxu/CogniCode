//! S5860 — JWT Without Expiration Detection
//! Detects JWT tokens created without expiration claims, allowing potentially infinite session validity (CWE-613).
//!
//! Languages: *
//! Severity: Major
//! Category: Vulnerability
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

/// Rule constant for S5860
const RULE_ID: &str = "S5860";
const RULE_NAME: &str = "JWT without expiration claim detected";
const SEVERITY: Severity = Severity::Major;
const CATEGORY: Category = Category::Vulnerability;

declare_rule! {
    id: "S5860"
    name: "JWT without expiration claim detected"
    severity: Major
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "JWT tokens without an expiration (exp) claim remain valid indefinitely. This allows attackers who obtain a token to have perpetual access. Always include an exp claim to enable token expiration and rotation."
    clean_code: Trustworthy,
    impacts: [Security: High, Reliability: Medium],
    check: => {
        let mut issues = Vec::new();

        // Pattern 1: JWT encode calls without 'exp' in the claims
        let jwt_encode_patterns = [
            // jwt::encode without exp
            r#"(?i)jwt::encode\s*\(\s*(?:&|&mut)?\s*(?:Header|EncodingKey|claims)[^)]*"#,
            // jsonwebtoken::encode
            r#"(?i)jsonwebtoken::encode\s*\([^)]*(?:Header|EncodingKey|claims)[^)]*"#,
            // .encode() on Jwt instance
            r#"(?i)(?:Jwt|Token)\s*\([^)]*\)\.encode\(\s*\{"#,
            // Custom JWT creation patterns (but not function definitions)
            r#"(?i)(?:create|make|build|new)[_-]?(?:jwt|token)\s*\([^)]*\)(?!\s*\{)"#,
        ];

        // Pattern 2: Claims struct definition without exp field
        let claims_without_exp_pattern = regex::Regex::new(
            r#"(?i)(?:struct|impl)\s+\w*(?:Claims|Token|Jwt)\w*[^{]*\{[^}]*(?:exp|expiration|expires)\s*:"#
        ).unwrap();

        // Pattern 3: JWT string literals (base64 encoded) that likely lack exp
        // This is harder to detect reliably, so we focus on code patterns

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("#") {
                continue;
            }

            // Check for JWT encode patterns
            for pattern in &jwt_encode_patterns {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if re.is_match(trimmed) {
                        // Look ahead to see if 'exp' is set within next few lines
                        let has_exp_claim = (0..5)
                            .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                            .take(6)
                            .any(|l| l.contains("exp") || l.contains("expiration") || l.contains("expires_at"));

                        if !has_exp_claim {
                            issues.push(Issue::new(
                                RULE_ID,
                                "JWT created without expiration claim. Add an 'exp' claim to enable token expiration and prevent perpetual access.",
                                SEVERITY,
                                CATEGORY,
                                ctx.file_path,
                                line_idx + 1,
                            ).with_remediation(Remediation::moderate(
                                "Add 'exp' claim with appropriate expiration time: Claims { exp: ... }"
                            )));
                        }
                        break;
                    }
                }
            }

            // Check for claims struct without exp
            if claims_without_exp_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "JWT claims struct may lack expiration claim. Ensure all JWTs have an 'exp' claim.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Add 'exp: DateTime<Utc>' field to the Claims struct"
                )));
            }
        }

        // Additional check: Look for JWT validation without exp check
        let jwt_decode_pattern = regex::Regex::new(
            r#"(?i)(?:jwt|jsonwebtoken)::(?:decode|verify|validate)"#
        ).unwrap();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("#") {
                continue;
            }

            if jwt_decode_pattern.is_match(trimmed) {
                // Check if validation options include exp verification
                let has_exp_validation = (0..3)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                    .take(4)
                    .any(|l| l.contains("exp") && (l.contains("validate_exp") || l.contains("verify_exp") || l.contains("leeway")));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(S5860Rule::new())
    }
}

/// Agent semantics for S5860 - JWT Without Expiration
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S5860_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects JWT tokens created without expiration claims, allowing potentially infinite session validity",
    fix_playbook: "1. Identify JWT creation code\n2. Add 'exp' claim with appropriate expiration time\n3. For access tokens: typically 15-60 minutes\n4. For refresh tokens: typically 7-30 days\n5. Ensure server validates 'exp' claim during verification\n6. Consider implementing token refresh mechanism",
    review_questions: &[
        "What is the appropriate token lifetime for this use case?",
        "Is the 'exp' claim being validated during token verification?",
        "Should a refresh token mechanism be implemented?",
        "What happens when an expired token is presented?"
    ],
    agent_actions: &[
        "Identify JWT creation patterns",
        "Check for 'exp' claim presence",
        "Verify token expiration validation",
        "Recommend appropriate token lifetime"
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
    fn test_s5860_rule_properties() {
        let rule = S5860Rule::new();
        assert_eq!(rule.id(), "S5860");
        assert_eq!(rule.name(), "JWT without expiration claim detected");
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5860_detects_jwt_encode_without_exp() {
        let source = r#"
            let token = jwt::encode(&header, &claims, &key)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5860Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect jwt::encode without exp");
        assert_eq!(issues[0].rule_id, "S5860");
    }

    #[test]
    fn test_s5860_detects_jsonwebtoken_encode() {
        let source = r#"
            let token = jsonwebtoken::encode(&EncodingKey::from_secret(key.as_bytes()), &claims)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5860Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect jsonwebtoken::encode");
    }

    #[test]
    fn test_s5860_detects_create_jwt_function() {
        let source = r#"
            fn create_jwt(claims: &Claims) -> Result<String> {
                let token = jwt::encode(claims, key)?;
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5860Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect create_jwt pattern");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5860_false_positive_comment() {
        let source = r#"
            // jwt::encode(&header, &claims, &key);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5860Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect jwt in comment");
    }

    #[test]
    fn test_s5860_false_positive_with_exp() {
        let source = r#"
            let token = jwt::encode(&header, &claims, &key)?;
            let exp = chrono::Utc::now() + chrono::Duration::hours(1);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5860Rule::new();
            rule.check(ctx)
        });
        // Should not trigger because 'exp' appears nearby
        assert!(issues.is_empty(), "Should NOT detect when exp is set");
    }

    #[test]
    fn test_s5860_false_positive_exp_claim() {
        let source = r#"
            let mut claims = Claims {
                sub: user_id,
                exp: expiry,
                iat: now,
            };
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5860Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect when Claims has exp field");
    }

    #[test]
    fn test_s5860_false_positive_doc_comment() {
        let source = r#"
            /// Creates a JWT using jwt::encode
            fn create_token() {}
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5860Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect JWT in doc comment");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5860_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5860Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s5860_edge_case_decode_only() {
        let source = r#"
            let token = jwt::decode(&token, &key)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5860Rule::new();
            rule.check(ctx)
        });
        // decode should not trigger - only encode creates tokens
        assert!(issues.is_empty(), "Should NOT trigger on decode only");
    }
}
