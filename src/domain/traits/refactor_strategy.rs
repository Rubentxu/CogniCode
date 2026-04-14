//! Trait for refactoring strategy operations
//!
//! Provides methods for validating, preparing, and executing refactoring operations.

#[allow(unused_imports)]
use crate::domain::aggregates::Symbol;
use crate::domain::aggregates::{Refactor, RefactorKind};
use crate::domain::value_objects::Location;

/// Strategy for performing refactoring operations
pub trait RefactorStrategy: Send + Sync {
    /// Validates a refactor operation
    fn validate(&self, refactor: &Refactor) -> RefactorValidation;

    /// Prepares the edits for a refactor operation
    fn prepare_edits(&self, refactor: &Refactor) -> Result<PreparedEdits, RefactorError>;

    /// Executes a refactor operation
    fn execute(&self, refactor: &Refactor) -> Result<RefactorResult, RefactorError>;

    /// Returns the supported refactor kinds for this strategy
    fn supported_kinds(&self) -> Vec<RefactorKind>;

    /// Checks if this strategy can handle the given refactor
    fn can_handle(&self, refactor: &Refactor) -> bool {
        self.supported_kinds().contains(refactor.kind())
    }
}

/// Result of validating a refactor
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefactorValidation {
    /// Whether the refactor is valid
    pub is_valid: bool,
    /// Errors if invalid
    pub errors: Vec<ValidationError>,
    /// Warnings if valid but with concerns
    pub warnings: Vec<String>,
    /// The refactor with updated validation state
    pub refactor: Refactor,
}

impl RefactorValidation {
    /// Creates a successful validation
    pub fn success(refactor: Refactor) -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            refactor,
        }
    }

    /// Creates a failed validation
    pub fn failure(errors: Vec<ValidationError>, refactor: Refactor) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
            refactor,
        }
    }

    /// Adds a warning
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

/// Validation error
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Error code
    pub code: ValidationErrorCode,
    /// Error message
    pub message: String,
    /// Optional location related to the error
    pub location: Option<Location>,
}

impl ValidationError {
    /// Creates a new validation error
    pub fn new(code: ValidationErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            location: None,
        }
    }

    /// Creates a new validation error with location
    pub fn with_location(
        code: ValidationErrorCode,
        message: impl Into<String>,
        location: Location,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            location: Some(location),
        }
    }
}

/// Validation error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationErrorCode {
    /// Symbol not found
    SymbolNotFound,
    /// Name conflict
    NameConflict,
    /// Would break dependencies
    WouldBreakDependencies,
    /// Would create cycles
    WouldCreateCycles,
    /// Invalid parameters
    InvalidParameters,
    /// File access error
    FileAccessError,
    /// Unsupported operation
    UnsupportedOperation,
}

/// Prepared edits for a refactor
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedEdits {
    /// The edits to apply
    pub edits: Vec<crate::domain::aggregates::RefactorParameters>,
    /// Files to modify
    pub files_to_modify: Vec<std::path::PathBuf>,
    /// Files to create
    pub files_to_create: Vec<FileCreation>,
    /// Files to delete
    pub files_to_delete: Vec<std::path::PathBuf>,
}

impl PreparedEdits {
    /// Creates empty prepared edits
    pub fn empty() -> Self {
        Self {
            edits: Vec::new(),
            files_to_modify: Vec::new(),
            files_to_create: Vec::new(),
            files_to_delete: Vec::new(),
        }
    }

    /// Returns the total number of changes
    pub fn total_changes(&self) -> usize {
        self.edits.len() + self.files_to_create.len() + self.files_to_delete.len()
    }
}

/// File to be created during refactoring
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCreation {
    /// Path for the new file
    pub path: std::path::PathBuf,
    /// Initial content
    pub content: String,
}

/// Result of executing a refactor
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefactorResult {
    /// Whether the refactor was successful
    pub success: bool,
    /// The refactor that was executed
    pub refactor: Refactor,
    /// Files that were modified
    pub modified_files: Vec<std::path::PathBuf>,
    /// Files that were created
    pub created_files: Vec<std::path::PathBuf>,
    /// Files that were deleted
    pub deleted_files: Vec<std::path::PathBuf>,
    /// Error message if failed
    pub error: Option<String>,
}

impl RefactorResult {
    /// Creates a successful result
    pub fn success(refactor: Refactor) -> Self {
        Self {
            success: true,
            refactor,
            modified_files: Vec::new(),
            created_files: Vec::new(),
            deleted_files: Vec::new(),
            error: None,
        }
    }

    /// Creates a failed result
    pub fn failure(refactor: Refactor, error: impl Into<String>) -> Self {
        Self {
            success: false,
            refactor,
            modified_files: Vec::new(),
            created_files: Vec::new(),
            deleted_files: Vec::new(),
            error: Some(error.into()),
        }
    }

    /// Adds modified files to the result
    pub fn with_modified_files(mut self, files: Vec<std::path::PathBuf>) -> Self {
        self.modified_files = files;
        self
    }

    /// Adds created files to the result
    pub fn with_created_files(mut self, files: Vec<std::path::PathBuf>) -> Self {
        self.created_files = files;
        self
    }

