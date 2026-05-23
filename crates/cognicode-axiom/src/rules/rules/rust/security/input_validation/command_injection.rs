//! S4721 — Command Injection via Shell Detection
//! Detects shell commands built with user input that could enable command injection (CWE-78).
//!
//! Languages: Rust, C, Python, JavaScript, Go
//! Severity: Blocker
//! Category: Vulnerability
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S4721
const RULE_ID: &str = "S4721";
const RULE_NAME: &str = "Shell command built with user input detected";
const SEVERITY: Severity = Severity::Blocker;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Patterns that indicate shell invocation with user input
static SHELL_COMMAND_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Rust: Command::new("sh") or Command::new("bash") with shell args
        regex::Regex::new(r#"Command::new\s*\(\s*["']sh["']\s*\)"#).unwrap(),
        regex::Regex::new(r#"Command::new\s*\(\s*["']bash["']\s*\)"#).unwrap(),
        regex::Regex::new(r#"Command::new\s*\(\s*["']cmd["']\s*\)"#).unwrap(),
        regex::Regex::new(r#"Command::new\s*\(\s*["']powershell["']\s*\)"#).unwrap(),
        // Rust: shell = true
        regex::Regex::new(r#"\.shell\s*\(\s*true\s*\)"#).unwrap(),
        regex::Regex::new(r#"spawn\s*\(\s*["']sh["']"#).unwrap(),
        // C: system(), popen(), exec*()
        regex::Regex::new(r#"\bsystem\s*\("#).unwrap(),
        regex::Regex::new(r#"\bpopen\s*\("#).unwrap(),
        regex::Regex::new(r#"\bexec(?:v|p|ve|vp)?\s*\("#).unwrap(),
        regex::Regex::new(r#"\bspawn\s*\("#).unwrap(),
        // Python: os.system(), os.popen(), subprocess with shell=True
        regex::Regex::new(r#"os\.system\s*\("#).unwrap(),
        regex::Regex::new(r#"os\.popen\s*\("#).unwrap(),
        regex::Regex::new(r#"subprocess\.run\s*\([^)]*shell\s*=\s*True"#).unwrap(),
        regex::Regex::new(r#"subprocess\.Popen\s*\([^)]*shell\s*=\s*True"#).unwrap(),
        // JavaScript: child_process with shell: true
        regex::Regex::new(r#"child_process\.(spawn|exec|execSync)\s*\([^)]*shell\s*:\s*true"#).unwrap(),
        regex::Regex::new(r#"exec\s*\(\s*["'][^"']+["']\s*\+"#).unwrap(),
        // Go: exec.Command with shell
        regex::Regex::new(r##"exec\.Command\s*\(\s*["']sh["']"##).unwrap(),
        regex::Regex::new(r##"exec\.Command\s*\(\s*["']bash["']"##).unwrap(),
    ]
});

/// Patterns for dangerous shell argument construction
static DANGEROUS_ARG_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Any arg() or args() call with variable containing shell metacharacters
        regex::Regex::new(r#"\.(?:arg|args)\s*\(\s*&"#).unwrap(),
        regex::Regex::new(r#"\.(?:arg|args)\s*\(\s*["'][^"']*\$"#).unwrap(),
        regex::Regex::new(r#"\.(?:arg|args)\s*\(\s*[a-z_][a-z_0-9]*\s*,"#).unwrap(),
        // Shell metacharacters in arguments
        regex::Regex::new(r#"(?:arg|args)\s*\(\s*[^)]*[;&|`$]"#).unwrap(),
    ]
});

/// Safe patterns - commands without user input
static SAFE_COMMAND_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Static commands with no user input
        regex::Regex::new(r#"Command::new\s*\(\s*["'][a-z]+["']\s*\)\s*\.arg\s*\(\s*["'][^$]+["']"#).unwrap(),
        // Commands with only literal string arguments
        regex::Regex::new(r#"\.arg\s*\(\s*["'][^${}()]+["']\s*\)"#).unwrap(),
        // exec with format that has no shell metacharacters
        regex::Regex::new(r#"(?i)Command::new.*\.arg\(.*\)\.arg\(.*\).*spawn\("#).unwrap(),
    ]
});

declare_rule! {
    id: "S4721"
    name: "Shell command built with user input detected"
    severity: Blocker
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Shell commands built with user input without proper sanitization are vulnerable to command injection attacks. Attackers can inject additional commands via shell metacharacters (;, |, &, $, `, etc.)."
    clean_code: Trustworthy,
    impacts: [Security: High],
    check: => {
        let mut issues = Vec::new();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("*/") || trimmed.starts_with("*")
               || trimmed.starts_with("#") {
                continue;
            }

            // Check for shell command patterns
            let has_shell_pattern = SHELL_COMMAND_PATTERNS.iter().any(|re| re.is_match(line));

            if has_shell_pattern {
                // Check if user input is being passed to the command
                let has_variable_arg = DANGEROUS_ARG_PATTERNS.iter().any(|re| re.is_match(line));
                let has_format_interpolation = line.contains("format!")
                    || line.contains("concat!")
                    || line.contains("${")
                    || line.contains("$(");

                // Check for shell metacharacters that indicate injection risk
                let has_shell_metachar = line.contains(';')
                    || line.contains("&&")
                    || line.contains("||")
                    || line.contains('|')
                    || line.contains('`')
                    || line.contains("$(")
                    || line.contains("${");

                if has_variable_arg || (has_format_interpolation && !SAFE_COMMAND_PATTERNS.iter().any(|re| re.is_match(line))) || has_shell_metachar {
                    issues.push(Issue::new(
                        RULE_ID,
                        format!("Shell command may incorporate user input unsafely - use Command::new() with explicit args instead of shell"),
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Avoid shell invocation with user input:\n\
                        - Use Command::new() with explicit args (safer): Command::new(\"ls\").arg(\"-la\")\n\
                        - Never pass user input directly to shell commands\n\
                        - If shell is required, validate and sanitize input against whitelist of allowed characters\n\
                        - Consider using absolute paths instead of shell commands"
                    )));
                }
            }

            // Additional check: system(), popen(), exec() family
            let dangerous_funcs = ["system(", "popen(", "exec(", "execve(", "execvp(", "execl(", "spawn("];
            for func in dangerous_funcs.iter() {
                if line.contains(func) && !trimmed.starts_with("//") {
                    // Check if there's user input being passed
                    let has_concatenation = line.contains('+')
                        || line.contains("format!")
                        || line.contains('"')
                        || line.contains('\'');

                    if has_concatenation || line.contains('$') {
                        issues.push(Issue::new(
                            RULE_ID,
                            format!("Dangerous function '{}' may incorporate user input - command injection risk", func.trim_end_matches('(')),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Replace with safer alternatives:\n\
                            - For system()/popen(): Use Command::new() with explicit args\n\
                            - Avoid passing user input to any shell function\n\
                            - Use absolute paths and explicit argument lists"
                        )));
                    }
                }
            }
        }

        issues
    }
}

// Note: inventory::submit! is auto-generated by declare_rule! macro

/// Agent semantics for S4721 - Command Injection via Shell
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S4721_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects shell commands that may incorporate user input unsafely, enabling command injection attacks where attackers can execute arbitrary commands",
    fix_playbook: "1. Identify shell command construction with user input\n\
                   2. Replace Command::new(\"sh\").arg(\"-c\").arg(input) with explicit args\n\
                   3. For Rust: Command::new(\"ls\").arg(\"-la\").arg(path) instead of shell\n\
                   4. For Python: Use subprocess.run([\"ls\", \"-la\"]) not shell=True\n\
                   5. For JavaScript: Use {shell: false} in child_process options\n\
                   6. If shell is unavoidable, validate input against strict whitelist\n\
                   7. Use allowlist for permitted characters only (alphanumeric, dash, underscore)",
    review_questions: &[
        "Is user input being passed to any shell command?",
        "Could an attacker control any part of the command arguments?",
        "Are there shell metacharacters in the input path?",
        "Is there a non-shell alternative using Command::new with explicit args?",
        "Is input validation strict enough to prevent injection?"
    ],
    agent_actions: &[
        "Identify shell command construction with user input",
        "Replace shell=true or sh -c with Command::new explicit args",
        "Verify no shell metacharacters in user input paths",
        "Suggest whitelist validation if shell is required",
        "Ensure no string concatenation in command construction"
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
    fn test_s4721_rule_properties() {
        let rule = S4721Rule::new();
        assert_eq!(rule.id(), "S4721");
        assert_eq!(rule.name(), "Shell command built with user input detected");
        assert_eq!(rule.severity(), Severity::Blocker);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4721_detects_shell_c_with_user_input() {
        let source = r#"
            Command::new("sh").arg("-c").arg(&user_input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect sh -c with user input");
        assert_eq!(issues[0].rule_id, "S4721");
    }

    #[test]
    fn test_s4721_detects_bash_c_with_user_input() {
        let source = r#"
            Command::new("bash").arg("-c").arg(user_input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect bash -c with user input");
    }

    #[test]
    fn test_s4721_detects_cmd_with_user_input() {
        let source = r#"
            std::process::Command::new("cmd").args(["/C", &user_cmd]);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect cmd /C with user input");
    }

    #[test]
    fn test_s4721_detects_system_with_concatenation() {
        let source = r#"
            system("ls -la " + user_path);
        "#;
        let issues = with_rule_context(source, Language::C, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect system() with string concatenation");
    }

    #[test]
    fn test_s4721_detects_popen_with_user_input() {
        let source = r#"
            FILE *fp = popen(user_cmd, "r");
        "#;
        let issues = with_rule_context(source, Language::C, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect popen() with user input");
    }

    #[test]
    fn test_s4721_detects_subprocess_shell_true() {
        let source = r#"
            subprocess.run(cmd, shell=True)
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect subprocess with shell=True");
    }

    #[test]
    fn test_s4721_detects_exec_with_concatenation() {
        let source = r#"
            exec("ls -la " + user_path)
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect exec() with concatenation");
    }

    #[test]
    fn test_s4721_detects_shell_metachar_injection() {
        let source = r#"
            Command::new("sh").arg("-c").arg("ls; rm -rf /");
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect shell metacharacters");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4721_false_positive_static_args() {
        let source = r#"
            Command::new("ls").arg("-la");
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static arguments");
    }

    #[test]
    fn test_s4721_false_positive_explicit_args() {
        let source = r#"
            Command::new("ls").arg("-la").arg("/path/to/dir");
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect explicit args without shell");
    }

    #[test]
    fn test_s4721_false_positive_comment() {
        let source = r#"
            // system("ls -la " + user_path);
        "#;
        let issues = with_rule_context(source, Language::C, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect command in comment");
    }

    #[test]
    fn test_s4721_false_positive_doc_comment() {
        let source = r#"
            /// Runs: system("ls -la")
            fn example() {}
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect command in doc comment");
    }

    #[test]
    fn test_s4721_false_positive_subprocess_no_shell() {
        let source = r#"
            subprocess.run(["ls", "-la"], shell=False)
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect subprocess without shell");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4721_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s4721_edge_case_multiple_commands() {
        let source = r#"
            Command::new("sh").arg("-c").arg(&cmd1);
            Command::new("sh").arg("-c").arg(&cmd2);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect multiple shell commands");
    }

    #[test]
    fn test_s4721_edge_case_var_injection() {
        let source = r#"
            Command::new("sh").arg("-c").arg(format!("ls {}", user_input));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4721Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect format! injection in shell command");
    }
}