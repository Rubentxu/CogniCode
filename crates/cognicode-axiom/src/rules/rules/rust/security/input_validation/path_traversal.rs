//! S2591 — Path Traversal Detection
//! Detects file operations with unsanitized user input in paths (CWE-22).
//!
//! Languages: Rust (std::fs, std::path), Python (open(), include()), Java (Files.read(), Path.of())
//! Severity: Critical
//! Category: Vulnerability

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S2591
const RULE_ID: &str = "S2591";
const RULE_NAME: &str = "Path traversal vulnerability detected";
const SEVERITY: Severity = Severity::Critical;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Pattern for path traversal sequences
static PATH_TRAVERSAL_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?:\.\.[/\\]|~|/etc/passwd|c:\\windows|c:\\boot)"#).unwrap()
});

/// Pattern for file operation functions with user input
static FILE_OPS_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Rust file operations
        regex::Regex::new(r#"std::fs::(?:read|write|read_to_string|read_dir|remove_file|remove_dir|copy|rename|canonicalize)\s*\("#).unwrap(),
        regex::Regex::new(r#"File::(?:open|create|with_extension|set_extension)\s*\("#).unwrap(),
        regex::Regex::new(r#"std::path::Path::(?:new|from|join)\s*\("#).unwrap(),
        regex::Regex::new(r#"include_bytes!\s*\("#).unwrap(),
        regex::Regex::new(r#"include_str!\s*\("#).unwrap(),
        regex::Regex::new(r#"std::fs::OpenOptions::new\(\).*(?:read|write|open)\s*\("#).unwrap(),
        // Python file operations
        regex::Regex::new(r#"(?i)open\s*\([^,)]+,"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:read|write|readlines|writelines)\s*\("#).unwrap(),
        // Java file operations
        regex::Regex::new(r#"Files\.(?:read|write|copy|move|delete|walk)\s*\("#).unwrap(),
        regex::Regex::new(r#"Path\.of\s*\("#).unwrap(),
        regex::Regex::new(r#"FileInputStream\s*\("#).unwrap(),
        regex::Regex::new(r#"FileReader\s*\("#).unwrap(),
        regex::Regex::new(r#"FileOutputStream\s*\("#).unwrap(),
    ]
});

/// Pattern to detect string concatenation/formatting with user input
static DYNAMIC_PATH_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"format!\s*\([^)]*(?:{}|{\d+}|{\w+})[^)]*""#).unwrap(),
        regex::Regex::new(r#"push_str\s*\("#).unwrap(),
        regex::Regex::new(r#"join\s*\([^)]*user"#).unwrap(),
        regex::Regex::new(r#"\+[^;]*user"#).unwrap(),
        regex::Regex::new(r#"format!("[^"]*{}"#).unwrap(),
    ]
});

/// Pattern for static/safe paths that should not trigger
static SAFE_PATH_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"std::fs::(?:read|write)\s*\(\s*"#).unwrap(),
        regex::Regex::new(r#"include_bytes!\s*\(\s*""#).unwrap(),
        regex::Regex::new(r#"include_str!\s*\(\s*""#).unwrap(),
        regex::Regex::new(r#"File::open\s*\(\s*""#).unwrap(),
    ]
});

declare_rule! {
    id: "S2591"
    name: "Path traversal vulnerability detected"
    severity: Critical
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "User input is used in file path operations without proper sanitization. Attackers can use path traversal sequences like '../' to access sensitive files outside the intended directory. This can lead to unauthorized file access, information disclosure, or remote code execution."
    clean_code: Trustworthy,
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

            // Check if this is a static safe path (not user input)
            let is_static_path = SAFE_PATH_PATTERNS.iter().any(|re| re.is_match(trimmed));
            if is_static_path {
                continue;
            }

            // Check for path traversal patterns in the line
            if PATH_TRAVERSAL_PATTERN.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    format!("Path traversal pattern detected in file operation: {}", trimmed),
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Validate and sanitize user input before using in file paths. Use allowlists for permitted values, remove '..' sequences, and ensure paths are within expected directories."
                )));
                continue;
            }

            // Check for file operations that might involve user input
            for re in FILE_OPS_PATTERNS.iter() {
                if re.is_match(trimmed) {
                    // Look ahead to see if user input is involved
                    let context: String = (0..3)
                        .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                        .take(4)
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Check if there's dynamic path construction or user input nearby
                    let has_user_input = context.to_lowercase().contains("user")
                        || context.to_lowercase().contains("input")
                        || context.to_lowercase().contains("request")
                        || context.to_lowercase().contains("param")
                        || context.to_lowercase().contains("query")
                        || context.to_lowercase().contains("args")
                        || context.contains("{}")
                        || context.contains("${");

                    let has_dynamic_path = DYNAMIC_PATH_PATTERNS.iter().any(|re| re.is_match(&context));

                    if has_user_input || has_dynamic_path {
                        issues.push(Issue::new(
                            RULE_ID,
                            format!("File operation may use unsanitized user input in path: {}", trimmed),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Validate that user input does not contain path traversal sequences. Use Path::canonicalize() to resolve paths and verify they are within the expected directory."
                        )));
                    } else {
                        // File operation without obvious user input - still warn if it looks like dynamic
                        if has_dynamic_path {
                            issues.push(Issue::new(
                                RULE_ID,
                                format!("Dynamic path construction detected in file operation: {}", trimmed),
                                SEVERITY,
                                CATEGORY,
                                ctx.file_path,
                                line_idx + 1,
                            ).with_remediation(Remediation::substantial(
                                "Ensure dynamic path components are properly validated and sanitized."
                            )));
                        }
                    }
                    break;
                }
            }
        }

        issues
    }
}


/// Agent semantics for S2591 - Path Traversal
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S2591_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects file operations with unsanitized user input in paths, allowing attackers to access files outside intended directories via '../' sequences",
    fix_playbook: "1. Identify all user-controlled input used in file paths\n2. Implement input validation using allowlists\n3. Remove or encode path traversal sequences ('..', '~', absolute paths)\n4. Use Path::canonicalize() to resolve and validate paths\n5. Ensure resolved path is within expected directory using starts_with()\n6. Consider using a safe wrapper function for all file operations\n7. Log all file access attempts for auditing",
    review_questions: &[
        "Is the file path derived from user input?",
        "Is the input properly validated against an allowlist?",
        "Are path traversal sequences removed or encoded?",
        "Is the final resolved path checked to be within the expected directory?",
        "Are file access attempts logged for security auditing?"
    ],
    agent_actions: &[
        "Identify file operations with user input",
        "Check for path canonicalization validation",
        "Verify allowlist-based input validation exists",
        "Ensure path traversal sequences are blocked",
        "Suggest Path::canonicalize() for path validation"
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
    fn test_s2591_rule_properties() {
        let rule = PATH_TRAVERSALRule::new();
        assert_eq!(rule.id(), "S2591");
        assert_eq!(rule.name(), "Path traversal vulnerability detected");
        assert_eq!(rule.severity(), Severity::Critical);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2591_detects_path_traversal_sequence() {
        let source = r#"
            let path = format!("uploads/{}", user_filename);
            std::fs::read(&path);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect path traversal via format!");
        assert_eq!(issues[0].rule_id, "S2591");
    }

    #[test]
    fn test_s2591_detects_file_open_with_user_input() {
        let source = r#"
            let file = File::open(&user_input_path);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect File::open with user input");
    }

    #[test]
    fn test_s2591_detects_std_fs_read_with_format() {
        let source = r#"
            let data = std::fs::read(format!("files/{}", request.param("name")));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect std::fs::read with format!");
    }

    #[test]
    fn test_s2591_detects_include_bytes_user_input() {
        let source = r#"
            let content = include_bytes!(user_input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect include_bytes! with user input");
    }

    #[test]
    fn test_s2591_detects_path_traversal_absolute() {
        let source = r#"
            std::fs::read("/etc/passwd");
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect path traversal absolute path");
    }

    #[test]
    fn test_s2591_detects_dotdot_sequence() {
        let source = r#"
            let path = format!("static/{}/../config", user_input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect ../ sequence");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2591_false_positive_static_path() {
        let source = r#"
            let data = std::fs::read("static/assets/file.txt");
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static string path");
    }

    #[test]
    fn test_s2591_false_positive_include_static() {
        let source = r#"
            let data = include_bytes!("assets/image.png");
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static include_bytes!");
    }

    #[test]
    fn test_s2591_false_positive_comment() {
        let source = r#"
            // std::fs::read(&user_path);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect file operation in comment");
    }

    #[test]
    fn test_s2591_false_positive_canonicalized_path() {
        let source = r#"
            let base = Path::new("/safe/dir");
            let canonical = base.canonicalize().unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect safe path canonicalization");
    }

    #[test]
    fn test_s2591_false_positive_string_literal_file_open() {
        let source = r#"
            let file = File::open("config/app.conf").unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect File::open with string literal");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2591_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s2591_edge_case_multiple_file_ops() {
        let source = r#"
            std::fs::read(user_input1);
            std::fs::write(user_input2, data);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect multiple file operations with user input");
    }

    #[test]
    fn test_s2591_edge_case_mixed_static_dynamic() {
        let source = r#"
            std::fs::read("static/file.txt");
            std::fs::read(user_path);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = PATH_TRAVERSALRule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect only the dynamic path");
    }
}