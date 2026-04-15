//! Application Layer Error Types
//!
//! Error types for the application layer.

use thiserror::Error;

/// Application-level error type
#[derive(Error, Debug)]
pub enum AppError {
    // Navigation errors
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Location not found: {0}")]
    LocationNotFound(String),

    #[error("Navigation error: {0}")]
    NavigationError(String),

    // Analysis errors
    #[error("Analysis error: {0}")]
    AnalysisError(String),

    #[error("Cycle detected in dependency graph")]
    CycleDetected,

    #[error("Invalid call graph: {0}")]
    InvalidCallGraph(String),

    // Refactor errors
    #[error("Refactor error: {0}")]
    RefactorError(String),

    #[error("Invalid refactor action: {0}")]
    InvalidRefactorAction(String),

    #[error("Refactor target not found: {0}")]
    RefactorTargetNotFound(String),

    #[error("Strategy not found: {0}")]
    StrategyNotFound(String),

    #[error("Safety check failed: {0}")]
    SafetyCheckFailed(String),

    // Inspection errors
    #[error("Search error: {0}")]
    SearchError(String),

    #[error("Inspection error: {0}")]
    InspectionError(String),

    // Repository errors
    #[error("Repository error: {0}")]
    RepositoryError(String),

    // General errors
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

// Note: AppError already implements std::error::Error via thiserror,
// so it can be converted to anyhow::Error via the blanket impl.

impl From<crate::domain::error::DomainError> for AppError {
    fn from(err: crate::domain::error::DomainError) -> Self {
        match err {
            crate::domain::error::DomainError::SymbolNotFound(s) => AppError::SymbolNotFound(s),
            crate::domain::error::DomainError::CycleDetected => AppError::CycleDetected,
            crate::domain::error::DomainError::AnalysisError(s) => AppError::AnalysisError(s),
            crate::domain::error::DomainError::RefactoringError(s) => AppError::RefactorError(s),
            _ => AppError::InternalError(err.to_string()),
        }
    }
}

impl From<crate::domain::traits::CodeIntelligenceError> for AppError {
    fn from(err: crate::domain::traits::CodeIntelligenceError) -> Self {
        AppError::NavigationError(err.to_string())
    }
}

impl From<crate::domain::traits::RefactorError> for AppError {
    fn from(err: crate::domain::traits::RefactorError) -> Self {
        AppError::RefactorError(err.to_string())
    }
}

impl From<crate::domain::traits::DependencyError> for AppError {
    fn from(err: crate::domain::traits::DependencyError) -> Self {
        AppError::RepositoryError(err.to_string())
    }
}

impl From<crate::domain::traits::SearchError> for AppError {
    fn from(err: crate::domain::traits::SearchError) -> Self {
        AppError::SearchError(err.to_string())
    }
}

/// Result type for application operations
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_not_found_error() {
        let err = AppError::SymbolNotFound("main".to_string());
        assert!(err.to_string().contains("main"));
        assert!(matches!(err, AppError::SymbolNotFound(_)));
    }

    #[test]
    fn test_cycle_detected_error() {
        let err = AppError::CycleDetected;
        let msg = err.to_string();
        assert!(msg.contains("Cycle detected"));
        assert!(matches!(err, AppError::CycleDetected));
    }

    #[test]
    fn test_navigation_error() {
        let err = AppError::NavigationError("connection failed".to_string());
        assert!(err.to_string().contains("Navigation error"));
        assert!(err.to_string().contains("connection failed"));
        assert!(matches!(err, AppError::NavigationError(_)));
    }

    #[test]
    fn test_analysis_error() {
        let err = AppError::AnalysisError("parse failed".to_string());
        assert!(err.to_string().contains("Analysis error"));
        assert!(err.to_string().contains("parse failed"));
        assert!(matches!(err, AppError::AnalysisError(_)));
    }

    #[test]
    fn test_refactor_error() {
        let err = AppError::RefactorError("rename failed".to_string());
        assert!(err.to_string().contains("Refactor error"));
        assert!(err.to_string().contains("rename failed"));
        assert!(matches!(err, AppError::RefactorError(_)));
    }

