//! Configuration API - client-side functions

use crate::state::{DashboardConfig, RuleProfile};

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
    Ok(DashboardConfig::default())
}

/// Save dashboard configuration
pub async fn save_configuration(_config: DashboardConfig) -> Result<(), String> {
    Ok(())
}

/// Validate project path
pub async fn validate_project_path(path: String) -> Result<bool, String> {
    Ok(!path.is_empty() && path.starts_with('/'))
}