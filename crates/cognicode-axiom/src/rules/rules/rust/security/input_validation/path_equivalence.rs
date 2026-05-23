//! S5693 — Path Equivalence Detection
//! Detects multiple path representations that resolve to the same location,
//! bypassing path traversal protections (CWE-22).
//!
//! Languages: *
//! Severity: Major
//! Category: Vulnerability
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S5693
const RULE_ID: &str = "S5693";
const RULE_NAME: &str = "Path equivalence detected";
const SEVERITY: Severity = Severity::Major;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Pattern for path traversal sequences: /../ or /..\
static PATH_TRAVERSAL_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"/\.\.(?:/|$)|/\\|\.\.%(?:2f|2F|5c|5C)"#).unwrap()
});

/// Pattern for double slashes: //
static DOUBLE_SLASH_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"/{2,}"#).unwrap()
});

/// Pattern for current dir references: /./
static CURRENT_DIR_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"/\.(?:/|$)"#).unwrap()
});

/// Pattern for URL-encoded dot sequences: %2e%2e
static URL_ENCODED_DOTS_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)%2e%2e|%2e\.(?:%2f|%5c|/)"#).unwrap()
});

/// Pattern for semicolon path separators: ..;
static SEMICOLON_PATH_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"\.\.;|;\.\."#).unwrap()
});

/// Pattern for trailing slash variations
static TRAILING_SLASH_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"/+$"#).unwrap()
});

/// Pattern for user-controlled path concatenation with dangerous patterns
static USER_PATH_CONCAT_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // format! or concat! with user input and traversal
        regex::Regex::new(r#"(?i)(?:format|concat|concat_string)\s*\([^)]*\)\s*.*(?:/\.\./|/\.\./|/%2e%2e)"#).unwrap(),
        // String concatenation with .. patterns
        regex::Regex::new(r#"(?i)(?:\+|&format|format!)\s*\([^)]*\)\s*(?:/|%2e)"#).unwrap(),
        // PathBuf::from or Path::new with concatenation
        regex::Regex::new(r#"(?i)(?:PathBuf::from|Path::new|Path::join)\s*\([^)]*\)\s*(?:/|%2e)"#).unwrap(),
        // join with user input
        regex::Regex::new(r#"(?i)\.join\s*\([^)]*\)"#).unwrap(),
    ]
});

/// Pattern for file path operations that might use user input
static FILE_OPS_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Rust file operations
        regex::Regex::new(r#"(?i)(?:std::fs::|File::|OpenOptions)".*#).unwrap(),
        regex::Regex::new(r#"(?i)(?:read|write|read_dir|remove|rename|copy)\s*\([^)]*\)"#).unwrap(),
        // actix-web static files
        regex::Regex::new(r#"(?i)(?:static_files?|NamedFile|fs)"#).unwrap(),
        // rocket static files
        regex::Regex::new(r#"(?i)Asset::"#).unwrap(),
    ]
});

declare_rule! {
    id: "S5693"
    name: "Path equivalence detected"
    severity: Major
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Path equivalence sequences (e.g., /../, /./, //, %2e%2e) can be used to bypass path traversal protections. An attacker could access files outside the intended directory by manipulating path representations."
    clean_code: Clear,
    impacts: [Security: High],
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

            // Check for path traversal patterns
            if PATH_TRAVERSAL_PATTERN.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "Path traversal sequence detected (/../). This could allow access to files outside the intended directory.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Validate and sanitize path input. Use canonicalize() to resolve paths and verify they remain within the allowed directory."
                )));
            }

            // Check for double slash patterns that might be dangerous
            if DOUBLE_SLASH_PATTERN.is_match(trimmed) {
                // Look for context indicating user-controlled path
                let has_user_input = USER_PATH_CONCAT_PATTERNS.iter().any(|p| p.is_match(trimmed));
                let has_file_ops = FILE_OPS_PATTERNS.iter().any(|p| p.is_match(trimmed));

                if has_user_input || has_file_ops {
                    issues.push(Issue::new(
                        RULE_ID,
                        "Multiple slash sequence detected in potential user-controlled path. This could be used to bypass path restrictions.",
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Normalize paths by removing duplicate slashes and validate the final path stays within the allowed directory."
                    )));
                }
            }

            // Check for current directory references
            if CURRENT_DIR_PATTERN.is_match(trimmed) {
                let has_user_input = USER_PATH_CONCAT_PATTERNS.iter().any(|p| p.is_match(trimmed));
                let has_file_ops = FILE_OPS_PATTERNS.iter().any(|p| p.is_match(trimmed));

                if has_user_input || has_file_ops {
                    issues.push(Issue::new(
                        RULE_ID,
                        "Current directory reference (/./) detected in path. This could bypass path traversal protections.",
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Remove /./ sequences from path input and validate the resulting path."
                    )));
                }
            }

            // Check for URL-encoded dot sequences
            if URL_ENCODED_DOTS_PATTERN.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "URL-encoded path traversal (%2e%2e) detected. This is a common bypass technique.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Decode and sanitize URL-encoded path input. Validate decoded paths against allowed directories."
                )));
            }

            // Check for semicolon path separators
            if SEMICOLON_PATH_PATTERN.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "Semicolon path separator detected. This could be used to inject additional path components.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Reject paths containing semicolons as they are not valid path separators in most filesystems."
                )));
            }

            // Check for user path concatenation patterns
            for pattern in USER_PATH_CONCAT_PATTERNS.iter() {
                if pattern.is_match(trimmed) {
                    // Look for dangerous patterns in the concatenated value
                    let context: String = (0..3)
                        .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                        .take(4)
                        .collect::<Vec<_>>()
                        .join("\n");

                    if PATH_TRAVERSAL_PATTERN.is_match(&context)
                       || URL_ENCODED_DOTS_PATTERN.is_match(&context)
                       || SEMICOLON_PATH_PATTERN.is_match(&context) {
                        issues.push(Issue::new(
                            RULE_ID,
                            "User-controlled path concatenation with potential traversal patterns detected.",
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Use canonicalize() to resolve the final path and verify it stays within the allowed directory."
                        )));
                        break;
                    }
                }
            }
        }

        issues
    }
}


