//! Rule validator — validates Cedar policies
//!
//! Provides basic validation for Cedar policy text.
//! Note: Full Cedar validation requires the cedar-policy crate which has been removed
//! from this crate. This validator performs basic syntax and semantic checks only.

use crate::error::{AxiomResult, ValidationDiagnostic, DiagnosticSeverity};

/// Validates Cedar policies
#[derive(Debug)]
pub struct RuleValidator {
    // Schema support removed - cedar-policy dependency removed
}

/// Validation result for a policy
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationResult {
    /// Whether the policy passed validation (no errors)
    pub is_valid: bool,
    /// All diagnostics (errors, warnings, info)
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl RuleValidator {
    /// Create a new validator
    pub fn new() -> Self {
        Self {}
    }

    /// Create a validator with the default schema (no-op, kept for API compatibility)
    pub fn with_default_schema() -> AxiomResult<Self> {
        Ok(Self::new())
    }

    /// Validate a single Cedar policy text
    pub fn validate(&self, policy_text: &str) -> ValidationResult {
        let mut diagnostics = Vec::new();

        // 1. Basic syntax checks
        self.check_basic_syntax(policy_text, &mut diagnostics);

        // 2. Semantic checks
        self.check_semantic(policy_text, &mut diagnostics);

        ValidationResult {
            is_valid: diagnostics.iter().all(|d| d.severity != DiagnosticSeverity::Error),
            diagnostics,
        }
    }

    /// Validate a batch of policies
    pub fn validate_batch(&self, policies: &[String]) -> Vec<ValidationResult> {
        policies.iter().map(|p| self.validate(p)).collect()
    }

    /// Check for basic syntax issues
    fn check_basic_syntax(&self, policy_text: &str, diagnostics: &mut Vec<ValidationDiagnostic>) {
        let trimmed = policy_text.trim();

        // Check for empty policy
        if trimmed.is_empty() {
            diagnostics.push(ValidationDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: "Policy is empty".to_string(),
                line: None,
                column: None,
                code: Some("EMPTY_POLICY".to_string()),
            });
            return;
        }

        // Check for valid policy structure (permit/forbid)
        let has_permit = trimmed.contains("permit(");
        let has_forbid = trimmed.contains("forbid(");

        if !has_permit && !has_forbid {
            diagnostics.push(ValidationDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: "Policy must start with 'permit(' or 'forbid('".to_string(),
                line: None,
                column: None,
                code: Some("MISSING_PERMIT_FORBID".to_string()),
            });
        }

        // Check for balanced parentheses
        let open_count = trimmed.matches('(').count();
        let close_count = trimmed.matches(')').count();
        if open_count != close_count {
            diagnostics.push(ValidationDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: format!("Unbalanced parentheses: {} '(' but {} ')'", open_count, close_count),
                line: None,
                column: None,
                code: Some("UNBALANCED_PARENS".to_string()),
            });
        }

        // Check for semicolon at end
        if !trimmed.ends_with(';') {
            diagnostics.push(ValidationDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: "Policy must end with ';'".to_string(),
                line: None,
                column: None,
                code: Some("MISSING_SEMICOLON".to_string()),
            });
        }
    }

    /// Check for common semantic issues
    fn check_semantic(&self, policy_text: &str, diagnostics: &mut Vec<ValidationDiagnostic>) {
        let trimmed = policy_text.trim();

        // Check for overly broad permit
        if trimmed.contains("permit(principal, action, resource);")
            || trimmed.contains("permit(principal, action == Action::\"*\", resource);")
        {
            diagnostics.push(ValidationDiagnostic {
                severity: DiagnosticSeverity::Warning,
                message: "Policy grants unrestricted access (permit all)".to_string(),
                line: None,
                column: None,
                code: Some("OVERLY_BROAD".to_string()),
            });
        }

        // Check for missing when/unless conditions on permit/forbid
        if (trimmed.contains("permit(") || trimmed.contains("forbid("))
            && !trimmed.contains("when")
            && !trimmed.contains("unless")
        {
            diagnostics.push(ValidationDiagnostic {
                severity: DiagnosticSeverity::Info,
                message: "Consider adding 'when' or 'unless' conditions to restrict this permit/forbid".to_string(),
                line: None,
                column: None,
                code: Some("NO_CONDITIONS".to_string()),
            });
        }
    }
}

impl Default for RuleValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_policy() {
        let validator = RuleValidator::new();
        let result = validator.validate(r#"permit(principal, action == Action::"file_read", resource);"#);
        assert!(result.is_valid);
        assert!(result.diagnostics.is_empty() || result.diagnostics.iter().all(|d| d.severity != DiagnosticSeverity::Error));
    }

    #[test]
    fn test_invalid_syntax() {
        let validator = RuleValidator::new();
        let result = validator.validate(r#"this is garbage"#);
        assert!(!result.is_valid);
        assert!(result.diagnostics.iter().any(|d| d.severity == DiagnosticSeverity::Error));
    }

    #[test]
    fn test_empty_policy() {
        let validator = RuleValidator::new();
        let result = validator.validate(r#""#);
        assert!(!result.is_valid);
        assert!(result.diagnostics.iter().any(|d| d.code.as_deref() == Some("EMPTY_POLICY")));
    }

    #[test]
    fn test_overly_broad_warning() {
        let validator = RuleValidator::new();
        let result = validator.validate(r#"permit(principal, action, resource);"#);
        assert!(result.is_valid); // Warning, not error
        assert!(result.diagnostics.iter().any(|d| d.code.as_deref() == Some("OVERLY_BROAD")));
    }

    #[test]
    fn test_batch_validation() {
        let validator = RuleValidator::new();
        let results = validator.validate_batch(&[
            r#"permit(principal, action, resource);"#.to_string(),
            r#"invalid syntax"#.to_string(),
        ]);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_valid); // has warnings but no errors
        assert!(!results[1].is_valid);
    }
}
