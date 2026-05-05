//! Configuration API - functions for dashboard configuration

use crate::state::DashboardConfig;
use serde::{Deserialize, Serialize};

/// Available rule profiles
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rule_count: usize,
}

/// Get available rule profiles
pub async fn get_rule_profiles() -> Result<Vec<RuleProfile>, String> {
    Ok(vec![
        RuleProfile {
            id: "sonarqube".to_string(),
            name: "SonarQube Default".to_string(),
            description: "Standard SonarQube rule set covering reliability, security, and maintainability".to_string(),
            rule_count: 150,
        },
        RuleProfile {
            id: "security-first".to_string(),
            name: "Security First".to_string(),
            description: "Focused on security vulnerabilities and secure coding practices".to_string(),
            rule_count: 85,
        },
        RuleProfile {
            id: "minimal".to_string(),
            name: "Minimal Rules".to_string(),
            description: "Lightweight set of essential rules for quick feedback".to_string(),
            rule_count: 25,
        },
        RuleProfile {
            id: "strict".to_string(),
            name: "Strict Mode".to_string(),
            description: "Comprehensive rules for maximum code quality".to_string(),
            rule_count: 250,
        },
    ])
}

/// Get current dashboard configuration
pub async fn get_configuration() -> Result<DashboardConfig, String> {
    // TODO: Load from persistent storage or environment
    Ok(DashboardConfig::default())
}

/// Save dashboard configuration
pub async fn save_configuration(config: DashboardConfig) -> Result<(), String> {
    // TODO: Persist to storage
    // For now, just validate the config
    if config.project_path.is_empty() {
        return Err("Project path is required".to_string());
    }
    if config.coverage_threshold < 0.0 || config.coverage_threshold > 100.0 {
        return Err("Coverage threshold must be between 0 and 100".to_string());
    }
    Ok(())
}

/// Validate project path
pub async fn validate_project_path(path: String) -> Result<bool, String> {
    // TODO: Actually check if path exists and is a valid project
    Ok(!path.is_empty() && path.starts_with('/'))
}