/// Agent semantics for S5693 - Path Equivalence
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S5693_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects path equivalence sequences (/../, /./, //, %2e%2e) that can bypass path traversal protections and access files outside intended directories",
    fix_playbook: "1. Identify all user-controlled path inputs\n2. Check for path equivalence patterns: /../, /./, //, %2e%2e, ..;\n3. Use canonicalize() to resolve paths to absolute form\n4. Verify resolved path stays within allowed directory using starts_with()\n5. Reject paths with null bytes or other invalid characters\n6. Consider using a whitelist of allowed paths",
    review_questions: &[
        "Is this path derived from user input?",
        "What is the base directory this path should be restricted to?",
        "Does the application need to handle symbolic links in paths?",
        "Are there any allowed exceptions for path traversal?",
    ],
    agent_actions: &[
        "Identify user-controlled path sources",
        "Detect path equivalence patterns in path construction",
        "Check for proper path canonicalization",
        "Verify path containment within allowed directories",
        "Suggest proper path validation approach",
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
    fn test_s5693_rule_properties() {
        let rule = S5693Rule::new();
        assert_eq!(rule.id(), "S5693");
        assert_eq!(rule.name(), "Path equivalence detected");
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5693_detects_path_traversal() {
        let source = r#"
            let path = format!("/images/../{}", user_input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect /../ path traversal");
        assert_eq!(issues[0].rule_id, "S5693");
    }

    #[test]
    fn test_s5693_detects_current_dir_reference() {
        let source = r#"
            let path = format!("/static/./{}", user_input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect /./ current directory reference");
    }

    #[test]
    fn test_s5693_detects_double_slash() {
        let source = r#"
            let path = format!("/images//{}", user_input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect double slash pattern");
    }

    #[test]
    fn test_s5693_detects_url_encoded_traversal() {
        let source = r#"
            let url = "/images/%2e%2e/%2e%2e/etc/passwd";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect %2e%2e URL-encoded traversal");
    }

    #[test]
    fn test_s5693_detects_semicolon_separator() {
        let source = r#"
            let path = "/images/..;/etc/passwd";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect semicolon path separator");
    }

    #[test]
    fn test_s5693_detects_pathbuf_join() {
        let source = r#"
            let path = PathBuf::from("/static").join(&user_input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect PathBuf::join with user input");
    }

    #[test]
    fn test_s5693_detects_multiple_traversal() {
        let source = r#"
            let path = "/images/../../etc/passwd";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect multiple /../ sequences");
    }

    #[test]
    fn test_s5693_detects_backslash_traversal() {
        let source = r#"
            let path = "C:\\..\\..\\Windows\\System32";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect backslash traversal");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5693_false_positive_static_path() {
        let source = r#"
            let path = "/images/normal/file.jpg";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static safe path");
    }

    #[test]
    fn test_s5693_false_positive_comment() {
        let source = r#"
            // Path: /images/../user/upload - this is safe
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect path in comment");
    }

    #[test]
    fn test_s5693_false_positive_version_number() {
        let source = r#"
            let version = "1.2.3";
            let path = "/images/v1.2.3/icon.png";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        // Should not trigger on version-like patterns
        assert!(issues.is_empty(), "Should NOT detect version numbers in paths");
    }

    #[test]
    fn test_s5693_false_positive_canonicalized_path() {
        let source = r#"
            use std::fs;
            let canonical = fs::canonicalize("/images/normal/file.jpg").unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        // fs::canonicalize is safe - paths are resolved
        assert!(issues.is_empty(), "Should NOT detect paths using canonicalize");
    }

    #[test]
    fn test_s5693_false_positive_whitelisted() {
        let source = r#"
            if path.starts_with("/allowed/") {
                serve_file(&path);
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect properly validated paths");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5693_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s5693_edge_case_single_slash() {
        let source = "let path = \"/\";";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on single slash");
    }

    #[test]
    fn test_s5693_edge_case_mixed_operators() {
        let source = r#"
            // Handle both Unix and Windows paths
            let unix = "/images/../user/file";
            let windows = "C:\images\..\..\system";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect traversal in both formats");
    }

    #[test]
    fn test_s5693_edge_case_nested_traversal() {
        let source = r#"
            let malicious = "/images/./.././../../../etc/passwd";
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5693Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect nested path equivalence");
    }
}