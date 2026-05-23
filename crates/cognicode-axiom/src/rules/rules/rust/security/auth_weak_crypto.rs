//! Auth Weak Crypto & Credential Storage Rules
//!
//! - S256: Plain text credential storage
//! - S2068b: Hardcoded API key
//! - S532: Password logged in plain text
//!
//! Languages: *
//! Severity: Blocker/Critical
//! Category: Vulnerability
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

/// Rule constant for S256
const S256_RULE_ID: &str = "S256";
const S256_RULE_NAME: &str = "Plain text credential storage detected";
const S256_SEVERITY: Severity = Severity::Blocker;
const S256_CATEGORY: Category = Category::Vulnerability;

/// Rule constant for S2068b
const S2068B_RULE_ID: &str = "S2068b";
const S2068B_RULE_NAME: &str = "Hardcoded API key detected";
const S2068B_SEVERITY: Severity = Severity::Blocker;
const S2068B_CATEGORY: Category = Category::SecurityHotspot;

/// Rule constant for S532
const S532_RULE_ID: &str = "S532";
const S532_RULE_NAME: &str = "Password logged in plain text";
const S532_SEVERITY: Severity = Severity::Critical;
const S532_CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// S256 — Plain Text Credential Storage
// ═══════════════════════════════════════════════════════════════════════════════

declare_rule! {
    id: "S256"
    name: "Plain text credential storage detected"
    severity: Blocker
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Storing credentials in plain text exposes them to anyone with access to the storage medium. Credentials should be hashed using appropriate algorithms (e.g., bcrypt, Argon2) before storage."
    clean_code: Trustworthy,
    impacts: [Security: High],
    check: => {
        let mut issues = Vec::new();

        // Pattern: password written to file, database, or transmitted without hashing
        let password_storage_patterns = [
            // File operations with password
            r#"(?i)(?:std::fs::write|File::create|fwrite|file_put_contents)\s*\([^)]*(?:password|passwd|pwd|credential|secret)"#,
            // Database inserts with password - handles both password='value' and (password) VALUES('value')
            r#"(?i)(?:INSERT|UPDATE|SET)\s+[^;]*(?:password|passwd|pwd)[^;']*'[^']{6,}'"#,
            // println!/echo with password
            r#"(?i)(?:println!|fprintln!|printf!|echo|print)\s*\([^)]*(?:password|passwd|pwd)"#,
            // Password sent in plain text
            r#"(?i)(?:send|write|transmit)\s*\([^)]*(?:password|passwd|pwd)\s*(?:to|into|-->)"#,
        ];

        let regexes: Vec<_> = password_storage_patterns.iter()
            .filter_map(|p| regex::Regex::new(p).ok())
            .collect();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("#") {
                continue;
            }

            for re in &regexes {
                if re.is_match(trimmed) {
                    issues.push(Issue::new(
                        S256_RULE_ID,
                        "Plain text credential storage detected. Store passwords using secure hashing algorithms like bcrypt, Argon2, or scrypt.",
                        S256_SEVERITY,
                        S256_CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Hash the credential before storage using a secure password hashing algorithm"
                    )));
                    break;
                }
            }
        }

        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// S2068b — Hardcoded API Key
// ═══════════════════════════════════════════════════════════════════════════════

declare_rule! {
    id: "S2068b"
    name: "Hardcoded API key detected"
    severity: Blocker
    category: SecurityHotspot
    language: "*"
    params: {}

    explanation: "Hardcoded API keys can be extracted from source code and used by attackers to gain unauthorized access to services. API keys should be stored in environment variables or a secure secrets manager."
    clean_code: Trustworthy,
    impacts: [Security: High],
    check: => {
        let mut issues = Vec::new();

        // API key patterns with specific naming
        let api_key_patterns = [
            // Direct API key assignments
            r#"(?i)(?:api[_-]?key|apikey|api[_-]?secret|api[_-]?token)\s*[=:]\s*['"][a-zA-Z0-9_\-]{16,}['"]"#,
            // Authorization headers with API keys
            r#"(?i)Authorization:\s*(?:Bearer|Basic)\s+[a-zA-Z0-9_\-]{16,}"#,
            // x-api-key header style
            r#"(?i)x[_-]?api[_-]?key\s*[=:]\s*['"][a-zA-Z0-9_\-]{16,}['"]"#,
            // AWS-style keys
            r#"(?i)(?:AWS|aws)[_-]?(?:access[_-]?key|secret)[_-]?id\s*[=:]\s*['"][a-zA-Z0-9]{16,}['"]"#,
            // Service-specific API keys
            r#"(?i)(?:stripe|sendgrid|twilio|s3)[_-]?(?:api[_-]?key|secret)\s*[=:]\s*['"][a-zA-Z0-9_\-]{16,}['"]"#,
        ];

        let regexes: Vec<_> = api_key_patterns.iter()
            .filter_map(|p| regex::Regex::new(p).ok())
            .collect();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("#") {
                continue;
            }

            for re in &regexes {
                if re.is_match(trimmed) {
                    issues.push(Issue::new(
                        S2068B_RULE_ID,
                        "Hardcoded API key detected. Store API keys in environment variables or a secure secrets manager instead of hardcoding them.",
                        S2068B_SEVERITY,
                        S2068B_CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Use environment variables: std::env::var('API_KEY') or a secrets manager"
                    )));
                    break;
                }
            }
        }

        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// S532 — Password Logged in Plain Text
// ═══════════════════════════════════════════════════════════════════════════════

