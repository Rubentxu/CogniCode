//! SonarQube rule metadata importer
//!
//! Imports rule metadata from SonarQube JSON exports and provides
//! utilities for generating Rust rule stubs.

use serde::{Deserialize, Serialize};

/// Metadata for a rule imported from SonarQube API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedRule {
    pub rule_id: String,
    pub name: String,
    pub severity: String,
    pub rule_type: String,
    pub language: String,
    pub description: String,
    pub parameters: Vec<RuleParameter>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleParameter {
    pub name: String,
    pub description: String,
    pub default_value: Option<String>,
    pub param_type: String,
}

/// Rule catalog loaded from a JSON file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCatalog {
    pub version: String,
    pub source: String,
    pub exported_at: String,
    pub rules: Vec<ImportedRule>,
}

impl RuleCatalog {
    /// Load a catalog from a JSON file
    pub fn from_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let catalog: Self = serde_json::from_str(&content)?;
        Ok(catalog)
    }

    /// Filter rules by language
    pub fn for_language(&self, lang: &str) -> Vec<&ImportedRule> {
        self.rules.iter().filter(|r| r.language == lang).collect()
    }

    /// Filter rules by type (CODE_SMELL, BUG, VULNERABILITY, SECURITY_HOTSPOT)
    pub fn for_type(&self, rule_type: &str) -> Vec<&ImportedRule> {
        self.rules.iter().filter(|r| r.rule_type == rule_type).collect()
    }

    /// Generate Rust declare_rule! stubs for rules not yet implemented
    pub fn generate_rust_stubs(&self, existing_ids: &[&str]) -> String {
        let existing_set: std::collections::HashSet<_> = existing_ids.iter().collect();
        let mut output = String::from("// Auto-generated rule stubs\n\n");
        
        for rule in &self.rules {
            if existing_set.contains(&rule.rule_id.as_str()) {
                continue;
            }
            
            let severity = match rule.severity.as_str() {
                "BLOCKER" => "Blocker",
                "CRITICAL" => "Critical",
                "MAJOR" => "Major",
                "MINOR" => "Minor",
                "INFO" => "Info",
                _ => "Info",
            };
            
            let category = match rule.rule_type.as_str() {
                "CODE_SMELL" => "CodeSmell",
                "BUG" => "Bug",
                "VULNERABILITY" => "Vulnerability",
                "SECURITY_HOTSPOT" => "SecurityHotspot",
                _ => "CodeSmell",
            };
            
            output.push_str(&format!(
                "// TODO: {} - {}\n\
                 // declare_rule! {{\n\
                 //     id: \"{}\"\n\
                 //     name: \"{}\"\n\
                 //     severity: {}\n\
                 //     category: {}\n\
                 //     language: \"{}\"\n\
                 //     params: {{}}\n\
                 //     check: |ctx| {{\n\
                 //         let mut issues = Vec::new();\n\
                 //         // TODO: Implement detection logic\n\
                 //         issues\n\
                 //     }}\n\
                 // }}\n\n",
                severity, category, rule.rule_id, rule.name,
                severity, category, rule.language
            ));
        }
        
        output
    }
}

/// Create a sample SonarQube export JSON (for testing/demo)
pub fn create_sample_catalog() -> RuleCatalog {
    RuleCatalog {
        version: "1.0".to_string(),
        source: "sonarqube-api".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        rules: vec![
            ImportedRule {
                rule_id: "S1226".to_string(),
                name: "Method parameters should not be reassigned".to_string(),
                severity: "MAJOR".to_string(),
                rule_type: "CODE_SMELL".to_string(),
                language: "rust".to_string(),
                description: "Reassigning parameter values makes code harder to understand".to_string(),
                parameters: vec![],
                tags: vec!["bad-practice".to_string()],
            },
            ImportedRule {
                rule_id: "S1186".to_string(),
                name: "Empty functions should be removed or completed".to_string(),
                severity: "MAJOR".to_string(),
                rule_type: "CODE_SMELL".to_string(),
                language: "rust".to_string(),
                description: "Empty functions add noise and may indicate incomplete work".to_string(),
                parameters: vec![],
                tags: vec!["suspicious".to_string()],
            },
            ImportedRule {
                rule_id: "S1871".to_string(),
                name: "Branches in conditional structure should not have exactly the same implementation".to_string(),
                severity: "MAJOR".to_string(),
                rule_type: "BUG".to_string(),
                language: "rust".to_string(),
                description: "Duplicate branches indicate copy-paste errors".to_string(),
                parameters: vec![],
                tags: vec!["bug".to_string()],
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_sample_catalog() {
        let catalog = create_sample_catalog();
        assert_eq!(catalog.rules.len(), 3);
    }

    #[test]
    fn test_filter_by_language() {
        let catalog = create_sample_catalog();
        let rust_rules = catalog.for_language("rust");
        assert_eq!(rust_rules.len(), 3);
    }

    #[test]
    fn test_generate_stubs() {
        let catalog = create_sample_catalog();
        let existing = ["S138", "S3776"];
        let stubs = catalog.generate_rust_stubs(&existing);
        assert!(stubs.contains("S1226"));
        assert!(stubs.contains("S1186"));
        assert!(stubs.contains("S1871"));
    }
}