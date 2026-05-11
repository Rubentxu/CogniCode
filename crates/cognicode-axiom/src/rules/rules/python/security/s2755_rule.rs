//! S2755 — XXE (XML External Entity) vulnerability
//!
//! Detects lxml.etree.parse() without disabling entity resolution.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2755"
    name: "XML parsing should not use external entities"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "XML parsing with external entity resolution enabled can lead to XXE attacks, allowing attackers to read local files, perform SSRF attacks, or cause denial of service.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect etree.parse() or lxml.etree.parse() - check for parse followed by XMLParser with resolve_entities=False
        let etree_parse = regex::Regex::new(r"etree\.parse\s*\(").unwrap();
        let resolve_entities_false = regex::Regex::new(r"resolve_entities\s*=\s*False").unwrap();
        
        // Track if resolve_entities=False appears before we find an issue
        let source_lines: Vec<&str> = ctx.source.lines().collect();
        
        for (line_num, line) in source_lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if etree_parse.is_match(trimmed) {
                // Check if this line or nearby lines have resolve_entities=False
                let has_safe_parser = trimmed.contains("resolve_entities=False") || 
                    (line_num > 0 && source_lines[line_num - 1].contains("resolve_entities=False"));
                if !has_safe_parser {
                    issues.push(Issue::new(
                        "PY_S2755",
                        "XXE vulnerability - etree.parse() without resolve_entities=False",
                        Severity::Critical,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::substantial(
                        "Use XMLParser(resolve_entities=False) or defusedxml library to prevent XXE attacks."
                    )));
                }
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;

    fn with_python_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Python.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Python,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_s2755_registered() {
        let rule = PY_S2755Rule::new();
        assert_eq!(rule.id(), "PY_S2755");
    }

    #[test]
    fn test_s2755_detects_unsafe_etree_parse() {
        let rule = PY_S2755Rule::new();
        let smelly = r#"
from lxml import etree
tree = etree.parse("config.xml")
"#;
        let issues = with_python_context(smelly, "parser.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unsafe etree.parse()");
        assert_eq!(issues[0].rule_id, "PY_S2755");
    }

    #[test]
    fn test_s2755_allows_safe_etree_parse() {
        let rule = PY_S2755Rule::new();
        let clean = r#"
from lxml import etree
parser = etree.XMLParser(resolve_entities=False)
tree = etree.parse("config.xml", parser)
"#;
        let issues = with_python_context(clean, "parser.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag etree.parse() with resolve_entities=False");
    }
}
