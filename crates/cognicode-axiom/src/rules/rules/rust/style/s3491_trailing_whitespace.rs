//! S3491 — Lines should not end with trailing whitespace
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S3491"
    name: "Lines should not end with trailing whitespace"
    severity: Minor
    category: CodeSmell
    language: "*"
    params: {
    }

    explanation: "Trailing whitespace is unnecessary and can cause noise in diffs and version control. Remove trailing spaces and tabs from the end of lines.",
    clean_code: Complete,
    impacts: [Maintainability: Low],

    agent_semantics: {
        summary: "Detects lines ending with trailing whitespace - unnecessary spaces that cause noise in diffs",
        fix_playbook: "1. Identify the line with trailing whitespace\n2. Remove all trailing spaces and tabs from that line\n3. Ensure the line ends immediately before the newline character",
        review_questions: [
            "Is the whitespace genuinely trailing (not intentional indentation)?",
            "Could this be in a string literal or comment where it's meaningful?"
        ],
        semantic_chunks: [
            "Trailing whitespace is unnecessary and can cause noise in diffs",
            "Lines should end without any spaces or tabs before the newline",
            "This is a minor code style issue"
        ],
        safe_autofix: true,
        autofix_guidance: "Safe to autofix - simply remove trailing whitespace characters"
    }

    check: => {
        let mut issues = Vec::new();

        // Pre-compile regex once - pattern is constant
        let re = regex::Regex::new(r"[ \t]+$").unwrap();

        // Comment prefixes to skip (lines starting with these are comments)
        let comment_prefixes = ["//", "///", "//!", "#", "/*", "*"];

        for (line_num, line) in ctx.source.lines().enumerate() {
            // Skip empty lines
            if line.is_empty() {
                continue;
            }

            // Skip comment lines - check if trimmed line starts with comment marker
            let trimmed = line.trim_start();
            if comment_prefixes.iter().any(|prefix| trimmed.starts_with(*prefix)) {
                continue;
            }

            // Check for trailing whitespace
            if re.is_match(line) {
                // Count the trailing whitespace characters
                let trailing_count: usize = line.chars().rev().take_while(|c| c.is_whitespace()).count();

                issues.push(Issue::new(
                    "S3491",
                    format!(
                        "Line ends with {} trailing whitespace character{}",
                        trailing_count,
                        if trailing_count == 1 { "" } else { "s" }
                    ),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ));
            }
        }
        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(S3491Rule::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s3491_registered() {
        let rule = S3491Rule::new();
        assert_eq!(rule.id(), "S3491");
        assert!(rule.name().len() > 0);
    }

    #[test]
    fn test_s3491_severity_and_category() {
        let rule = S3491Rule::new();
        assert_eq!(rule.severity(), Severity::Minor);
        assert_eq!(rule.category(), Category::CodeSmell);
    }
}
