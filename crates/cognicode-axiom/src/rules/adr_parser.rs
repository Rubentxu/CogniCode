//! ADR (Architecture Decision Record) parser
//!
//! Parses ADR markdown files with YAML frontmatter and converts them
//! to Cedar policy rules.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AxiomError, AxiomResult};

/// A parsed Architecture Decision Record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedAdr {
    /// ADR identifier (e.g., "ADR-001")
    pub id: String,
    /// Title of the decision
    pub title: String,
    /// Status: proposed, accepted, deprecated, superseded
    pub status: String,
    /// Date of the decision
    pub date: Option<String>,
    /// People who made the decision
    pub deciders: Vec<String>,
    /// Context section
    pub context: String,
    /// Decision section
    pub decision: String,
    /// Consequences section
    pub consequences: String,
}

/// ADR parser with frontmatter support
pub struct AdrParser;

impl AdrParser {
    /// Parse ADR content from a string
    pub fn parse(content: &str) -> AxiomResult<ParsedAdr> {
        let (frontmatter, body) = split_frontmatter(content);

        let id = extract_adr_id(&frontmatter, body);
        let title = extract_title(body).unwrap_or_else(|| "Untitled ADR".to_string());
        let status = frontmatter
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("proposed")
            .to_string();
        let date = frontmatter
            .get("date")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let deciders = frontmatter
            .get("deciders")
            .and_then(|v| v.as_str())
            .map(|s| {
                s.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let context = extract_section(body, "Context").unwrap_or_default();
        let decision = extract_section(body, "Decision").unwrap_or_default();
        let consequences = extract_section(body, "Consequences").unwrap_or_default();

        Ok(ParsedAdr {
            id,
            title,
            status,
            date,
            deciders,
            context,
            decision,
            consequences,
        })
    }

    /// Parse an ADR file from disk
    pub fn parse_file(path: &Path) -> AxiomResult<ParsedAdr> {
        let content = std::fs::read_to_string(path).map_err(|e| AxiomError::Io {
            context: format!("Reading ADR file {}", path.display()),
            source: e,
        })?;
        Self::parse(&content)
    }

    /// Convert a parsed ADR to Cedar policy text
    pub fn to_cedar_rules(adr: &ParsedAdr) -> Vec<String> {
        let mut rules = Vec::new();

        // Only generate rules for accepted ADRs
        if adr.status != "accepted" {
            return rules;
        }

        // Generate a policy that references the ADR
        let policy = format!(
            r#"// Generated from {} - {}
// Status: {}
permit(
    principal,
    action,
    resource
) when {{
    context.adr_id == "{}"
}};"#,
            adr.id, adr.title, adr.status, adr.id
        );

        rules.push(policy);
        rules
    }
}

/// Split markdown content into YAML frontmatter and body
fn split_frontmatter(content: &str) -> (serde_json::Map<String, serde_json::Value>, &str) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (serde_json::Map::new(), content);
    }

    let rest = &trimmed[3..]; // skip opening ---
    if let Some(end) = rest.find("---") {
        let yaml_str = &rest[..end].trim();
        let body = &rest[end + 3..];

        // Simple YAML-like parsing (key: value)
        let mut map = serde_json::Map::new();
        for line in yaml_str.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_string();
                let value = value.trim().trim_matches('"').to_string();
                map.insert(key, serde_json::Value::String(value));
            }
        }

        (map, body)
    } else {
        (serde_json::Map::new(), content)
    }
}

/// Extract ADR ID from frontmatter or title
fn extract_adr_id(frontmatter: &serde_json::Map<String, serde_json::Value>, body: &str) -> String {
    frontmatter
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            let title = extract_title(body)?;
            // Try to extract "ADR-XXX" from title like "# ADR-001: Use Cedar..."
            let re = regex::Regex::new(r"ADR[- ]\d+").ok()?;
            re.find(&title).map(|m| m.as_str().to_string())
        })
        .unwrap_or_else(|| "ADR-UNKNOWN".to_string())
}

/// Extract the first markdown heading, stripping trailing colons
fn extract_title(body: &str) -> Option<String> {
    body.lines()
        .find(|l| l.starts_with('#'))
        .map(|l| l.trim_start_matches('#').trim().trim_end_matches(':').to_string())
}

/// Extract a named section from markdown body
fn extract_section(body: &str, section_name: &str) -> Option<String> {
    let section_header = format!("## {}", section_name);
    let mut lines = body.lines();
    let mut found = false;
    let mut content = String::new();

    while let Some(line) = lines.next() {
        if line.trim() == section_header {
            found = true;
            continue;
        }
        if found {
            if line.starts_with("## ") {
                break;
            }
            content.push_str(line);
            content.push('\n');
        }
    }

    if found && !content.trim().is_empty() {
        Some(content.trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ADR: &str = r#"---
status: accepted
date: 2026-04-30
deciders: alice, bob
---

# ADR-001: Use Cedar for authorization

## Context

We need a policy engine for governance.

## Decision

Use Cedar Policy from AWS.

## Consequences

Positive: Fast evaluation, type-safe.
Negative: AWS dependency.
"#;

    #[test]
    fn test_parse_adr() {
        let adr = AdrParser::parse(SAMPLE_ADR).unwrap();
        assert_eq!(adr.id, "ADR-001");
        assert_eq!(adr.status, "accepted");
        assert_eq!(adr.deciders, vec!["alice", "bob"]);
        assert!(adr.context.contains("policy engine"));
        assert!(adr.decision.contains("Cedar"));
    }

    #[test]
    fn test_to_cedar_rules_accepted() {
        let adr = AdrParser::parse(SAMPLE_ADR).unwrap();
        let rules = AdrParser::to_cedar_rules(&adr);
        assert_eq!(rules.len(), 1);
        assert!(rules[0].contains("ADR-001"));
    }

    #[test]
    fn test_to_cedar_rules_deprecated() {
        let deprecated = SAMPLE_ADR.replace("accepted", "deprecated");
        let adr = AdrParser::parse(&deprecated).unwrap();
        let rules = AdrParser::to_cedar_rules(&adr);
        assert!(rules.is_empty(), "Deprecated ADRs should not generate rules");
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let simple = "# Just a title\n\n## Context\n\nSome context.\n";
        let adr = AdrParser::parse(simple).unwrap();
        assert_eq!(adr.status, "proposed");
    }
}
