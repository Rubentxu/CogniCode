//! S4811 — LDAP Injection Detection
//! Detects LDAP queries with unsanitized user input (CWE-90).
//!
//! Languages: Rust, Python, Java, C#, PHP, JavaScript
//! Severity: Major
//! Category: Vulnerability
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S4811
const RULE_ID: &str = "S4811";
const RULE_NAME: &str = "Potential LDAP injection detected";
const SEVERITY: Severity = Severity::Major;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Pattern for LDAP search operations with string formatting (dangerous)
static LDAP_SEARCH_DYNAMIC: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Rust ldap3 crate - search with format!
        regex::Regex::new(r#"(?i)ldap3\s*::\s*(?:Conn|Connection)\s*::\s*search\s*\(.*format!"#).unwrap(),
        regex::Regex::new(r#"(?i)\.search\s*\(.*format\s*\("#).unwrap(),
        // Rust ldap crate
        regex::Regex::new(r#"(?i)ldap\s*::\s*(?:Conn|Connection)\s*::\s*search\s*\(.*\+"#).unwrap(),
        // Python ldap3
        regex::Regex::new(r#"(?i)ldap3\s*\.search(?:_s|_ext)?\s*\(.*format\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)ldap\.search(?:_s|_ext)?\s*\(.*%"#).unwrap(),
        // Java JNDI LDAP
        regex::Regex::new(r#"(?i)DirContext\s*\.search\s*\(.*\+"|#).unwrap(),
        regex::Regex::new(r#"(?i)InitialLdapContext\s*\.search\s*\(.*\+"|#).unwrap(),
        // C# System.DirectoryServices
        regex::Regex::new(r#"(?i)DirectorySearcher\s*\.Filter\s*=|#).unwrap(),
        regex::Regex::new(r#"(?i)DirectoryEntry\s*\.Path\s*="#).unwrap(),
        // PHP ldap_search
        regex::Regex::new(r#"(?i)ldap_search\s*\(.*\."#).unwrap(),
        regex::Regex::new(r#"(?i)ldap_search\s*\(.*\$"#).unwrap(),
    ]
});

/// Pattern for LDAP query construction with concatenation
static LDAP_CONCAT_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // String concatenation with user input in LDAP context
        regex::Regex::new(r#"(?i)"#).unwrap(),
        regex::Regex::new(r#"(?i)format\s*\(\s*"#).unwrap(),
        regex::Regex::new(r#"(?i)concat\s*\(\s*"#).unwrap(),
        // Replace with user input
        regex::Regex::new(r#"(?i)"\.replace\s*\("*\$"#).unwrap(),
        regex::Regex::new(r#"(?i)\.replaceAll\s*\("*\$"#).unwrap(),
        // f-string / interpolated strings
        regex::Regex::new(r#"f"[^"]*\{[^}]*\}"#).unwrap(),
        regex::Regex::new(r#"format!\s*\("[^"]*\{[^}]*\}"#).unwrap(),
    ]
});

/// Pattern for LDAP filter building blocks that are dangerous when user-controlled
static LDAP_FILTER_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Common LDAP filter components with user input
        regex::Regex::new(r#"(?i)\(uid\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(sn\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(cn\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(mail\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(givenName\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(ou\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(objectClass\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(member\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(manager\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(uniqueMember\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(owner\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(role\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\(group\s*="#).unwrap(),
        // LDAP meta-characters that can be injected
        regex::Regex::new(r#"\*"#).unwrap(),
        regex::Regex::new(r#"\("#).unwrap(),
        regex::Regex::new(r#"\)"#).unwrap(),
        regex::Regex::new(r#"\\"#).unwrap(),
        regex::Regex::new(r#"\/"#).unwrap(),
        regex::Regex::new(r#"NUL|#").unwrap(),
    ]
});

/// Pattern for user-controlled input indicators in LDAP context
static LDAP_USER_INPUT_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Generic user input indicators
        regex::Regex::new(r#"(?i)(?:username|user_name|userid|user_id|uid)\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)(?:password|passwd|pwd)\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)(?:email|e_mail|mail)\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)(?:name|cn|common_name)\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)(?:first_name|last_name|sn|givenName)\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)(?:department|dept|ou)\s*="#).unwrap(),
        // HTTP request parameters
        regex::Regex::new(r#"(?i)(?:request|req)\s*\.\s*(?:params|query|body|get)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:param|query)\s*\(\s*["'][^"']+["']\s*\)"#).unwrap(),
        // Function arguments
        regex::Regex::new(r#"(?i)fn\s+\w+\s*\([^)]*:\s*String\)"#).unwrap(),
    ]
});

/// Safe LDAP patterns (should NOT be flagged)
static LDAP_SAFE_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Parameterized/prepared LDAP queries
        regex::Regex::new(r#"(?i)search_filter\s*="#).unwrap(),
        // Static filters
        regex::Regex::new(r#"search\s*\(\s*"[\s\S]*?"\s*,\s*\["#).unwrap(),
        // Using ldap escape functions
        regex::Regex::new(r#"(?i)ldap3\.escape\.filter_chars"#).unwrap(),
        regex::Regex::new(r#"(?i)ldap\.escape_filter_chars"#).unwrap(),
        regex::Regex::new(r#"(?i)escape_str\( "#).unwrap(),
    ]
});

declare_rule! {
    id: "S4811"
    name: "Potential LDAP injection detected"
    severity: Major
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "LDAP injection occurs when user input is incorporated into LDAP queries without proper sanitization. Attackers can manipulate LDAP queries to bypass authentication, access unauthorized information, or execute arbitrary commands on the LDAP server. LDAP injection is a critical vulnerability (OWASP Top 10) that can lead to complete system compromise."
    clean_code: Trustworthy,
    impacts: [Security: High, Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("#") || trimmed.starts_with("*") {
                continue;
            }

            // Check for safe patterns first
            let is_safe = LDAP_SAFE_PATTERNS.iter()
                .any(|re| re.is_match(trimmed));
            if is_safe {
                continue;
            }

            // Check for dynamic LDAP search with string formatting
            let has_dynamic_search = LDAP_SEARCH_DYNAMIC.iter()
                .any(|re| re.is_match(trimmed));

            if has_dynamic_search {
                let has_user_input = LDAP_USER_INPUT_PATTERNS.iter()
                    .any(|re| re.is_match(trimmed))
                    || Self::has_ldap_meta_chars(trimmed);

                if has_user_input {
                    issues.push(Issue::new(
                        RULE_ID,
                        "Potential LDAP injection: LDAP query constructed with unsanitized user input. LDAP meta-characters (*, (, ), \\, /, NUL) can be used to manipulate queries.",
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Escape LDAP filter special characters using ldap3.escape_filter_chars() or equivalent. Use parameterized LDAP queries if available. Validate and whitelist user input before using in LDAP filters."
                    )));
                    continue;
                }
            }

            // Check for LDAP filter components with potential user input
            let has_filter_component = LDAP_FILTER_PATTERNS.iter()
                .any(|re| re.is_match(trimmed));

            if has_filter_component {
                // Check if this is part of a query with user input
                let has_user_input = LDAP_USER_INPUT_PATTERNS.iter()
                    .any(|re| re.is_match(trimmed))
                    || Self::has_ldap_meta_chars(trimmed)
                    || Self::is_in_ldap_context(line_idx, ctx);

                if has_user_input {
                    issues.push(Issue::new(
                        RULE_ID,
                        "Potential LDAP injection: LDAP filter contains user-controlled input without proper escaping.",
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Escape LDAP filter special characters: *, (, ), \\, /, NUL, space, #. Use ldap3.escape_filter_chars() or implement proper LDAP escaping."
                    )));
                    continue;
                }
            }

            // Check for string concatenation in LDAP context
            let has_concat = LDAP_CONCAT_PATTERNS.iter()
                .any(|re| re.is_match(trimmed));

            if has_concat && Self::is_in_ldap_context(line_idx, ctx) {
                issues.push(Issue::new(
                    RULE_ID,
                    "Potential LDAP injection: String concatenation used in LDAP query construction.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Use LDAP escaping functions instead of string concatenation. Never directly concatenate user input into LDAP filters."
                )));
            }
        }

        issues
    }
}

impl S4811Rule {
    /// Check if a line contains LDAP meta-characters that could be injection
    fn has_ldap_meta_chars(line: &str) -> bool {
        let meta_chars = ["*", "(", ")", "\\", "/", "NUL", "#", " "];
        let line_lower = line.to_lowercase();

        // Check for suspicious patterns like (uid=*) or (sn=*)
        for meta in &meta_chars {
            if line.contains(meta) {
                // Check if it's in LDAP filter context
                if line_lower.contains("uid=")
                    || line_lower.contains("sn=")
                    || line_lower.contains("cn=")
                    || line_lower.contains("mail=")
                    || line_lower.contains("ou=")
                    || line_lower.contains("objectclass=")
                {
                    return true;
                }
            }
        }

        false
    }

    /// Check if the line is in an LDAP context (check surrounding lines)
    fn is_in_ldap_context(line_idx: usize, ctx: &RuleContext) -> bool {
        let context_window = 3;

        for i in 0..=context_window {
            let offset = line_idx.saturating_sub(i);
            if let Some(prev_line) = ctx.source.lines().nth(offset) {
                let prev_lower = prev_line.to_lowercase();
                // Check for LDAP-related keywords in context
                if prev_lower.contains("ldap")
                    || prev_lower.contains("search")
                    || prev_lower.contains("filter")
                    || prev_lower.contains("query")
                    || prev_lower.contains("directory")
                    || prev_lower.contains("active_directory")
                    || prev_lower.contains("adfs")
                    || prev_lower.contains("openldap")
                {
                    return true;
                }
            }
        }

        false
    }
}


/// Agent semantics for S4811 - LDAP Injection
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S4811_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects LDAP queries constructed with unsanitized user input, which can allow attackers to manipulate LDAP queries for unauthorized access or information disclosure",
    fix_playbook: "1. Identify all LDAP query construction points\n2. Determine if user input is being incorporated into LDAP filters\n3. Use LDAP escape functions: ldap3.escape_filter_chars() for Python, ldap.escape_filter_chars() for ldap library\n4. Encode all user input before using in LDAP filters\n5. Consider using parameterized LDAP queries if available\n6. Implement input validation: whitelist characters if possible\n7. LDAP meta-characters to escape: * ( ) \\ / NUL # space\n8. Test with LDAP injection payloads: *, ), (, \\, /",
    review_questions: &[
        "What user inputs are incorporated into LDAP queries?",
        "Is the LDAP connection protected from injection?",
        "Are LDAP meta-characters being escaped?",
        "What is the LDAP server configuration and privileges?",
        "Could an attacker bypass authentication with LDAP injection?",
        "What data could an attacker access through LDAP injection?",
        "Is input validation applied before LDAP query construction?"
    ],
    agent_actions: &[
        "Identify all LDAP query construction locations",
        "Trace user input sources going into LDAP queries",
        "Verify LDAP escaping functions are used",
        "Check for LDAP meta-characters in user input paths",
        "Look for ldap3, rldap, ldap3 crate usage",
        "Verify proper escaping of special characters (*, (, ), \\, /, #, NUL)"
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
    fn test_s4811_rule_properties() {
        let rule = S4811Rule::new();
        assert_eq!(rule.id(), "S4811");
        assert_eq!(rule.name(), "Potential LDAP injection detected");
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule (Rust)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4811_detects_ldap_search_with_format() {
        let source = r#"
            conn.search(&format!("(uid={})", username), Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect LDAP search with format!");
        assert_eq!(issues[0].rule_id, "S4811");
    }

    #[test]
    fn test_s4811_detects_ldap_search_with_username() {
        let source = r#"
            let filter = format!("(sn={})", &user_input);
            conn.search(&filter, Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect LDAP search with user_input");
    }

    #[test]
    fn test_s4811_detects_ldap_concat() {
        let source = r#"
            let filter = "(uid=".to_string() + &username + ")";
            conn.search(&filter, Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect LDAP search with string concatenation");
    }

    #[test]
    fn test_s4811_detects_replace_in_ldap_filter() {
        let source = r#"
            let filter = "(uid=*)".replace("*", &user_input);
            conn.search(&filter, Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect replace() in LDAP filter");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Python
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4811_detects_python_ldap3_format() {
        let source = r#"
            results = conn.search('dc=example,dc=com', ldap3.SUBTREE, '({}={})'.format('uid', username))
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect Python ldap3 search with format");
    }

    #[test]
    fn test_s4811_detects_python_ldap_percent() {
        let source = r#"
            results = conn.search_s('dc=example,dc=com', ldap3.SUBTREE, '(uid=%s)' % username)
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect Python ldap with % formatting");
    }

    #[test]
    fn test_s4811_detects_python_ldap_concat() {
        let source = r#"
            filter = "(sn=" + lastname + ")"
            results = conn.search_s('dc=example,dc=com', ldap3.SUBTREE, filter)
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect Python ldap with string concat");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Java
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4811_detects_java_jndi_ldap() {
        let source = r#"
            DirContext ctx = new InitialDirContext(env);
            SearchResult result = ctx.search("ou=users", "uid=" + username, controls);
        "#;
        let issues = with_rule_context(source, Language::Java, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect Java JNDI LDAP injection");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — C#
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4811_detects_cs_directory_searcher() {
        let source = r#"
            DirectorySearcher searcher = new DirectorySearcher();
            searcher.Filter = "(uid=" + username + ")";
            SearchResultCollection results = searcher.FindAll();
        "#;
        let issues = with_rule_context(source, Language::CSharp, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect C# DirectorySearcher LDAP injection");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — PHP
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4811_detects_php_ldap_search() {
        let source = r#"
            $filter = "(uid=" . $_GET['username'] . ")";
            $result = ldap_search($ldapconn, $base, $filter);
        "#;
        let issues = with_rule_context(source, Language::PHP, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect PHP ldap_search injection");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — LDAP Meta-characters
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4811_detects_ldap_wildcard_injection() {
        let source = r#"
            let filter = format!("(uid={})", "*");
            conn.search(&filter, Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect LDAP wildcard injection");
    }

    #[test]
    fn test_s4811_detects_ldap_paren_injection() {
        let source = r#"
            let filter = format!("(sn={})", "admin)(cn=*");
            conn.search(&filter, Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect LDAP parenthesis injection");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4811_false_positive_static_filter() {
        let source = r#"
            conn.search("(uid=admin)", Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static LDAP filter");
    }

    #[test]
    fn test_s4811_false_positive_escaped_input() {
        let source = r#"
            let escaped = ldap3::escape_filter_chars(&username);
            let filter = format!("(uid={})", escaped);
            conn.search(&filter, Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect properly escaped LDAP filter");
    }

    #[test]
    fn test_s4811_false_positive_comment() {
        let source = r#"
            // conn.search(&format!("(uid={})", username), Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect LDAP search in comment");
    }

    #[test]
    fn test_s4811_false_positive_doc_comment() {
        let source = r#"
            /// Search example: conn.search("(uid=user)", Scope::Subtree, ...)
            fn search() {}
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect LDAP search in doc comment");
    }

    #[test]
    fn test_s4811_false_positive_prepared_query() {
        let source = r#"
            let search_filter = "(uid=admin)";
            conn.search(&search_filter, Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static search filter");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4811_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s4811_edge_case_single_line() {
        let source = r#"conn.search(&format!("(cn={})", name), Scope::Subtree, None, None)?;"#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect LDAP injection on single line");
    }

    #[test]
    fn test_s4811_edge_case_multiple_queries() {
        let source = r#"
            conn.search(&format!("(uid={})", user1), Scope::Subtree, None, None)?;
            conn.search("(ou=static)", Scope::Subtree, None, None)?;
            conn.search(&format!("(sn={})", user2), Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        // Should detect first and third, but not second (static filter)
        assert!(issues.len() >= 2, "Should detect multiple unsafe LDAP queries");
    }

    #[test]
    fn test_s4811_edge_case_multiline_context() {
        let source = r#"
            // LDAP search setup
            let username = get_user_input();
            // Perform search
            let filter = format!("(uid={})", username);
            conn.search(&filter, Scope::Subtree, None, None)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect LDAP injection with multiline context");
    }

    #[test]
    fn test_s4811_edge_case_case_insensitive() {
        let source = r#"
            let filter = Format!("(SN={})", USERNAME);
            CONN.SEARCH(&filter, SCOPE::SUBTREE, NONE, NONE)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4811Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should handle case-insensitive LDAP keywords");
    }
}