    /// Adds deleted files to the result
    pub fn with_deleted_files(mut self, files: Vec<std::path::PathBuf>) -> Self {
        self.deleted_files = files;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::{RefactorParameters, Symbol};
    use crate::domain::value_objects::{Location, SymbolKind};

    struct MockRefactorStrategy {
        validation_result: RefactorValidation,
        prepare_edits: Option<PreparedEdits>,
        execute_ok: bool,
    }

    impl MockRefactorStrategy {
        fn new() -> Self {
            let symbol = Symbol::new(
                "test_func",
                SymbolKind::Function,
                Location::new("test.rs", 1, 1),
            );
            let refactor = Refactor::new(RefactorKind::Rename, symbol, RefactorParameters::new());
            Self {
                validation_result: RefactorValidation::success(refactor),
                prepare_edits: None,
                execute_ok: true,
            }
        }

        fn with_validation_failure(
            mut self,
            errors: Vec<ValidationError>,
            refactor: Refactor,
        ) -> Self {
            self.validation_result = RefactorValidation::failure(errors, refactor);
            self
        }

        fn with_prepare_edits(mut self, edits: PreparedEdits) -> Self {
            self.prepare_edits = Some(edits);
            self
        }

        fn with_execute_failure(mut self) -> Self {
            self.execute_ok = false;
            self
        }
    }

    impl RefactorStrategy for MockRefactorStrategy {
        fn validate(&self, _refactor: &Refactor) -> RefactorValidation {
            self.validation_result.clone()
        }

        fn prepare_edits(&self, _refactor: &Refactor) -> Result<PreparedEdits, RefactorError> {
            match &self.prepare_edits {
                Some(edits) => Ok(edits.clone()),
                None => Ok(PreparedEdits::empty()),
            }
        }

        fn execute(&self, refactor: &Refactor) -> Result<RefactorResult, RefactorError> {
            if self.execute_ok {
                Ok(RefactorResult::success(refactor.clone()))
            } else {
                Ok(RefactorResult::failure(
                    refactor.clone(),
                    "Mock execution failure",
                ))
            }
        }

        fn supported_kinds(&self) -> Vec<RefactorKind> {
            vec![RefactorKind::Rename, RefactorKind::Extract]
        }
    }

    fn create_test_refactor() -> Refactor {
        let symbol = Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        Refactor::new(RefactorKind::Rename, symbol, RefactorParameters::new())
    }

    #[test]
    fn test_mock_validate_success() {
        let strategy = MockRefactorStrategy::new();
        let refactor = create_test_refactor();
        let validation = strategy.validate(&refactor);
        assert!(validation.is_valid);
        assert!(validation.errors.is_empty());
    }

    #[test]
    fn test_mock_validate_failure() {
        let errors = vec![ValidationError::new(
            ValidationErrorCode::SymbolNotFound,
            "Symbol not found",
        )];
        let refactor = create_test_refactor();
        let strategy =
            MockRefactorStrategy::new().with_validation_failure(errors, refactor.clone());
        let validation = strategy.validate(&refactor);
        assert!(!validation.is_valid);
        assert_eq!(validation.errors.len(), 1);
        assert_eq!(
            validation.errors[0].code,
            ValidationErrorCode::SymbolNotFound
        );
    }

    #[test]
    fn test_mock_prepare_edits() {
        let prepared = PreparedEdits {
            edits: Vec::new(),
            files_to_modify: vec![std::path::PathBuf::from("test.rs")],
            files_to_create: Vec::new(),
            files_to_delete: Vec::new(),
        };
        let strategy = MockRefactorStrategy::new().with_prepare_edits(prepared);
        let refactor = create_test_refactor();
        let result = strategy.prepare_edits(&refactor);
        assert!(result.is_ok());
        let edits = result.unwrap();
        assert_eq!(edits.files_to_modify.len(), 1);
        assert_eq!(
            edits.files_to_modify[0],
            std::path::PathBuf::from("test.rs")
        );
    }

    #[test]
    fn test_refactor_validation_is_valid() {
        let refactor = create_test_refactor();
        let success_validation = RefactorValidation::success(refactor.clone());
        assert!(success_validation.is_valid);
        let failure_validation = RefactorValidation::failure(
            vec![ValidationError::new(
                ValidationErrorCode::NameConflict,
                "Name conflict",
            )],
            refactor,
        );
        assert!(!failure_validation.is_valid);
    }

    #[test]
    fn test_refactor_result_construction() {
        let refactor = create_test_refactor();
        let result = RefactorResult::success(refactor.clone());
        assert!(result.success);
        assert_eq!(result.refactor.kind(), &RefactorKind::Rename);
        assert!(result.modified_files.is_empty());
        assert!(result.created_files.is_empty());
        assert!(result.deleted_files.is_empty());
        assert!(result.error.is_none());
        let failure_result = RefactorResult::failure(refactor.clone(), "Execution failed");
        assert!(!failure_result.success);
        assert!(failure_result.error.is_some());
        assert_eq!(failure_result.error.unwrap(), "Execution failed");
    }
}

/// Error type for refactor operations
#[derive(Debug, thiserror::Error)]
pub enum RefactorError {
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Preparation failed: {0}")]
    PreparationFailed(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Unsupported refactor kind: {0:?}")]
    UnsupportedKind(RefactorKind),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("IO error: {0}")]
    IoError(String),
}
