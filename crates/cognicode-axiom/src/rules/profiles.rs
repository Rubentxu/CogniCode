//! Quality Profiles with YAML
//!
//! Implements section 5 of doc 09: Quality profiles that configure rules,
//! their severity, and parameters via YAML configuration.

use std::collections::HashMap;

use serde::Deserialize;

use crate::error::{AxiomError, AxiomResult};
use crate::rules::types::Severity;

/// A rule configuration within a quality profile
#[derive(Debug, Clone, Deserialize)]
pub struct RuleConfig {
    /// The rule identifier (e.g., "S138", "S3776")
    pub rule_id: String,
    /// Whether this rule is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Override severity for this rule
    #[serde(default)]
    pub severity: Option<String>,
    /// Rule-specific parameters
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

/// A quality profile defining which rules to run and their configuration
#[derive(Debug, Clone, Deserialize)]
pub struct QualityProfile {
    /// Profile name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Programming language this profile applies to
    pub language: String,
    /// Whether this is the default profile for the language
    #[serde(default)]
    pub is_default: bool,
    /// Optional parent profile to extend
    #[serde(default)]
    pub extends: Option<String>,
    /// Rule configurations
    pub rules: Vec<RuleConfig>,
}

/// Default value helper for serde
fn default_true() -> bool {
    true
}

/// A resolved profile with all inheritance and defaults applied
#[derive(Debug, Clone)]
pub struct ResolvedProfile {
    /// The original profile name
    pub name: String,
    /// Language this profile applies to
    pub language: String,
    /// Resolved rule configurations
    pub rules: HashMap<String, ResolvedRuleConfig>,
}

/// A resolved rule configuration
#[derive(Debug, Clone)]
pub struct ResolvedRuleConfig {
    /// Rule identifier
    pub rule_id: String,
    /// Whether the rule is enabled
    pub enabled: bool,
    /// Resolved severity
    pub severity: Severity,
    /// Rule parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

impl Default for ResolvedRuleConfig {
    fn default() -> Self {
        Self {
            rule_id: String::new(),
            enabled: true,
            severity: Severity::Minor,
            parameters: HashMap::new(),
        }
    }
}

/// Engine for loading and resolving quality profiles
#[derive(Debug)]
pub struct QualityProfileEngine {
    profiles: HashMap<String, QualityProfile>,
    default_profile: Option<String>,
}

impl QualityProfileEngine {
    /// Create a new engine with no profiles loaded
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            default_profile: None,
        }
    }

    /// Load profiles from YAML content
    pub fn from_yaml(yaml_content: &str) -> AxiomResult<Self> {
        // Parse the YAML - could be a single profile or a list
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_content)
            .map_err(|e| AxiomError::Other(format!("YAML parse error: {}", e)))?;

        let mut profiles = HashMap::new();
        let mut default_profile = None;

        // Handle both single profile and list of profiles
        match parsed {
            serde_yaml::Value::Sequence(items) => {
                for item in items {
                    let profile: QualityProfile = serde_yaml::from_value(item)
                        .map_err(|e| AxiomError::Other(format!("Profile parse error: {}", e)))?;
                    
                    if profile.is_default {
                        default_profile = Some(profile.name.clone());
                    }
                    profiles.insert(profile.name.clone(), profile);
                }
            }
            serde_yaml::Value::Mapping(map) => {
                // Single profile case - check if it has a 'profiles' key for bulk format
                if map.contains_key("profiles") {
                    if let Some(serde_yaml::Value::Sequence(profile_list)) = map.get("profiles") {
                        for item in profile_list {
                            let profile: QualityProfile = serde_yaml::from_value(item.clone())
                                .map_err(|e| AxiomError::Other(format!("Profile parse error: {}", e)))?;
                            
                            if profile.is_default {
                                default_profile = Some(profile.name.clone());
                            }
                            profiles.insert(profile.name.clone(), profile);
                        }
                    }
                } else {
                    // Single profile directly
                    let profile: QualityProfile = serde_yaml::from_value(serde_yaml::Value::Mapping(map))
                        .map_err(|e| AxiomError::Other(format!("Profile parse error: {}", e)))?;
                    
                    if profile.is_default {
                        default_profile = Some(profile.name.clone());
                    }
                    profiles.insert(profile.name.clone(), profile);
                }
            }
            _ => {
                return Err(AxiomError::Other(
                    "Invalid YAML format: expected profile or list of profiles".to_string()
                ));
            }
        }

