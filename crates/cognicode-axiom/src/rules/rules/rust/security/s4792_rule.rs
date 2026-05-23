//! S4792 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

declare_rule! {
    id: "S4792"
    name: "Weak cryptography should not be used"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "Weak cryptographic algorithms like MD5, SHA1, DES, and RC4 are vulnerable to modern attacks and should not be used for security-sensitive operations.",
    clean_code: Trustworthy,
    impacts: [Security: High, Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let weak_patterns = [
            (r"(?i)\bmd5\b", "MD5 hash function"),
            (r"(?i)\bsha1?\b", "SHA-0/SHA-1 hash function"),
            // Match des/rc4 when preceded by underscore (common in function names like encrypt_with_des)
            // Also match when followed by ( or preceded by word boundary
            (r"(?i)(?:\b|_)des\b", "DES block cipher"),
            (r"(?i)(?:\b|_)3des\b", "Triple DES (3DES) block cipher"),
            (r"(?i)(?:\b|_)rc4\b", "RC4 stream cipher"),
            (r"(?i)\bcrypt\b", "crypt(3) function"),
        ];

        let compiled_patterns: Vec<(regex::Regex, &str)> = weak_patterns
            .iter()
            .filter_map(|(p, d)| {
                match regex::Regex::new(p) {
                    Ok(r) => Some((r, *d)),
                    Err(e) => {
                        eprintln!("Warning: Failed to compile S4792 pattern '{}': {}", p, e);
                        None
                    }
                }
            })
            .collect();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            for (re, description) in &compiled_patterns {
                if let Some(m) = re.find(line) {
                    let pt = m.start();
                    issues.push(Issue::new(
                        "S4792",
                        format!(
                            "Use of weak cryptography: {} detected on line {}",
                            description, line_idx + 1
                        ),
                        Severity::Critical,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_column(pt + 1)
                    .with_remediation(Remediation::substantial(
                        "Use a modern cryptographic algorithm (e.g., SHA-256, AES-256-GCM)"
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

    #[test]
    fn test_s4792_registered() {
        let rule = S4792Rule::new();
        assert_eq!(rule.id(), "S4792");
        assert!(rule.name().len() > 0);
    }
}
