//! Refactor DTO - Data Transfer Objects for refactoring operations

use crate::domain::aggregates::refactor::{RefactorKind, ValidationResult};
use serde::{Deserialize, Serialize};

/// DTO for refactoring plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorPlanDto {
    /// Plan ID
    pub id: String,
    /// Description of the refactoring
    pub description: String,
    /// Kind of refactoring
    pub kind: String,
    /// Number of actions in the plan
    pub action_count: usize,
    /// Whether the plan is validated
    pub is_validated: bool,
    /// Whether the plan is safe to apply
    pub is_safe: bool,
    /// Impact level (1-10)
    pub impact_level: u8,
}

impl RefactorPlanDto {
    /// Creates a new RefactorPlanDto
    pub fn new(id: impl Into<String>, description: impl Into<String>, kind: RefactorKind) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            kind: format!("{:?}", kind),
            action_count: 0,
            is_validated: false,
            is_safe: false,
            impact_level: 1,
        }
    }
}

/// DTO for validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResultDto {
    /// Whether the validation passed
    pub is_valid: bool,
    /// Severity level
    pub severity: String,
    /// Number of impacted symbols
    pub impacted_symbol_count: usize,
    /// Whether cycles were detected
    pub has_cycles: bool,
    /// List of warnings
    pub warnings: Vec<String>,
}

impl From<&ValidationResult> for ValidationResultDto {
    fn from(result: &ValidationResult) -> Self {
        Self {
            is_valid: result.is_valid,
            severity: format!("{:?}", result.severity()),
            impacted_symbol_count: result.impacted_symbol_count,
            has_cycles: result.has_cycles,
            warnings: result.warnings.clone(),
        }
    }
}

/// DTO for refactoring preview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorPreviewDto {
    /// Description of what will change
    pub description: String,
    /// Files that will be modified
    pub files_to_modify: Vec<String>,
    /// Symbols that will be affected
    pub symbols_affected: Vec<String>,
    /// Estimated number of changes
    pub change_count: usize,
    /// Risk assessment
    pub risk_assessment: String,
}

impl RefactorPreviewDto {
    /// Creates a new RefactorPreviewDto
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            files_to_modify: Vec::new(),
            symbols_affected: Vec::new(),
            change_count: 0,
            risk_assessment: "unknown".to_string(),
        }
    }

    /// Sets files to modify
    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files_to_modify = files;
        self
    }

    /// Sets symbols affected
    pub fn with_symbols(mut self, symbols: Vec<String>) -> Self {
        self.symbols_affected = symbols;
        self
    }

    /// Sets risk assessment
    pub fn with_risk(mut self, risk: impl Into<String>) -> Self {
        self.risk_assessment = risk.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::refactor::{ValidationResult, ValidationSeverity};
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::value_objects::{Location, SymbolKind};

    #[test]
    fn test_refactor_plan_dto_new() {
        let dto = RefactorPlanDto::new("plan_1", "Test refactoring", RefactorKind::Rename);
        assert_eq!(dto.id, "plan_1");
        assert_eq!(dto.description, "Test refactoring");
        assert_eq!(dto.kind, "Rename");
        assert_eq!(dto.action_count, 0);
        assert!(!dto.is_validated);
        assert!(!dto.is_safe);
        assert_eq!(dto.impact_level, 1);
    }

    #[test]
    fn test_refactor_plan_dto_default_values() {
        let dto = RefactorPlanDto::new("id", "desc", RefactorKind::Extract);
        assert_eq!(dto.action_count, 0);
        assert!(!dto.is_validated);
        assert!(!dto.is_safe);
    }

    #[test]
    fn test_validation_result_dto_from_validation_result() {
        let result = ValidationResult {
            is_valid: true,
            impacted_symbol_count: 5,
            has_cycles: false,
            breaking_changes: Vec::new(),
            warnings: vec!["Warning 1".to_string()],
        };
        let dto = ValidationResultDto::from(&result);
        assert!(dto.is_valid);
        assert_eq!(dto.impacted_symbol_count, 5);
        assert!(!dto.has_cycles);
        assert_eq!(dto.warnings, vec!["Warning 1"]);
    }

    #[test]
    fn test_validation_result_dto_invalid_with_cycles() {
        let result = ValidationResult {
            is_valid: false,
            impacted_symbol_count: 10,
            has_cycles: true,
            breaking_changes: Vec::new(),
            warnings: Vec::new(),
        };
        let dto = ValidationResultDto::from(&result);
        assert!(!dto.is_valid);
        assert!(dto.has_cycles);
        assert_eq!(dto.severity, "Error");
    }

    #[test]
    fn test_refactor_preview_dto_new() {
        let dto = RefactorPreviewDto::new("Extract function");
        assert_eq!(dto.description, "Extract function");
        assert!(dto.files_to_modify.is_empty());
        assert!(dto.symbols_affected.is_empty());
        assert_eq!(dto.change_count, 0);
        assert_eq!(dto.risk_assessment, "unknown");
    }

    #[test]
    fn test_refactor_preview_dto_builder_pattern() {
        let dto = RefactorPreviewDto::new("Test preview")
            .with_files(vec!["file1.rs".to_string(), "file2.rs".to_string()])
            .with_symbols(vec!["func1".to_string(), "func2".to_string()])
            .with_risk("low");
        assert_eq!(dto.files_to_modify.len(), 2);
        assert_eq!(dto.symbols_affected.len(), 2);
        assert_eq!(dto.risk_assessment, "low");
    }

    #[test]
    fn test_refactor_preview_dto_with_files() {
        let dto = RefactorPreviewDto::new("desc").with_files(vec!["main.rs".to_string()]);
        assert_eq!(dto.files_to_modify, vec!["main.rs"]);
    }

    #[test]
    fn test_refactor_preview_dto_with_symbols() {
        let dto = RefactorPreviewDto::new("desc")
            .with_symbols(vec!["symbol1".to_string(), "symbol2".to_string()]);
        assert_eq!(dto.symbols_affected.len(), 2);
    }

    #[test]
    fn test_refactor_preview_dto_with_risk() {
        let dto = RefactorPreviewDto::new("desc").with_risk("high");
        assert_eq!(dto.risk_assessment, "high");
    }
}
