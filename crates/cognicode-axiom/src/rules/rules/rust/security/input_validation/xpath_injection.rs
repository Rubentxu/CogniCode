//! S4813 — XPath Injection Detection
//! Detects XPath queries built with unsanized user input, allowing attackers to manipulate query logic.
//!
//! Languages: Rust
//! Severity: Major
//! Category: Vulnerability
//!
//! ## Description
//! XPath injection vulnerabilities occur when user input is directly concatenated into XPath
//! query strings without proper sanitization. Attackers can manipulate the query to access
//! unauthorized data or bypass authentication.
//!
//! ## Vulnerable Patterns
//! - `format!("//user[@name='{}']", user_input)` - direct string interpolation
//! - `"/root/element[contains(text(), '{}')]".replace("{}", &user_input)` - replace patterns
//! - `xpath_expr += &user_input` - string concatenation
//!
//! ## Safe Patterns
//! - Parameterized XPath queries (if supported by library)
//! - Input validation/sanitization before query construction
//! - Using libraries that support query parameterization

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S4813
const RULE_ID: &str = "S4813";
const RULE_NAME: &str = "XPath injection vulnerability detected";
const SEVERITY: Severity = Severity::Major;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Pattern for detecting xpath-related function calls
static XPATH_FN_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Rust xpath crate patterns
        regex::Regex::new(r"(?i)xpath\s*::\s*Dom\s*::\s*parse").unwrap(),
        regex::Regex::new(r"(?i)\.select_xpath\s*\(").unwrap(),
        regex::Regex::new(r"(?i)xpath\s*\(\s*&").unwrap(),
        // sxd-xpath crate patterns
        regex::Regex::new(r"(?i)sxd_xpath\s*::\s*.*\s*parse").unwrap(),
        regex::Regex::new(r"(?i)sxd_xpath\s*::\s*.*\s*evaluate").unwrap(),
        // General xpath evaluation
        regex::Regex::new(r"(?i)\.xpath\s*\(").unwrap(),
        regex::Regex::new(r"(?i)evaluate_xpath\s*\(").unwrap(),
        regex::Regex::new(r"(?i)query\s*\([^)]*//[^)]*\)").unwrap(),
    ]
});

/// Pattern for detecting dangerous string interpolation/concatenation in XPath context
static XPATH_DYNAMIC_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)(?:format!\s*\([^)]*//[^)]*\}|"[^"]*\{}[^"]*"|\.replace\s*\(\s*["'][^"']*\{}[^"']*["']|^\s*\w+\s*\+=|concat!\s*\("#).unwrap()
});

/// Pattern for detecting format! macro with xpath content
static XPATH_FORMAT_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)format!\s*\(\s*["'][^"']*//[^"']*\{["']"#).unwrap()
});

/// Pattern for detecting string replace operations that could inject xpath
static XPATH_REPLACE_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)["'][^"']*//[^"']*["']\s*\.\s*replace\s*\("#).unwrap()
});

/// Known sanitization functions that make input safe
static SANITIZATION_FUNCTIONS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r"(?i)escape_xpath|xpath_escape|sanitize_xpath|html_escape|escape_html").unwrap(),
        regex::Regex::new(r"(?i)ammonia|bleach|dompurify|sanitize\s*\(").unwrap(),
        regex::Regex::new(r"(?i)\.text\(\)|\.inner_text\(\)|\.content\(\)").unwrap(),
    ]
});

declare_rule! {
    id: "S4813"
    name: "XPath injection vulnerability detected"
    severity: Major
    category: Vulnerability
    language: "Rust"
    params: {}

    explanation: "XPath queries should not be constructed from unsanitized user input. Attackers can manipulate XPath expressions to access unauthorized data or bypass authentication checks."
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

            // Check for dynamic xpath patterns (format!, replace, concat)
            if XPATH_FORMAT_PATTERN.is_match(trimmed) || XPATH_REPLACE_PATTERN.is_match(trimmed) {
                // Check if there's any sanitization nearby
                let context_lines: String = (0..3)
                    .filter_map(|i| ctx.source.lines().nth(line_idx.saturating_sub(i)))
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("\n");

                let has_sanitization = SANITIZATION_FUNCTIONS.iter()
                    .any(|re| re.is_match(&context_lines));

                if !has_sanitization {
                    issues.push(Issue::new(
                        RULE_ID,
                        "XPath query constructed from dynamic string. This is vulnerable to XPath injection if user input reaches this point.".to_string(),
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Use parameterized XPath queries or sanitize user input with xpath_escape() before including in XPath expressions"
                    )));
                }
            }

            // Check for xpath function calls with dynamic content
            for xpath_re in XPATH_FN_PATTERNS.iter() {
                if xpath_re.is_match(trimmed) {
                    // Look ahead for dynamic content indicators
                    let context: String = (0..5)
                        .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                        .take(6)
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Check for interpolation or concatenation with variables
                    if context.contains("{}") || context.contains("format!")
                       || context.contains("replace") || context.contains("+=")
                       || context.contains("concat!") || context.contains("format_args")
                    {
                        // Check for sanitization in context
                        let has_sanitization = SANITIZATION_FUNCTIONS.iter()
                            .any(|re| re.is_match(&context));

                        if !has_sanitization {
                            issues.push(Issue::new(
                                RULE_ID,
                                "XPath query may contain user input without sanitization. This could allow XPath injection attacks.".to_string(),
                                SEVERITY,
                                CATEGORY,
                                ctx.file_path,
                                line_idx + 1,
                            ).with_remediation(Remediation::substantial(
                                "Validate and sanitize all user input before using in XPath queries. Consider using a safer API that separates query structure from data."
                            )));
                        }
                    }
                }
            }

            // Check for direct string concatenation patterns
            if XPATH_DYNAMIC_PATTERN.is_match(trimmed) && trimmed.contains("//") {
                // Additional check: see if variable interpolation is happening
                if trimmed.contains("${") || trimmed.contains("{}") || trimmed.matches('\'').count() >= 2 {
                    let has_sanitization = SANITIZATION_FUNCTIONS.iter()
                        .any(|re| re.is_match(trimmed));

                    if !has_sanitization {
                        issues.push(Issue::new(
                            RULE_ID,
                            "Dynamic XPath expression with potential user input. Ensure proper input sanitization.".to_string(),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Sanitize user input or use parameterized XPath queries"
                        )));
                    }
                }
            }
        }

        issues
    }
}


