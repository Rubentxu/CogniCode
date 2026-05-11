//! S5547 — Weak cipher (DES, RC4, Blowfish)
//!
//! Detects usage of weak symmetric encryption algorithms.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S5547"
    name: "Strong encryption algorithms should be used"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "DES, RC4, and Blowfish are weak encryption algorithms susceptible to attacks. They should not be used for securing sensitive data.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect weak cipher algorithms
        let weak_ciphers = [
            r"DES\.new\s*\(",
            r"ARC4\.new\s*\(",
            r"Blowfish\.new\s*\(",
            r#"['"].*DES.*['"]"#,
            r#"['"].*RC4.*['"]"#,
            r#"['"].*Blowfish.*['"]"#,
            r"cryptography\.hazmat\.primitives\.ciphers\.algorithms\.ARC4",
        ];
        let re = weak_ciphers.iter()
            .map(|p| regex::Regex::new(p).unwrap())
            .collect::<Vec<_>>();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            for regex in &re {
                if regex.is_match(line) {
                    issues.push(Issue::new(
                        "PY_S5547",
                        format!("Weak cipher detected (DES, RC4, or Blowfish)"),
                        Severity::Critical,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::moderate(
                        "Use AES or ChaCha20 from the cryptography library for strong encryption."
                    )));
                    break;
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
    fn test_s5547_registered() {
        let rule = PY_S5547Rule::new();
        assert_eq!(rule.id(), "PY_S5547");
    }

    #[test]
    fn test_s5547_detects_rc4() {
        let rule = PY_S5547Rule::new();
        let smelly = r#"
from Crypto.Cipher import ARC4
cipher = ARC4.new(key)
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect RC4");
        assert_eq!(issues[0].rule_id, "PY_S5547");
    }

    #[test]
    fn test_s5547_allows_aes() {
        let rule = PY_S5547Rule::new();
        let clean = r#"
from cryptography.hazmat.primitives.ciphers import AES
cipher = AES.new(key, AES.MODE_ECB)
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag AES");
    }
}
