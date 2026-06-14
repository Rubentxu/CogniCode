//! Interface Error Types
//!
//! Errors at the MCP interface layer. Preserves causal chain via thiserror's #[source].

use thiserror::Error;
use crate::interface::mcp::security::SecurityError;
use crate::interface::mcp::handlers::HandlerError;

#[derive(Error, Debug)]
pub enum InterfaceError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("security: {0}")]
    Security(#[from] SecurityError),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("internal: {0}")]
    Internal(String),

    /// Catch-all for domain and application errors, preserving the causal chain
    #[error("domain: {0}")]
    Domain(#[source] crate::domain::error::DomainError),

    /// Application-level errors that don't map to domain errors
    #[error("application: {0}")]
    Application(#[source] crate::application::error::AppError),
}

impl From<crate::domain::error::DomainError> for InterfaceError {
    fn from(err: crate::domain::error::DomainError) -> Self {
        match err {
            crate::domain::error::DomainError::SymbolNotFound(s) => {
                InterfaceError::NotFound(format!("symbol not found: {}", s))
            }
            crate::domain::error::DomainError::InvalidSymbolKind(s) => {
                InterfaceError::InvalidInput(format!("invalid symbol kind: {}", s))
            }
            crate::domain::error::DomainError::CycleDetected => {
                InterfaceError::InvalidInput("cycle detected in dependency graph".into())
            }
            crate::domain::error::DomainError::LocationOutOfBounds(s) => {
                InterfaceError::InvalidInput(format!("location out of bounds: {}", s))
            }
            crate::domain::error::DomainError::ParseError(s) => {
                InterfaceError::InvalidInput(format!("parse error: {}", s))
            }
            crate::domain::error::DomainError::AnalysisError(_) => {
                InterfaceError::Domain(err)
            }
            crate::domain::error::DomainError::RefactoringError(_) => {
                InterfaceError::Domain(err)
            }
            crate::domain::error::DomainError::InvalidRange(s) => {
                InterfaceError::InvalidInput(format!("invalid range: {}", s))
            }
        }
    }
}

impl From<crate::application::error::AppError> for InterfaceError {
    fn from(err: crate::application::error::AppError) -> Self {
        match err {
            crate::application::error::AppError::SymbolNotFound(s) => {
                InterfaceError::NotFound(format!("symbol not found: {}", s))
            }
            crate::application::error::AppError::LocationNotFound(s) => {
                InterfaceError::NotFound(format!("location not found: {}", s))
            }
            crate::application::error::AppError::NavigationError(s) => {
                InterfaceError::InvalidInput(format!("navigation error: {}", s))
            }
            crate::application::error::AppError::AnalysisError(_) => {
                InterfaceError::Application(err)
            }
            crate::application::error::AppError::CycleDetected => {
                InterfaceError::InvalidInput("cycle detected in dependency graph".into())
            }
            crate::application::error::AppError::InvalidCallGraph(s) => {
                InterfaceError::InvalidInput(format!("invalid call graph: {}", s))
            }
            crate::application::error::AppError::RefactorError(_) => {
                InterfaceError::Application(err)
            }
            crate::application::error::AppError::InvalidRefactorAction(s) => {
                InterfaceError::InvalidInput(format!("invalid refactor action: {}", s))
            }
            crate::application::error::AppError::RefactorTargetNotFound(s) => {
                InterfaceError::NotFound(format!("refactor target not found: {}", s))
            }
            crate::application::error::AppError::StrategyNotFound(s) => {
                InterfaceError::NotFound(format!("strategy not found: {}", s))
            }
            crate::application::error::AppError::SafetyCheckFailed(s) => {
                InterfaceError::InvalidInput(format!("safety check failed: {}", s))
            }
            crate::application::error::AppError::SearchError(s) => {
                InterfaceError::InvalidInput(format!("search error: {}", s))
            }
            crate::application::error::AppError::InspectionError(s) => {
                InterfaceError::InvalidInput(format!("inspection error: {}", s))
            }
            crate::application::error::AppError::RepositoryError(s) => {
                InterfaceError::Internal(format!("repository error: {}", s))
            }
            crate::application::error::AppError::InvalidParameter(s) => {
                InterfaceError::InvalidInput(format!("invalid parameter: {}", s))
            }
            crate::application::error::AppError::InternalError(s) => {
                InterfaceError::Internal(s)
            }
            crate::application::error::AppError::NotImplemented(s) => {
                InterfaceError::Internal(format!("not implemented: {}", s))
            }
        }
    }
}

// From<SecurityError> for InterfaceError is auto-generated by #[from] SecurityError

impl From<crate::domain::traits::CodeIntelligenceError> for InterfaceError {
    fn from(err: crate::domain::traits::CodeIntelligenceError) -> Self {
        InterfaceError::InvalidInput(format!("navigation error: {}", err))
    }
}

impl From<crate::domain::traits::RefactorError> for InterfaceError {
    fn from(err: crate::domain::traits::RefactorError) -> Self {
        InterfaceError::Application(crate::application::error::AppError::RefactorError(err.to_string()))
    }
}

impl From<crate::domain::traits::DependencyError> for InterfaceError {
    fn from(err: crate::domain::traits::DependencyError) -> Self {
        InterfaceError::Internal(format!("dependency error: {}", err))
    }
}

impl From<crate::domain::traits::SearchError> for InterfaceError {
    fn from(err: crate::domain::traits::SearchError) -> Self {
        InterfaceError::InvalidInput(format!("search error: {}", err))
    }
}

// Note: From<SecurityError> for InterfaceError is auto-generated by #[from] SecurityError

impl From<serde_json::Error> for InterfaceError {
    fn from(err: serde_json::Error) -> Self {
        InterfaceError::InvalidInput(format!("JSON error: {}", err))
    }
}

impl From<HandlerError> for InterfaceError {
    fn from(err: HandlerError) -> Self {
        // HandlerError is a wrapper enum that can contain InterfaceError-convertible types
        use crate::interface::mcp::handlers::HandlerError as HE;
        match err {
            HE::Security(e) => InterfaceError::Security(e),
            HE::App(e) => InterfaceError::Application(e),
            HE::Domain(e) => InterfaceError::Domain(e),
            HE::InvalidInput(s) => InterfaceError::InvalidInput(s),
            HE::NotFound(s) => InterfaceError::NotFound(s),
            HE::Internal(s) => InterfaceError::Internal(s),
        }
    }
}

/// Result type for interface operations
pub type InterfaceResult<T> = Result<T, InterfaceError>;
