//! Domain Error Types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Invalid symbol kind: {0}")]
    InvalidSymbolKind(String),

    #[error("Cycle detected in dependency graph")]
    CycleDetected,

    #[error("Location out of bounds: {0}")]
    LocationOutOfBounds(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Analysis error: {0}")]
    AnalysisError(String),

    #[error("Refactoring error: {0}")]
    RefactoringError(String),

    #[error("Invalid range: {0}")]
    InvalidRange(String),
}

/// Common error variants shared across all layers.
/// Use as a base enum that other error types convert from via `?`.
#[derive(Error, Debug, Clone)]
pub enum CommonError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("internal error: {0}")]
    Internal(String),
}

// Note: DomainError already implements std::error::Error via thiserror,
// so it can be converted to anyhow::Error via the blanket impl.