    #[test]
    fn test_invalid_refactor_action_error() {
        let err = AppError::InvalidRefactorAction("unknown action".to_string());
        assert!(err.to_string().contains("Invalid refactor action"));
        assert!(err.to_string().contains("unknown action"));
        assert!(matches!(err, AppError::InvalidRefactorAction(_)));
    }

    #[test]
    fn test_safety_check_failed_error() {
        let err = AppError::SafetyCheckFailed("unsafe operation".to_string());
        assert!(err.to_string().contains("Safety check failed"));
        assert!(err.to_string().contains("unsafe operation"));
        assert!(matches!(err, AppError::SafetyCheckFailed(_)));
    }

    #[test]
    fn test_repository_error() {
        let err = AppError::RepositoryError("db connection lost".to_string());
        assert!(err.to_string().contains("Repository error"));
        assert!(err.to_string().contains("db connection lost"));
        assert!(matches!(err, AppError::RepositoryError(_)));
    }

    #[test]
    fn test_internal_error() {
        let err = AppError::InternalError("unexpected".to_string());
        assert!(err.to_string().contains("Internal error"));
        assert!(err.to_string().contains("unexpected"));
        assert!(matches!(err, AppError::InternalError(_)));
    }

    #[test]
    fn test_not_implemented_error() {
        let err = AppError::NotImplemented("feature x".to_string());
        assert!(err.to_string().contains("Not implemented"));
        assert!(err.to_string().contains("feature x"));
        assert!(matches!(err, AppError::NotImplemented(_)));
    }

    #[test]
    fn test_location_not_found_error() {
        let err = AppError::LocationNotFound("pos: 10:5".to_string());
        assert!(err.to_string().contains("Location not found"));
        assert!(err.to_string().contains("pos: 10:5"));
        assert!(matches!(err, AppError::LocationNotFound(_)));
    }

    #[test]
    fn test_invalid_call_graph_error() {
        let err = AppError::InvalidCallGraph("missing node".to_string());
        assert!(err.to_string().contains("Invalid call graph"));
        assert!(err.to_string().contains("missing node"));
        assert!(matches!(err, AppError::InvalidCallGraph(_)));
    }

    #[test]
    fn test_refactor_target_not_found_error() {
        let err = AppError::RefactorTargetNotFound("func @old".to_string());
        assert!(err.to_string().contains("Refactor target not found"));
        assert!(err.to_string().contains("func @old"));
        assert!(matches!(err, AppError::RefactorTargetNotFound(_)));
    }

    #[test]
    fn test_strategy_not_found_error() {
        let err = AppError::StrategyNotFound("extract_method".to_string());
        assert!(err.to_string().contains("Strategy not found"));
        assert!(err.to_string().contains("extract_method"));
        assert!(matches!(err, AppError::StrategyNotFound(_)));
    }

    #[test]
    fn test_search_error() {
        let err = AppError::SearchError("no results".to_string());
        assert!(err.to_string().contains("Search error"));
        assert!(err.to_string().contains("no results"));
        assert!(matches!(err, AppError::SearchError(_)));
    }

    #[test]
    fn test_inspection_error() {
        let err = AppError::InspectionError("type mismatch".to_string());
        assert!(err.to_string().contains("Inspection error"));
        assert!(err.to_string().contains("type mismatch"));
        assert!(matches!(err, AppError::InspectionError(_)));
    }

    #[test]
    fn test_invalid_parameter_error() {
        let err = AppError::InvalidParameter("negative value".to_string());
        assert!(err.to_string().contains("Invalid parameter"));
        assert!(err.to_string().contains("negative value"));
        assert!(matches!(err, AppError::InvalidParameter(_)));
    }

    #[test]
    fn test_error_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AppError>();
    }

    #[test]
    fn test_error_debug_format() {
        let err = AppError::SymbolNotFound("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("SymbolNotFound"));
    }
}
