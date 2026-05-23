//! S5732 — Trusting URL Scheme Detection
//! Detects automatic trust of URL schemes like javascript: or data: in places
//! where they shouldn't be trusted (CWE-079, CWE-918).
//!
//! Languages: JavaScript, HTML, Rust (web frameworks)
//! Severity: Minor
//! Category: Vulnerability
//!
//! Dangerous patterns include:
//! - <a href="javascript:...">
//! - window.location = user_url
//! - Redirect to javascript: URLs
//! - data: URIs in places expecting same-origin resources
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S5732
const RULE_ID: &str = "S5732";
const RULE_NAME: &str = "Trusting URL scheme detected";
const SEVERITY: Severity = Severity::Minor;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Dangerous URL schemes that should not be trusted
static DANGEROUS_URL_SCHEMES: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"(?i)javascript\s*:"#).unwrap(),
        regex::Regex::new(r#"(?i)data\s*:"#).unwrap(),
        regex::Regex::new(r#"(?i)vbscript\s*:"#).unwrap(),
    ]
});

/// Places where URL schemes are set/trusted
static URL_SET_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // JavaScript DOM manipulation
        regex::Regex::new(r#"(?i)\.href\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)\.src\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)window\.location\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)location\.href\s*="#).unwrap(),
        regex::Regex::new(r#"(?i)document\.location\s*="#).unwrap(),
        // HTML attributes
        regex::Regex::new(r#"(?i)href\s*=\s*["']"#).unwrap(),
        regex::Regex::new(r#"(?i)src\s*=\s*["']"#).unwrap(),
        // Rust web frameworks - redirect
        regex::Regex::new(r#"(?i)\.redirect\("#).unwrap(),
        regex::Regex::new(r#"(?i)Redirect::"#).unwrap(),
        regex::Regex::new(r#"(?i)SeeOther|SeeOther::"#).unwrap(),
        regex::Regex::new(r#"(?i)Location\s*::"#).unwrap(),
        // URL parsing without scheme validation
        regex::Regex::new(r#"(?i)Url::parse\("#).unwrap(),
        regex::Regex::new(r#"(?i)reqwest.*get\("#).unwrap(),
        regex::Regex::new(r#"(?i)hyper.*Location"#).unwrap(),
    ]
});

/// Safe URL patterns - static or validated URLs
static SAFE_URL_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"^/[^/]"#).unwrap(),                    // Root-relative paths
        regex::Regex::new(r#"^https?://"#).unwrap(),                // Only http/https allowed
        regex::Regex::new(r#"^#"#).unwrap(),                       // Fragment only
        regex::Regex::new(r#"(?i)allow_schemes\s*\(\s*\["#).unwrap(), // Scheme allowlist
        regex::Regex::new(r#"(?i)url::parse.*scheme\s*\(\s*['\"]https?['\"]"#).unwrap(),
        regex::Regex::new(r#"(?i)check_scheme|validate_scheme"#).unwrap(),
    ]
});

/// User-controlled URL sources
static USER_URL_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"(?i)(?:user_url|redirect_url|return_url|next_url)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:query_param|url_param|request\.uri)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:http_referer|referer|header\["#).unwrap(),
        regex::Regex::new(r#"(?i)(?:req\.query|req\.params|request\.param)"#).unwrap(),
    ]
});

declare_rule! {
    id: "S5732"
    name: "Trusting URL scheme detected"
    severity: Minor
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "URLs with dangerous schemes like 'javascript:' or 'data:' are being trusted. This can enable XSS attacks when user-controlled URLs are used in locations that execute code."
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

            // Check for dangerous URL scheme
            let has_dangerous_scheme = DANGEROUS_URL_SCHEMES.iter().any(|re| re.is_match(trimmed));

            // Check for URL being set/trusted
            let has_url_set = URL_SET_PATTERNS.iter().any(|re| re.is_match(trimmed));

            // Check if it's a safe URL
            let has_safe_pattern = SAFE_URL_PATTERNS.iter().any(|re| re.is_match(trimmed));

            // Check for user-controlled URL
            let has_user_url = USER_URL_PATTERNS.iter().any(|re| re.is_match(trimmed));

            // If dangerous scheme in URL-setting context without safety check
            if has_dangerous_scheme && has_url_set && !has_safe_pattern {
                issues.push(Issue::new(
                    RULE_ID,
                    format!("Potentially dangerous URL scheme (javascript:, data:, vbscript:) detected in trusted context."),
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Validate URL schemes before use. Only allow safe schemes (http, https). \
                     Implement scheme allowlisting: url.scheme() == \"https\""
                )));
            }

            // If user-controlled URL without scheme validation
            if has_user_url && has_url_set && !has_safe_pattern {
                issues.push(Issue::new(
                    RULE_ID,
                    format!("User-controlled URL used without scheme validation."),
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Validate URL scheme before using user-controlled URLs. \
                     Only allow http and https schemes."
                )));
            }
        }

        issues
    }
}


/// Agent semantics for S5732 - Trusting URL Scheme
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S5732_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects automatic trust of dangerous URL schemes (javascript:, data:, vbscript:) in places where they can enable XSS attacks, such as href attributes, window.location, or redirect operations",
    fix_playbook: "1. Identify where URLs are being set/trusted\n\
                   2. If user-controlled, validate scheme before use\n\
                   3. Implement scheme allowlist (only http, https)\n\
                   4. For HTML: Use URL.createObjectURL() for blob URLs instead of data:\n\
                   5. For redirects: Validate URL origin or use path allowlist\n\
                   6. Consider using 'noopener' for external links\n\
                   7. Example (Rust): url.scheme() == \"https\" || url.scheme() == \"http\"",
    review_questions: &[
        "Is this URL user-controlled or from an untrusted source?",
        "What is the context where the URL is used (href, redirect, src)?",
        "Could a javascript: or data: URL be injected here?",
        "What schemes should be allowed in this context?",
        "Is there existing validation on the URL scheme?",
        "What is the impact if a javascript: URL is processed?"
    ],
    agent_actions: &[
        "Identify URL-setting operations (href, src, redirect, location)",
        "Detect dangerous schemes (javascript:, data:, vbscript:)",
        "Check for user-controlled URL sources",
        "Verify scheme validation is present",
        "Suggest scheme allowlisting",
        "Recommend URL origin validation for redirects"
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
    fn test_s5732_rule_properties() {
        let rule = TRUSTING_URL_SCHEMERule::new();
        assert_eq!(rule.id(), "S5732");
        assert_eq!(rule.name(), "Trusting URL scheme detected");
        assert_eq!(rule.severity(), Severity::Minor);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5732_detects_javascript_href() {
        let source = r#"
            <a href="javascript:alert('xss')">Click</a>
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect javascript: in href");
        assert_eq!(issues[0].rule_id, "S5732");
    }

    #[test]
    fn test_s5732_detects_window_location_assignment() {
        let source = r#"
            window.location = userProvidedUrl;
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect window.location with user URL");
    }

    #[test]
    fn test_s5732_detects_data_url_in_img_src() {
        let source = r#"
            <img src="data:text/html,<script>alert(1)</script>">
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect data: URL in src");
    }

    #[test]
    fn test_s5732_detects_redirect_to_javascript() {
        let source = r#"
            return Redirect::to(user_input_url);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect redirect with user URL");
    }

    #[test]
    fn test_s5732_detects_url_parse_without_validation() {
        let source = r#"
            let url = Url::parse(&user_redirect_url)?;
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect Url::parse with user URL");
    }

    #[test]
    fn test_s5732_detects_location_href() {
        let source = r#"
            location.href = response.redirectUrl;
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect location.href assignment");
    }

    #[test]
    fn test_s5732_detects_vbscript_scheme() {
        let source = r#"
            <a href="vbscript:msgbox('test')">Click</a>
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect vbscript: scheme");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5732_false_positive_static_https() {
        let source = r#"
            <a href="https://example.com/page">Link</a>
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static https URL");
    }

    #[test]
    fn test_s5732_false_positive_relative_path() {
        let source = r#"
            <a href="/static/page">Link</a>
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect relative path");
    }

    #[test]
    fn test_s5732_false_positive_fragment_only() {
        let source = r##"
            <a href="#section">Link</a>
        "##;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect fragment-only href");
    }

    #[test]
    fn test_s5732_false_positive_scheme_validation() {
        let source = r#"
            if url.scheme() == "https" || url.scheme() == "http" {
                redirect(&url);
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect when scheme validation is present");
    }

    #[test]
    fn test_s5732_false_positive_comment() {
        let source = r#"
            // href="javascript:void(0)"
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect javascript: in comment");
    }

    #[test]
    fn test_s5732_false_positive_allowlist() {
        let source = r#"
            let allowed = allow_schemes(["https", "http"]);
            if allowed.contains(&url.scheme()) {
                redirect(&url);
            }
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect when scheme allowlist is present");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5732_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s5732_edge_case_case_insensitive() {
        let source = r#"
            <a href="JAVASCRIPT:alert(1)">Click</a>
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect case-insensitive javascript: scheme");
    }

    #[test]
    fn test_s5732_edge_case_space_before_colon() {
        let source = r#"
            <a href="javascript :alert(1)">Click</a>
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect javascript : with space");
    }

    #[test]
    fn test_s5732_edge_case_multiple_urls() {
        let source = r#"
            <a href="javascript:alert(1)">Bad</a>
            <a href="/safe/page">Good</a>
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = TRUSTING_URL_SCHEMERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect at least the dangerous URL");
    }
}