/// Agent semantics for S4813 - XPath Injection
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S4813_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects XPath queries constructed from unsanitized user input, which can allow attackers to manipulate query logic and access unauthorized data",
    fix_playbook: "1. Identify where XPath queries are built with user input\n2. Replace string concatenation with parameterized XPath if library supports it\n3. If parameterization not available, implement input sanitization using xpath_escape()\n4. Validate input against allowlist patterns (e.g., alphanumeric only)\n5. Consider using a safer data access layer that doesn't rely on string-based XPath\n6. Test with payloads like: ' or '1'='1, ]//*|text()|//*, etc.",
    review_questions: &[
        "Does this XPath query receive any user-controlled input?",
        "What is the source of the data being interpolated into the XPath?",
        "Is there input validation or sanitization in place?",
        "What data can be accessed if the injection succeeds?",
        "Can this query be rewritten to avoid dynamic string building?"
    ],
    agent_actions: &[
        "Identify XPath query construction points",
        "Trace user input sources to XPath queries",
        "Check for xpath_escape or similar sanitization",
        "Verify input validation allows only safe characters",
        "Suggest parameterized XPath alternatives"
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
    fn test_s4813_rule_properties() {
        let rule = S4813Rule::new();
        assert_eq!(rule.id(), "S4813");
        assert_eq!(rule.name(), "XPath injection vulnerability detected");
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4813_detects_format_xpath_injection() {
        let source = r#"
            let expr = format!("//user[@name='{}']", username);
            doc.xpath(&expr);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect format! with XPath injection");
        assert_eq!(issues[0].rule_id, "S4813");
    }

    #[test]
    fn test_s4813_detects_xpath_with_replace() {
        let source = r#"
            let query = "/root/element[contains(text(), '{}')]".replace("{}", &user_input);
            doc.xpath(&query);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect .replace() XPath injection");
    }

    #[test]
    fn test_s4813_detects_string_concat_xpath() {
        let source = r#"
            let mut xpath = "//user[@name='".to_string();
            xpath += &username;
            xpath += "']";
            doc.xpath(&xpath);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect string concatenation XPath injection");
    }

    #[test]
    fn test_s4813_detects_xpath_dom_parse() {
        let source = r#"
            let doc = xpath::Dom::parse(&format!("//item[@id='{}']", item_id));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect xpath::Dom::parse with dynamic content");
    }

    #[test]
    fn test_s4813_detects_select_xpath() {
        let source = r#"
            let results = doc.select_xpath(&format!("//div[@class='{}']", class_name));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect select_xpath with dynamic content");
    }

    #[test]
    fn test_s4813_detects_sxd_xpath_evaluate() {
        let source = r#"
            let result = sxd_xpath::evaluate(&format!("//account[@id='{}']", id), &doc);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect sxd_xpath evaluate with dynamic content");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4813_false_positive_static_xpath() {
        let source = r#"
            doc.xpath("//user[@name='admin']");
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static XPath query");
    }

    #[test]
    fn test_s4813_false_positive_sanitized_input() {
        let source = r#"
            let escaped = xpath_escape(&user_input);
            doc.xpath(&format!("//user[@name='{}']", escaped));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect XPath with sanitized input");
    }

    #[test]
    fn test_s4813_false_positive_html_escape() {
        let source = r#"
            let safe = html_escape(&user_content);
            doc.xpath(&format!("//div[contains(text(), '{}')]", safe));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect XPath with html_escape");
    }

    #[test]
    fn test_s4813_false_positive_comment() {
        let source = r#"
            // doc.xpath(&format!("//user[@name='{}']", username));
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect XPath in comment");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s4813_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s4813_edge_case_multiline_xpath() {
        let source = r#"
            let query = format!(
                "//user[@name='{}']",
                username
            );
            doc.xpath(&query);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect multiline format! XPath");
    }

    #[test]
    fn test_s4813_edge_case_nested_xpath() {
        let source = r#"
            let expr = format!("//a[contains(@href, '{}')]//b[text()='{}']", href, text);
            doc.select_xpath(&expr);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect nested dynamic XPath");
    }

    #[test]
    fn test_s4813_edge_case_doc_comment() {
        let source = r#"
            /// Builds xpath like: `format!("//user[@name='{}']", name)`
            fn build_xpath(name: &str) -> String {
                format!("//user[@name='{}']", name)
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S4813Rule::new();
            rule.check(ctx)
        });
        // The actual code inside function should still trigger
        assert!(!issues.is_empty(), "Should detect xpath in function body despite doc comment");
    }
}