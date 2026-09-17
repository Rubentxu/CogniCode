//! Code Verifier Trait — ISP-segregated interface for code verification
//!
//! This trait abstracts the verification of code files via compilation checks.
//! Implementations include RustVerifier (rustc-based verification).

use async_trait::async_trait;
use thiserror::Error;

/// Errors emitted by [`CodeVerifier`] adapters. Lives next to the trait
/// because it is the port's own contract (mirrors the
/// `CodeIntelligenceError` / `RefactorError` / `SearchError` precedent in
/// `domain::traits`). `Err` is reserved for infrastructure failures
/// (toolchain not found, subprocess crash, timeout). A "Verified",
/// "Rejected", or "Skipped" outcome is reported inside
/// [`CompilationResult`] as `Ok(_)`.
///
/// `From<CodeVerifierError>` for `AppError` is implemented at the
/// application-layer boundary so the trait stays free of presentation
/// concerns.
#[derive(Debug, Clone, Error)]
pub enum CodeVerifierError {
    /// The requested file could not be read (missing path, permissions, etc.).
    #[error("failed to read file for verification: {0}")]
    FileUnreadable(String),

    /// The verifier depends on an external toolchain that is not available
    /// on this host (e.g. `rustc` not in `PATH`).
    #[error("required toolchain not available: {0}")]
    ToolchainUnavailable(String),

    /// The verifier subprocess failed unexpectedly (not a `Rejected`
    /// compilation outcome).
    #[error("verifier subprocess failed: {0}")]
    SubprocessFailed(String),

    /// The verifier exceeded the supplied timeout budget.
    #[error("verification timed out after {0}s")]
    Timeout(u64),
}

/// Compilation result from a code verification pass.
#[derive(Debug, Clone)]
pub enum CompilationResult {
    /// File compiled successfully
    Verified {
        /// stdout from the compiler
        stdout: String,
    },
    /// File failed to compile
    Rejected {
        /// Error message from the compiler (truncated to 200 chars)
        error: String,
    },
    /// File was skipped (e.g., not a supported language)
    Skipped {
        /// Reason for skipping
        reason: String,
    },
}

/// Trait for verifying code files via compilation checks.
///
/// This follows the ISP (Interface Segregation Principle) by providing
/// a dedicated interface for code verification, separate from file operations.
#[async_trait]
pub trait CodeVerifier: Send + Sync {
    /// Synchronous verification of a single file.
    ///
    /// Returns `Ok(CompilationResult)` on success (including Skipped/Rejected).
    /// Returns `Err` only on infrastructure failures (e.g., rustc not found).
    fn verify(&self, path: &str) -> Result<CompilationResult, CodeVerifierError>;

    /// Verifies a file with a timeout limit.
    ///
    /// When the timeout is reached, the subprocess is killed and
    /// `Err(CodeVerifierError::Timeout(_))` is returned.
    ///
    /// This method is async to allow timeout enforcement without blocking.
    async fn verify_with_timeout(
        &self,
        path: &str,
        timeout_secs: u64,
    ) -> Result<CompilationResult, CodeVerifierError>;
}