declare_rule! {
    id: "S532"
    name: "Password logged in plain text"
    severity: Critical
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Logging passwords exposes them in log files, which may be accessible to attackers or unauthorized personnel. Passwords should never appear in application logs."
    clean_code: Trustworthy,
    impacts: [Security: High],
    check: => {
        let mut issues = Vec::new();

        // Pattern: password being logged
        let logging_patterns = [
            // println! with password
            r#"(?i)println!\s*\([^)]*(?:\b|_)(?:password|passwd|pwd)\b"#,
            // log! and log:: macros (various frameworks)
            r#"(?i)(?:log|log::)\w*!\s*\([^)]*(?:password|passwd|pwd)"#,
            // eprintln!, format!
            r#"(?i)(?:eprintln!|format!|fprintln!)\s*\([^)]*(?:\b|_)(?:password|passwd|pwd)\b"#,
            // console.log (JavaScript/TypeScript)
            r#"(?i)console\.(?:log|info|debug|warn|error)\s*\([^)]*(?:password|passwd|pwd)"#,
            // Python logging
            r#"(?i)logging\.(?:info|debug|warning|error|critical)\s*\([^)]*(?:password|passwd|pwd)"#,
            // Java System.out.println with password
            r#"(?i)System\.(?:out|err)\.println\s*\([^)]*(?:password|passwd|pwd)"#,
        ];

        let regexes: Vec<_> = logging_patterns.iter()
            .filter_map(|p| regex::Regex::new(p).ok())
            .collect();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("#") {
                continue;
            }

            for re in &regexes {
                if re.is_match(trimmed) {
                    issues.push(Issue::new(
                        S532_RULE_ID,
                        "Password logged in plain text. Remove password logging or use structured logging that redacts sensitive fields.",
                        S532_SEVERITY,
                        S532_CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::quick(
                        "Remove password from log statements or use a placeholder like '***'"
                    )));
                    break;
                }
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(S256Rule::new())
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(S2068bRule::new())
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(S532Rule::new())
    }
}

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
    fn test_s256_rule_properties() {
        let rule = S256Rule::new();
        assert_eq!(rule.id(), "S256");
        assert_eq!(rule.name(), "Plain text credential storage detected");
        assert_eq!(rule.severity(), Severity::Blocker);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    #[test]
    fn test_s2068b_rule_properties() {
        let rule = S2068bRule::new();
        assert_eq!(rule.id(), "S2068b");
        assert_eq!(rule.name(), "Hardcoded API key detected");
        assert_eq!(rule.severity(), Severity::Blocker);
        assert_eq!(rule.category(), Category::SecurityHotspot);
    }

    #[test]
    fn test_s532_rule_properties() {
        let rule = S532Rule::new();
        assert_eq!(rule.id(), "S532");
        assert_eq!(rule.name(), "Password logged in plain text");
        assert_eq!(rule.severity(), Severity::Critical);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // S256 Positive Detection Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s256_detects_password_file_write() {
        let source = r#"
            std::fs::write("config.txt", password);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S256Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect password file write");
        assert_eq!(issues[0].rule_id, "S256");
    }

    #[test]
    fn test_s256_detects_password_in_log() {
        let source = r#"
            println!("User password: {}", password);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S256Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect password in println");
    }

    #[test]
    fn test_s256_detects_password_insert_db() {
        let source = r#"
            INSERT INTO users (password) VALUES ('secret123');
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S256Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect plain password INSERT");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // S2068b Positive Detection Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2068b_detects_api_key_assignment() {
        let source = r#"
            api_key = "sk_live_abc123xyz789def456";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S2068bRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect api_key assignment");
        assert_eq!(issues[0].rule_id, "S2068b");
    }

    #[test]
    fn test_s2068b_detects_authorization_header() {
        let source = r#"
            Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S2068bRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect Authorization header");
    }

    #[test]
    fn test_s2068b_detects_x_api_key() {
        let source = r#"
            x-api-key = "my_secret_api_key_here";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S2068bRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect x-api-key");
    }

    #[test]
    fn test_s2068b_detects_aws_access_key() {
        let source = r#"
            AWS_ACCESS_KEY_ID = "AKIAIOSFODNN7EXAMPLE";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S2068bRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect AWS access key");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // S532 Positive Detection Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s532_detects_password_in_println() {
        let source = r#"
            println!("Password is: {}", password);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S532Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect password in println");
        assert_eq!(issues[0].rule_id, "S532");
    }

    #[test]
    fn test_s532_detects_password_in_log_macro() {
        let source = r#"
            log::info!("Authenticating user with password: {}", password);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S532Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect password in log macro");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s256_false_positive_comment() {
        let source = r#"
            // std::fs::write("config.txt", password); // Don't do this
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S256Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect password in comment");
    }

    #[test]
    fn test_s2068b_false_positive_comment() {
        let source = r#"
            // api_key = "sk_live_abc123"; // This is fine
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S2068bRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect api_key in comment");
    }

    #[test]
    fn test_s532_false_positive_comment() {
        let source = r#"
            // println!("Password: {}", password);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S532Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect password in comment");
    }

    #[test]
    fn test_s532_false_positive_function_name() {
        let source = r#"
            fn get_password() -> String { "secret".to_string() }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S532Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect get_password function");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s256_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S256Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s2068b_edge_case_short_value() {
        let source = r#"
            api_key = "short";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S2068bRule::new();
            rule.check(ctx)
        });
        // Should not trigger because value is too short (< 16 chars)
        assert!(issues.is_empty(), "Should NOT detect api_key with short value");
    }
}