        Ok(Self {
            profiles,
            default_profile,
        })
    }

    /// Add a profile to the engine
    pub fn add_profile(&mut self, profile: QualityProfile) {
        if profile.is_default {
            self.default_profile = Some(profile.name.clone());
        }
        self.profiles.insert(profile.name.clone(), profile);
    }

    /// Get a profile by name
    pub fn get(&self, name: &str) -> Option<&QualityProfile> {
        self.profiles.get(name)
    }

    /// Get the default profile for a language
    pub fn get_default_for_language(&self, language: &str) -> Option<&QualityProfile> {
        self.profiles
            .values()
            .find(|p| p.language == language && p.is_default)
            .or_else(|| {
                // Fall back to any profile for this language marked as default
                self.profiles
                    .values()
                    .find(|p| p.language == language && p.name == self.default_profile.as_deref().unwrap_or(""))
            })
            .or_else(|| {
                // Last resort: first profile for this language
                self.profiles.values().find(|p| p.language == language)
            })
    }

    /// Resolve a profile by name, applying inheritance
    pub fn resolve_profile(&self, name: &str) -> ResolvedProfile {
        // First find the profile
        let profile = match self.profiles.get(name) {
            Some(p) => p,
            None => {
                // Try to find by language default
                match self.get_default_for_language(name) {
                    Some(p) => p,
                    None => {
                        // Return empty resolved profile
                        return ResolvedProfile {
                            name: name.to_string(),
                            language: String::new(),
                            rules: HashMap::new(),
                        };
                    }
                }
            }
        };

        self.resolve_profile_internal(profile)
    }

    /// Internal method to resolve a profile with inheritance
    fn resolve_profile_internal(&self, profile: &QualityProfile) -> ResolvedProfile {
        // Start with inherited profile if specified
        let mut resolved_rules: HashMap<String, ResolvedRuleConfig> = HashMap::new();

        // Process inheritance chain
        if let Some(ref extends) = profile.extends {
            if let Some(parent) = self.profiles.get(extends) {
                let parent_resolved = self.resolve_profile_internal(parent);
                resolved_rules = parent_resolved.rules;
            }
        }

        // Apply this profile's rules (override inherited)
        for rule_config in &profile.rules {
            let resolved = ResolvedRuleConfig {
                rule_id: rule_config.rule_id.clone(),
                enabled: rule_config.enabled,
                severity: self.parse_severity(&rule_config.severity),
                parameters: rule_config.parameters.clone(),
            };
            resolved_rules.insert(rule_config.rule_id.clone(), resolved);
        }

        ResolvedProfile {
            name: profile.name.clone(),
            language: profile.language.clone(),
            rules: resolved_rules,
        }
    }

    /// Parse a severity string to Severity enum
    fn parse_severity(&self, severity_str: &Option<String>) -> Severity {
        match severity_str {
            Some(s) => match s.to_lowercase().as_str() {
                "info" => Severity::Info,
                "minor" => Severity::Minor,
                "major" => Severity::Major,
                "critical" => Severity::Critical,
                "blocker" => Severity::Blocker,
                _ => Severity::Minor,
            },
            None => Severity::Minor,
        }
    }

    /// List all profile names
    pub fn profile_names(&self) -> Vec<&String> {
        self.profiles.keys().collect()
    }
}

impl Default for QualityProfileEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PROFILE_YAML: &str = r#"
name: "Sonar way"
description: "Sonar way quality profile"
language: "rust"
is_default: true
rules:
  - rule_id: "S138"
    enabled: true
    severity: "major"
    parameters:
      threshold: 50
  - rule_id: "S3776"
    enabled: true
    severity: "major"
  - rule_id: "S1066"
    enabled: false
"#;

    const PROFILE_WITH_INHERITANCE_YAML: &str = r#"
- name: "parent-profile"
  description: "Parent profile"
  language: "rust"
  rules:
    - rule_id: "S138"
      enabled: true
      severity: "major"
    - rule_id: "S3776"
      enabled: true

- name: "child-profile"
  description: "Child profile extending parent"
  language: "rust"
  extends: "parent-profile"
  rules:
    - rule_id: "S138"
      severity: "critical"
    - rule_id: "S2306"
      enabled: true
"#;

    #[test]
    fn test_parse_single_profile() {
        let engine = QualityProfileEngine::from_yaml(SAMPLE_PROFILE_YAML).unwrap();
        
        let profile = engine.get("Sonar way").unwrap();
        assert_eq!(profile.name, "Sonar way");
        assert_eq!(profile.language, "rust");
        assert!(profile.is_default);
        assert_eq!(profile.rules.len(), 3);
    }

    #[test]
    fn test_parse_profile_list() {
        let engine = QualityProfileEngine::from_yaml(PROFILE_WITH_INHERITANCE_YAML).unwrap();
        
        assert!(engine.get("parent-profile").is_some());
        assert!(engine.get("child-profile").is_some());
    }

    #[test]
    fn test_resolve_profile() {
        let engine = QualityProfileEngine::from_yaml(SAMPLE_PROFILE_YAML).unwrap();
        
        let resolved = engine.resolve_profile("Sonar way");
        
        assert_eq!(resolved.name, "Sonar way");
        assert!(resolved.rules.contains_key("S138"));
        
        let s138 = resolved.rules.get("S138").unwrap();
        assert_eq!(s138.severity, Severity::Major);
        assert_eq!(s138.parameters.get("threshold").and_then(|v| v.as_u64()), Some(50));
    }

    #[test]
    fn test_resolve_profile_inheritance() {
        let engine = QualityProfileEngine::from_yaml(PROFILE_WITH_INHERITANCE_YAML).unwrap();
        
        let resolved = engine.resolve_profile("child-profile");
        
        // S138 should be overridden to critical
        let s138 = resolved.rules.get("S138").unwrap();
        assert_eq!(s138.severity, Severity::Critical);
        
        // S3776 should be inherited from parent
        let s3776 = resolved.rules.get("S3776").unwrap();
        assert_eq!(s3776.severity, Severity::Minor); // Default severity
        
        // S2306 should be from child
        let s2306 = resolved.rules.get("S2306").unwrap();
        assert!(s2306.enabled);
    }

    #[test]
    fn test_resolve_by_language() {
        let engine = QualityProfileEngine::from_yaml(SAMPLE_PROFILE_YAML).unwrap();
        
        let resolved = engine.resolve_profile("rust");
        assert_eq!(resolved.name, "Sonar way");
    }

    #[test]
    fn test_resolve_nonexistent() {
        let engine = QualityProfileEngine::from_yaml(SAMPLE_PROFILE_YAML).unwrap();
        
        let resolved = engine.resolve_profile("nonexistent");
        assert!(resolved.rules.is_empty());
    }
}
